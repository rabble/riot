//! App resource bundle: a fixed list of `(path, content_type, bytes)`
//! resources plus a primary entry point, deterministically CBOR-encoded in
//! the same manual style as `model/mod.rs::encode_alert`.
//!
//! This is a minimal, self-contained resource-pack format, not a
//! byte-for-byte WICG Web Bundle (`.wbn`) — the design doc names WICG Web
//! Bundle as the packaging inspiration, but nothing outside this crate's
//! own decoder ever parses these bytes: the native host unpacks a bundle
//! and serves its resources locally to an embedded webview. Full binary
//! spec compliance would buy nothing here, so we keep the encoding manual,
//! strictly ordered, and bounded like the rest of this crate's codecs
//! instead of pulling in a WICG-compliant framing implementation.
//!
//! Not to be confused with the unrelated `import::bundle` module, which
//! encodes evidence-import artifacts (`RiotEvidenceBundleV1`).

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use super::AppsError;

const APP_BUNDLE_DIGEST_DOMAIN: &[u8] = b"riot/app-bundle/v1";

pub const MAX_BUNDLE_RESOURCES: usize = 32;
pub const MAX_RESOURCE_PATH_BYTES: usize = 256;
pub const MAX_RESOURCE_CONTENT_TYPE_BYTES: usize = 64;
pub const MAX_BUNDLE_TOTAL_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppResource {
    pub path: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBundle {
    pub entry_point: String,
    pub resources: Vec<AppResource>,
}

fn validate(bundle: &AppBundle) -> Result<(), AppsError> {
    if bundle.resources.is_empty() || bundle.resources.len() > MAX_BUNDLE_RESOURCES {
        return Err(AppsError::BundleFieldInvalid);
    }

    let mut total_bytes: usize = 0;
    let mut entry_point_found = false;
    for resource in &bundle.resources {
        if resource.path.is_empty() || resource.path.len() > MAX_RESOURCE_PATH_BYTES {
            return Err(AppsError::BundleFieldInvalid);
        }
        if resource.content_type.is_empty()
            || resource.content_type.len() > MAX_RESOURCE_CONTENT_TYPE_BYTES
        {
            return Err(AppsError::BundleFieldInvalid);
        }
        if resource.path == bundle.entry_point {
            entry_point_found = true;
        }
        // Saturation preserves the public `BundleTooLarge` outcome even if a
        // caller could somehow hold allocations whose lengths overflow usize.
        total_bytes = total_bytes.saturating_add(resource.bytes.len());
    }

    if !entry_point_found {
        return Err(AppsError::BundleFieldInvalid);
    }
    if total_bytes > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }

    Ok(())
}

/// WebRTC API identifiers whose presence in a bundle's resource bytes makes it
/// unhostable (Risk 9). A `RTCPeerConnection` opens a transport that bypasses
/// the WebView URL loader, so the runtime egress backstop cannot see it; the
/// only tractable defense at import time is to refuse the bundle. `webkit`/`moz`
/// prefixed variants are listed explicitly even though they contain
/// `RTCPeerConnection` as a substring, so the refused set is legible on its own.
pub const REFUSED_EGRESS_APIS: [&[u8]; 6] = [
    b"RTCPeerConnection",
    b"webkitRTCPeerConnection",
    b"mozRTCPeerConnection",
    b"getUserMedia",
    b"navigator.mediaDevices",
    b"RTCDataChannel",
];

/// Refuses a bundle that references any WebRTC API, scanning the **raw bytes**
/// of every resource — not its path or declared content type. Scanning bytes is
/// deliberate: the refusal must be by content, so a `.png`-named or
/// `text/plain`-typed resource that actually carries script cannot smuggle a
/// peer connection past the gate.
///
/// DENY-CLOSED: a literal substring match refuses even a mere MENTION — a
/// comment, dead code, or a vendored lib that names but never calls the API.
/// That is the intended posture (an activist tool should host no WebRTC-capable
/// code), not a bug. It is also evadable in the other direction (obfuscated
/// identifiers, dynamic construction like `window['RTC'+'PeerConnection']`), so
/// it raises the bar without being a guarantee; the runtime `WKContentRuleList`
/// backstop plus the disabled WebRTC preference remain the actual net. Enforced
/// at the single `verify_app_pair` chokepoint.
pub fn scan_bundle_egress(bundle: &AppBundle) -> Result<(), AppsError> {
    for resource in &bundle.resources {
        for api in REFUSED_EGRESS_APIS {
            if resource
                .bytes
                .windows(api.len())
                .any(|window| window == api)
            {
                return Err(AppsError::BundleUsesWebRtc);
            }
        }
    }
    Ok(())
}

/// Domain-separated digest of a bundle's canonical encoded bytes — the
/// `bundle_digest` input to `manifest::app_id_for`, following the pattern
/// in `willow/digest.rs`.
pub fn app_bundle_digest(encoded_bundle: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(APP_BUNDLE_DIGEST_DOMAIN);
    hasher.update((encoded_bundle.len() as u32).to_be_bytes());
    hasher.update(encoded_bundle);
    hasher.finalize().into()
}

/// Validates and encodes the canonical byte representation.
pub fn encode_app_bundle(bundle: &AppBundle) -> Result<Vec<u8>, AppsError> {
    validate(bundle)?;

    let buffer = encode_validated_app_bundle(bundle);
    if buffer.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }
    Ok(buffer)
}

/// Encodes a bundle that has already passed [`validate`]. `Vec<u8>`'s
/// minicbor writer is infallible; primitive CBOR writes therefore cannot
/// produce an application error.
fn encode_validated_app_bundle(bundle: &AppBundle) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let _ = e.map(2);
    let _ = e.u8(0);
    let _ = e.str(&bundle.entry_point);
    let _ = e.u8(1);
    let _ = e.array(bundle.resources.len() as u64);
    for resource in &bundle.resources {
        let _ = e.map(3);
        let _ = e.u8(0);
        let _ = e.str(&resource.path);
        let _ = e.u8(1);
        let _ = e.str(&resource.content_type);
        let _ = e.u8(2);
        let _ = e.bytes(&resource.bytes);
    }
    buffer
}

/// Strict canonical decoder: rejects unknown/duplicate/misordered keys,
/// indefinite lengths, trailing bytes, and any non-canonical encoding.
/// Bounds (resource count, path/content-type lengths) are enforced before
/// any allocation sized from untrusted input.
pub fn decode_app_bundle(input: &[u8]) -> Result<AppBundle, AppsError> {
    if input.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }

    let mut d = Decoder::new(input);
    let pairs = d
        .map()
        .map_err(|_| AppsError::BundleFieldInvalid)?
        .ok_or(AppsError::BundleFieldInvalid)?;
    if pairs != 2 {
        return Err(AppsError::BundleFieldInvalid);
    }

    let key0 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
    if key0 != 0 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let entry_point = decode_text(&mut d, MAX_RESOURCE_PATH_BYTES)?;

    let key1 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
    if key1 != 1 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let resource_count = d
        .array()
        .map_err(|_| AppsError::BundleFieldInvalid)?
        .ok_or(AppsError::BundleFieldInvalid)?;
    if resource_count == 0 || resource_count as usize > MAX_BUNDLE_RESOURCES {
        return Err(AppsError::BundleFieldInvalid);
    }

    let mut resources = Vec::with_capacity(resource_count as usize);
    for _ in 0..resource_count {
        let resource_pairs = d
            .map()
            .map_err(|_| AppsError::BundleFieldInvalid)?
            .ok_or(AppsError::BundleFieldInvalid)?;
        if resource_pairs != 3 {
            return Err(AppsError::BundleFieldInvalid);
        }

        let rkey0 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
        if rkey0 != 0 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let path = decode_text(&mut d, MAX_RESOURCE_PATH_BYTES)?;

        let rkey1 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
        if rkey1 != 1 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let content_type = decode_text(&mut d, MAX_RESOURCE_CONTENT_TYPE_BYTES)?;

        let rkey2 = d.u64().map_err(|_| AppsError::BundleFieldInvalid)?;
        if rkey2 != 2 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let bytes = decode_bytes(&mut d)?;

        resources.push(AppResource {
            path,
            content_type,
            bytes,
        });
    }

    if d.position() != input.len() {
        return Err(AppsError::BundleFieldInvalid);
    }

    let bundle = AppBundle {
        entry_point,
        resources,
    };

    validate(&bundle)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    let reencoded = encode_validated_app_bundle(&bundle);
    if reencoded != input {
        return Err(AppsError::BundleFieldInvalid);
    }

    Ok(bundle)
}

fn decode_text(d: &mut Decoder<'_>, max: usize) -> Result<String, AppsError> {
    if d.datatype().map_err(|_| AppsError::BundleFieldInvalid)? != Type::String {
        return Err(AppsError::BundleFieldInvalid);
    }
    let text = d.str().map_err(|_| AppsError::BundleFieldInvalid)?;
    if text.is_empty() || text.len() > max {
        return Err(AppsError::BundleFieldInvalid);
    }
    Ok(text.to_string())
}

fn decode_bytes(d: &mut Decoder<'_>) -> Result<Vec<u8>, AppsError> {
    if d.datatype().map_err(|_| AppsError::BundleFieldInvalid)? != Type::Bytes {
        return Err(AppsError::BundleFieldInvalid);
    }
    let bytes = d.bytes().map_err(|_| AppsError::BundleFieldInvalid)?;
    // `decode_app_bundle` rejects an input larger than this ceiling before
    // parsing, so a byte string borrowed from that input cannot exceed it.
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod egress_scan_tests {
    use super::*;

    fn bundle_with(resources: Vec<AppResource>) -> AppBundle {
        AppBundle {
            entry_point: resources[0].path.clone(),
            resources,
        }
    }

    fn resource(path: &str, content_type: &str, bytes: &[u8]) -> AppResource {
        AppResource {
            path: path.to_string(),
            content_type: content_type.to_string(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn a_clean_bundle_passes_the_egress_scan() {
        let bundle = bundle_with(vec![
            resource(
                "index.html",
                "text/html",
                b"<html><body><h1>Roll call</h1></body></html>",
            ),
            resource(
                "app.js",
                "text/javascript",
                b"document.querySelector('h1').textContent = 'ready';",
            ),
        ]);
        assert_eq!(scan_bundle_egress(&bundle), Ok(()));
    }

    #[test]
    fn every_webrtc_api_token_is_refused() {
        // Each token, on its own, in an otherwise-benign script resource, must
        // make the whole bundle unhostable.
        for api in REFUSED_EGRESS_APIS {
            let mut script = b"const x = new ".to_vec();
            script.extend_from_slice(api);
            script.extend_from_slice(b"();");
            let bundle = bundle_with(vec![
                resource("index.html", "text/html", b"<html></html>"),
                resource("app.js", "text/javascript", &script),
            ]);
            assert_eq!(
                scan_bundle_egress(&bundle),
                Err(AppsError::BundleUsesWebRtc),
                "token {:?} must be refused",
                std::str::from_utf8(api).unwrap(),
            );
        }
    }

    #[test]
    fn refusal_is_by_content_not_filename_or_content_type() {
        // The WebRTC call is hidden in a resource that lies about what it is:
        // a `.png` path with an `image/png` content type. A filename- or
        // content-type-based gate would wave it through; a byte scan does not.
        let bundle = bundle_with(vec![
            resource("index.html", "text/html", b"<html></html>"),
            resource(
                "logo.png",
                "image/png",
                b"\x89PNG\r\n new RTCPeerConnection({iceServers:[]})",
            ),
        ]);
        assert_eq!(
            scan_bundle_egress(&bundle),
            Err(AppsError::BundleUsesWebRtc),
            "a WebRTC reference disguised as an image must still be refused",
        );
    }

    #[test]
    fn a_benign_name_containing_rtc_substring_does_not_false_positive() {
        // "getUserMediaList" is not the point; the point is that a resource whose
        // BYTES never contain a refused token passes even if its NAME hints at
        // media. The gate is byte content, and clean content is hostable.
        let bundle = bundle_with(vec![resource(
            "media-gallery.html",
            "text/html",
            b"<html><body>photo gallery</body></html>",
        )]);
        assert_eq!(scan_bundle_egress(&bundle), Ok(()));
    }
}
