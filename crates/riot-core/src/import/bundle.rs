//! `RiotEvidenceBundleV1` — the deliberately non-interoperable development
//! codec. Visible magic `RIOTE1`, then one deterministic CBOR document
//! framing canonical Willow component bytes. This is a pure codec layer:
//! no store, session, trust, or preview types appear here.
//!
//! Validation order (frozen): artifact size → magic → malformed/non-canonical
//! outer frame → unsupported version/codec → cumulative limits in encounter
//! order → duplicate canonical entry ID. Those reject globally. Once a
//! bounded canonical item frame is isolated, component-level failures stay
//! on that item's record and never hide valid siblings.

use minicbor::{Decoder, Encoder};

use crate::willow::{
    decode_capability_canonic, decode_entry_canonic, entry_id, evidence_digest, object_digest,
    verify_entry, william3_digest, AuthorisationToken, Entry, EntryId, NamespaceId,
    SignedWillowEntry,
};
use willow25::authorisation::WriteCapability;
use willow25::entry::{Entrylike, SubspaceSignature};
use willow25::groupings::{Keylike, Namespaced};

pub const BUNDLE_MAGIC: &[u8; 6] = b"RIOTE1";
pub const BUNDLE_CODEC_ID: &str = "org.riot.evidence-bundle/1";

/// Ceilings from fixtures/manifest.json (Revision 5 limits table).
pub const MAX_BUNDLE_BYTES: usize = 8_388_608;
pub const MAX_BUNDLE_ENTRIES: usize = 64;
pub const MAX_ITEM_PAYLOAD_BYTES: usize = 1_048_576;
/// Canonical Entry bytes per item: 4 KiB (distinct from the capability limit).
pub const MAX_ENTRY_BYTES: usize = 4_096;
/// Canonical capability bytes per item; also charged to the bundle total.
pub const MAX_CAPABILITY_BYTES: usize = 65_536;
pub const MAX_AUTH_BYTES_PER_BUNDLE: usize = 2_097_152;
const SIGNATURE_BYTES: usize = 64;
/// `willow25`'s own `Entry`/`Path` bounds (MCL=MCC=MPL=4096, hardcoded in
/// the crate) are far looser than these; nothing else in the import
/// pipeline checks path shape, so an oversized/malformed path would
/// otherwise be accepted from any validly signed entry.
pub const MAX_PATH_COMPONENTS: usize = 64;
pub const MAX_PATH_COMPONENT_BYTES: usize = 256;
pub const MAX_PATH_TOTAL_BYTES: usize = 2_048;

/// Global rejection: the artifact as a whole is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionCode {
    TooLarge,
    WrongMagic,
    MalformedFrame,
    NonCanonicalFrame,
    UnsupportedCodec,
    TooManyEntries,
    EntryBytesExceeded,
    CapabilityBytesExceeded,
    PayloadBytesExceeded,
    AuthorizationBudgetExceeded,
    DuplicateEntryId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRejection {
    pub code: RejectionCode,
    /// Sanitized static description. Never contains input bytes or payload text.
    pub detail: &'static str,
}

/// Which component of an item a diagnostic is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemComponent {
    Entry,
    Capability,
    Signature,
    Payload,
    Authorization,
    Schema,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    NonCanonicalEntry,
    NonCanonicalCapability,
    BadSignatureLength,
    PayloadLengthMismatch,
    PayloadDigestMismatch,
    DoesNotAuthorise,
    UnsupportedCapability,
    UnsupportedSchema,
    PathBoundsExceeded,
}

/// Sanitized, item-scoped diagnostic: code + component only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleDiagnostic {
    pub code: DiagnosticCode,
    pub component: ItemComponent,
}

/// Raw component bytes of one framed item.
///
/// `Debug` is redacted: it prints only field lengths, never the bytes, so
/// formatting a decoded outcome cannot leak attacker-controlled payload bytes
/// into logs. Every container that holds a frame inherits this redaction.
#[derive(Clone, PartialEq, Eq)]
pub struct BundleItemFrame {
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
    payload_bytes: Vec<u8>,
}

impl std::fmt::Debug for BundleItemFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BundleItemFrame")
            .field("entry_bytes.len", &self.entry_bytes.len())
            .field("capability_bytes.len", &self.capability_bytes.len())
            .field("signature_bytes.len", &self.signature_bytes.len())
            .field("payload_bytes.len", &self.payload_bytes.len())
            .finish()
    }
}

impl BundleItemFrame {
    pub fn entry_bytes(&self) -> &[u8] {
        &self.entry_bytes
    }
    pub fn capability_bytes(&self) -> &[u8] {
        &self.capability_bytes
    }
    pub fn signature_bytes(&self) -> &[u8] {
        &self.signature_bytes
    }
    pub fn payload_bytes(&self) -> &[u8] {
        &self.payload_bytes
    }
}

/// A fully verified item: canonical entry plus its digest identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidItem {
    pub entry: Entry,
    pub entry_id: EntryId,
    pub evidence_digest: [u8; 32],
    pub object_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Valid(Box<ValidItem>),
    Invalid(BundleDiagnostic),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedItem {
    pub frame: BundleItemFrame,
    pub status: ItemStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedBundle {
    pub items: Vec<DecodedItem>,
    pub bundle_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum BundleDecodeOutcome {
    Decoded(DecodedBundle),
    Rejected(BundleRejection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleEncodeError {
    TooManyEntries,
    BundleTooLarge,
    AuthorizationBudgetExceeded,
    InvalidItem(BundleDiagnostic),
}

/// Validating export: every item is re-verified so Riot never exports bytes
/// it would itself reject.
pub fn encode_bundle(items: &[SignedWillowEntry]) -> Result<Vec<u8>, BundleEncodeError> {
    if items.len() > MAX_BUNDLE_ENTRIES {
        return Err(BundleEncodeError::TooManyEntries);
    }
    let mut auth_total = 0usize;
    for item in items {
        auth_total += item.capability_bytes.len() + item.signature.len();
        if auth_total > MAX_AUTH_BYTES_PER_BUNDLE {
            return Err(BundleEncodeError::AuthorizationBudgetExceeded);
        }
        let frame = BundleItemFrame {
            entry_bytes: item.entry_bytes.clone(),
            capability_bytes: item.capability_bytes.clone(),
            signature_bytes: item.signature.to_vec(),
            payload_bytes: item.payload_bytes.clone(),
        };
        // Producer-side self-consistency check: an owned item must be rooted at
        // its OWN namespace (a malformed entry surfaces its own diagnostic in
        // `verify_frame`). This is not admission — the follow binding is
        // enforced on the receiving side by `decode_bundle_with_root`.
        let item_root = decode_entry_canonic(&item.entry_bytes)
            .ok()
            .map(|entry| *entry.namespace_id().as_bytes());
        if let Err(diagnostic) = verify_frame(&frame, item_root.as_ref()) {
            return Err(BundleEncodeError::InvalidItem(diagnostic));
        }
    }
    let frames: Vec<BundleItemFrame> = items
        .iter()
        .map(|item| BundleItemFrame {
            entry_bytes: item.entry_bytes.clone(),
            capability_bytes: item.capability_bytes.clone(),
            signature_bytes: item.signature.to_vec(),
            payload_bytes: item.payload_bytes.clone(),
        })
        .collect();
    let bytes = frame_bundle(&frames);
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(BundleEncodeError::BundleTooLarge);
    }
    Ok(bytes)
}

/// Deterministic outer framing of already-validated frames (production
/// codec id).
fn frame_bundle(frames: &[BundleItemFrame]) -> Vec<u8> {
    frame_bundle_with_codec(BUNDLE_CODEC_ID, frames)
}

/// Frames with an arbitrary codec string. Used both for production framing
/// and to prove input canonicality *independent of the codec value* — we
/// re-encode with exactly the codec string the input carried, so a canonical
/// document with a wrong codec re-encodes identically and is judged
/// non-canonical only when its bytes truly differ.
fn frame_bundle_with_codec(codec: &str, frames: &[BundleItemFrame]) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut buffer);
    let _ = e.map(2);
    let _ = e.u8(0);
    let _ = e.str(codec);
    let _ = e.u8(1);
    let _ = e.array(frames.len() as u64);
    for frame in frames {
        let _ = e.map(4);
        let _ = e.u8(0);
        let _ = e.bytes(&frame.entry_bytes);
        let _ = e.u8(1);
        let _ = e.bytes(&frame.capability_bytes);
        let _ = e.u8(2);
        let _ = e.bytes(&frame.signature_bytes);
        let _ = e.u8(3);
        let _ = e.bytes(&frame.payload_bytes);
    }
    buffer
}

fn reject(code: RejectionCode, detail: &'static str) -> BundleDecodeOutcome {
    BundleDecodeOutcome::Rejected(BundleRejection { code, detail })
}

/// A structurally parsed outer document, before any semantic judgement.
struct RawOuter {
    codec: String,
    frames: Vec<BundleItemFrame>,
}

/// Strict bounded decode. Frozen fatal precedence, each winning over the
/// next: size → magic → malformed/non-canonical outer frame → unsupported
/// codec → cumulative limits (entry count, per-field ceilings, authorization
/// budget) in encounter order → duplicate entry ID. Only after all global
/// gates pass are items verified independently, with siblings isolated.
/// Decode with NO followed-site root. Owned-namespace entries fail closed:
/// used by every non-admission inspector (CLI pack, FFI listing surfaces) that
/// has no site-follow context. Exactly `decode_bundle_with_root(input, None)`.
pub fn decode_bundle(input: &[u8]) -> BundleDecodeOutcome {
    decode_bundle_with_root(input, None)
}

/// Decode for an admission gate that knows which owned site the caller follows.
///
/// `followed_site_root` is the owned-namespace root (its 32-byte id) the caller
/// is importing/syncing FOR. An owned-namespace editorial entry is admitted
/// only when its capability is owned and rooted at exactly this key (see
/// [`admissible_capability`]); `None` fails closed. Communal admission ignores
/// this argument and is byte-for-byte unchanged.
pub fn decode_bundle_with_root(
    input: &[u8],
    followed_site_root: Option<[u8; 32]>,
) -> BundleDecodeOutcome {
    // 1. Size, before any parsing (bounds all later reads to <= 8 MiB).
    if input.len() > MAX_BUNDLE_BYTES {
        return reject(RejectionCode::TooLarge, "artifact exceeds 8 MiB ceiling");
    }
    // 2. Magic.
    if input.len() < BUNDLE_MAGIC.len() || &input[..BUNDLE_MAGIC.len()] != BUNDLE_MAGIC {
        return reject(RejectionCode::WrongMagic, "missing RIOTE1 magic");
    }
    let body = &input[BUNDLE_MAGIC.len()..];

    // 3. Structural parse, reading the codec string as opaque data.
    let raw = match parse_outer_structure(body) {
        Ok(raw) => raw,
        Err(outcome) => return outcome,
    };
    // 3b. Canonicality, judged independent of the codec value: re-encode with
    // the exact codec string the input carried.
    let reframed = frame_bundle_with_codec(&raw.codec, &raw.frames);
    if reframed[BUNDLE_MAGIC.len()..] != *body {
        return reject(
            RejectionCode::NonCanonicalFrame,
            "outer frame is not the canonical encoding",
        );
    }

    // 4. Codec value.
    if raw.codec != BUNDLE_CODEC_ID {
        return reject(
            RejectionCode::UnsupportedCodec,
            "unknown codec id or version",
        );
    }

    // 5. Cumulative limits in encounter order.
    if raw.frames.len() > MAX_BUNDLE_ENTRIES {
        return reject(RejectionCode::TooManyEntries, "more than 64 entries");
    }
    let mut auth_total = 0usize;
    for frame in &raw.frames {
        if frame.entry_bytes.len() > MAX_ENTRY_BYTES {
            return reject(
                RejectionCode::EntryBytesExceeded,
                "canonical entry bytes exceed the 4 KiB ceiling",
            );
        }
        if frame.capability_bytes.len() > MAX_CAPABILITY_BYTES {
            return reject(
                RejectionCode::CapabilityBytesExceeded,
                "capability bytes exceed the 64 KiB ceiling",
            );
        }
        if frame.payload_bytes.len() > MAX_ITEM_PAYLOAD_BYTES {
            return reject(
                RejectionCode::PayloadBytesExceeded,
                "payload bytes exceed the 1 MiB ceiling",
            );
        }
        auth_total += frame.capability_bytes.len() + frame.signature_bytes.len();
        if auth_total > MAX_AUTH_BYTES_PER_BUNDLE {
            return reject(
                RejectionCode::AuthorizationBudgetExceeded,
                "cumulative authorization bytes exceed the bundle budget",
            );
        }
    }

    // 6. Duplicate canonical entry IDs reject globally.
    let mut seen: Vec<EntryId> = Vec::with_capacity(raw.frames.len());
    for frame in &raw.frames {
        let id = entry_id(&frame.entry_bytes);
        if seen.contains(&id) {
            return reject(
                RejectionCode::DuplicateEntryId,
                "artifact repeats a canonical entry",
            );
        }
        seen.push(id);
    }

    // Stage two: independent item verification; siblings stay isolated.
    let items = raw
        .frames
        .into_iter()
        .map(|frame| {
            let status = match verify_frame(&frame, followed_site_root.as_ref()) {
                Ok(valid) => ItemStatus::Valid(Box::new(valid)),
                Err(diagnostic) => ItemStatus::Invalid(diagnostic),
            };
            DecodedItem { frame, status }
        })
        .collect();

    BundleDecodeOutcome::Decoded(DecodedBundle {
        items,
        bundle_digest: crate::willow::bundle_digest(input),
    })
}

/// Structural CBOR parse into raw frames. Enforces well-formedness (definite
/// lengths, expected keys in order, no trailing bytes) but makes no semantic
/// codec/limit judgement; those are the caller's ordered gates. Reads are
/// bounded by the <= 8 MiB artifact size checked before this runs.
fn parse_outer_structure(body: &[u8]) -> Result<RawOuter, BundleDecodeOutcome> {
    let mut d = Decoder::new(body);
    let malformed =
        |detail: &'static str| Err::<RawOuter, _>(reject(RejectionCode::MalformedFrame, detail));

    let Ok(Some(pairs)) = d.map() else {
        return malformed("outer document is not a definite map");
    };
    if pairs != 2 {
        return malformed("outer map must have exactly two pairs");
    }
    if d.u8().ok() != Some(0) {
        return malformed("first outer key must be 0");
    }
    let Ok(codec) = d.str() else {
        return malformed("codec id must be a definite text string");
    };
    let codec = codec.to_string();
    if d.u8().ok() != Some(1) {
        return malformed("second outer key must be 1");
    }
    let Ok(Some(count)) = d.array() else {
        return malformed("items must be a definite array");
    };
    // Structural sanity only: a count beyond the artifact's own byte budget
    // cannot be honestly framed. The exact 64-entry limit is a later gate.
    if count > MAX_BUNDLE_BYTES as u64 {
        return malformed("item count exceeds the artifact byte budget");
    }

    let mut frames = Vec::with_capacity((count as usize).min(MAX_BUNDLE_ENTRIES + 1));
    for _ in 0..count {
        let Ok(Some(inner)) = d.map() else {
            return malformed("item must be a definite map");
        };
        if inner != 4 {
            return malformed("item map must have exactly four pairs");
        }
        let entry_bytes = read_bytes_field(&mut d, 0)?;
        let capability_bytes = read_bytes_field(&mut d, 1)?;
        let signature_bytes = read_bytes_field(&mut d, 2)?;
        let payload_bytes = read_bytes_field(&mut d, 3)?;
        frames.push(BundleItemFrame {
            entry_bytes,
            capability_bytes,
            signature_bytes,
            payload_bytes,
        });
    }

    if d.position() != body.len() {
        return malformed("trailing bytes after the outer document");
    }
    Ok(RawOuter { codec, frames })
}

fn read_bytes_field(d: &mut Decoder<'_>, expected_key: u8) -> Result<Vec<u8>, BundleDecodeOutcome> {
    if d.u8().ok() != Some(expected_key) {
        return Err(reject(
            RejectionCode::MalformedFrame,
            "item keys must be 0..=3 in order",
        ));
    }
    // `bytes()` requires a definite-length byte string; indefinite/chunked
    // strings and wrong major types are malformed.
    let Ok(bytes) = d.bytes() else {
        return Err(reject(
            RejectionCode::MalformedFrame,
            "item field must be a definite byte string",
        ));
    };
    Ok(bytes.to_vec())
}

/// The owned-vs-communal admission decision, shared by every admission gate so
/// they cannot drift. This is the AUTH-POLICY layer ONLY — it does NOT run the
/// willow25 cryptographic chain check; the caller runs `verify_entry` /
/// `does_authorise` afterwards. Returns `true` iff `capability` is admissible
/// for an entry in `entry_namespace` under `followed_site_root`.
///
/// Owned namespaces REQUIRE an explicit `capability.is_owned()`: a *communal*
/// genesis cap is unconditionally `is_valid()` and can NAME an owned namespace
/// id (`NamespaceId::is_owned()` is only the LSB marker bit, not bound to the
/// cap's genesis variant), so without this an attacker forges masthead writes
/// with a communal cap pointed at the owned id. The followed root then binds
/// the entry to the exact site the user follows — `None` fails closed, and a
/// different owned root is a different site, never silently this one. The
/// communal branch is unchanged: a zero-delegation communal cap only.
pub(crate) fn admissible_capability(
    capability: &WriteCapability,
    entry_namespace: &NamespaceId,
    followed_site_root: Option<&[u8; 32]>,
) -> bool {
    if entry_namespace.is_owned() {
        if !capability.is_owned() {
            return false;
        }
        let Some(root) = followed_site_root else {
            return false;
        };
        entry_namespace.as_bytes() == root
            && capability.genesis().namespace_key().as_bytes() == root
    } else {
        !capability.is_owned()
            && capability.delegations().is_empty()
            && entry_namespace.is_communal()
    }
}

/// Full component verification of one isolated frame, in the frozen order:
/// canonical Entry → canonical WriteCapability → 64-byte signature →
/// payload length/corrected WILLIAM3 → admission policy
/// ([`admissible_capability`]) → Meadowcap authorization → reserved typed
/// schema or Riot alert schema. `followed_site_root` binds owned-namespace
/// admission to the followed site; `None` fails owned entries closed.
fn verify_frame(
    frame: &BundleItemFrame,
    followed_site_root: Option<&[u8; 32]>,
) -> Result<ValidItem, BundleDiagnostic> {
    let entry = decode_entry_canonic(&frame.entry_bytes).map_err(|_| BundleDiagnostic {
        code: DiagnosticCode::NonCanonicalEntry,
        component: ItemComponent::Entry,
    })?;

    // willow25's own Path bounds (MCL=MCC=MPL=4096) are far looser than
    // riot-core's; check shape explicitly rather than trust the library's
    // wider defaults.
    let path = entry.path();
    let path_bounds_ok = path.component_count() <= MAX_PATH_COMPONENTS
        && path.total_length() <= MAX_PATH_TOTAL_BYTES
        && path
            .components()
            .all(|component| component.len() <= MAX_PATH_COMPONENT_BYTES);
    if !path_bounds_ok {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::PathBoundsExceeded,
            component: ItemComponent::Entry,
        });
    }

    let capability =
        decode_capability_canonic(&frame.capability_bytes).map_err(|_| BundleDiagnostic {
            code: DiagnosticCode::NonCanonicalCapability,
            component: ItemComponent::Capability,
        })?;
    let signature_array: [u8; SIGNATURE_BYTES] = frame
        .signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| BundleDiagnostic {
            code: DiagnosticCode::BadSignatureLength,
            component: ItemComponent::Signature,
        })?;

    if entry.payload_length() != frame.payload_bytes.len() as u64 {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::PayloadLengthMismatch,
            component: ItemComponent::Payload,
        });
    }
    if *entry.payload_digest().as_bytes() != william3_digest(&frame.payload_bytes) {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::PayloadDigestMismatch,
            component: ItemComponent::Payload,
        });
    }

    // Admission policy (the single chokepoint): a communal namespace still
    // requires a zero-delegation communal cap for the entry's own subspace; an
    // owned (composite-site) namespace requires an owned cap rooted at the
    // followed site. `admissible_capability` is the shared predicate every
    // gate routes through so they can never diverge.
    if !admissible_capability(&capability, entry.namespace_id(), followed_site_root) {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::UnsupportedCapability,
            component: ItemComponent::Authorization,
        });
    }

    let token = AuthorisationToken::new(capability, SubspaceSignature::from(signature_array));
    if !verify_entry(&entry, &token) {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::DoesNotAuthorise,
            component: ItemComponent::Authorization,
        });
    }

    // Schema: an alert-path entry's payload must be exactly one canonical
    // Riot alert. App-data paths (`apps/<app_id>/...`, shape defined by
    // `apps::entry::is_app_data_path`) instead carry opaque app payloads:
    // integrity is covered by the digest/length checks above, size by
    // `MAX_ITEM_PAYLOAD_BYTES`, and the payload embeds no identity for the
    // path to bind, so no payload schema applies to them. App-index paths
    // (shape defined by `apps::index::classify_app_index_path`) carry
    // decodable payloads, so each slot gets its strict canonical decoder;
    // an endorsement payload must additionally name the same app as its
    // path — the marker's `app_id` is signed content, and letting it drift
    // from the slot would let one signed marker be replayed under a
    // different app's directory row. (Endorser/organizer-slot ownership is
    // bound at `inspect`, where the entry's subspace is compared against the
    // path identity component.) Profile paths
    // (`profile/<32-byte subspace>/card`, shape defined by
    // `profile::path::classify_profile_path`) must carry a canonical
    // `ProfileCard`; card-slot ownership is likewise bound at `inspect`.
    //
    // `app-index/`, `profile/`, `newswire/v1`, and `coordinate/v1` are RESERVED
    // prefixes: a path under one that is not exactly a recognized slot shape is
    // UnsupportedSchema outright and must never fall through to the alert
    // schema check below — otherwise a valid alert payload could rescue a
    // malformed reserved path.
    // Admission stays policy-free: shape and schema only (and slot ownership
    // at `inspect`), never whether a name/marker is "allowed".
    // Everything else is UnsupportedSchema.
    let schema_ok = if entry.namespace_id().is_owned() {
        // Owned composite-site namespace, OPAQUE payload — integrity is the
        // digest/length checks above and the path is the identity (mirrors
        // app-data); the record schema is validated read-side (moderation.rs
        // `read_moderation_record`, manifest on its own path). Admitted regions:
        //   `/articles/` (Unit 1, editor-delegatable) and `/mod/` (Unit 3,
        //   owner + `/mod/`-scoped moderator caps).
        // The reserved `/manifest` carries no schema here and is refused (Unit 2
        // validates it on an independent path). A `/mod/` entry authored under an
        // `/articles/`-scoped editor cap is refused UPSTREAM by willow25
        // `does_authorise` (the editor area does not include `/mod/`), never
        // reaching this schema check. Admission already required a cap rooted at
        // the followed site above, so nothing communal reaches this branch.
        crate::willow::site_paths::is_under_articles(entry.path())
            || crate::willow::site_paths::is_under_mod(entry.path())
    } else if crate::apps::entry::is_app_data_path(entry.path()) {
        true
    } else {
        match crate::apps::index::classify_app_index_path(entry.path()) {
            Some(crate::apps::index::AppIndexSlot::Manifest { .. }) => {
                crate::apps::manifest::decode_manifest(&frame.payload_bytes).is_ok()
            }
            Some(crate::apps::index::AppIndexSlot::Bundle { .. }) => {
                crate::apps::bundle::decode_app_bundle(&frame.payload_bytes).is_ok()
            }
            Some(crate::apps::index::AppIndexSlot::Endorsement { app_id, .. }) => {
                crate::apps::endorse::decode_endorsement(&frame.payload_bytes)
                    .map(|marker| marker.app_id == app_id)
                    .unwrap_or(false)
            }
            Some(crate::apps::index::AppIndexSlot::Trust { app_id, .. }) => {
                crate::apps::trust::decode_trust_marker(&frame.payload_bytes)
                    .map(|marker| marker.app_id == app_id)
                    .unwrap_or(false)
            }
            None => {
                if crate::newswire::is_newswire_prefix(entry.path()) {
                    crate::newswire::inspect_verified_components(&entry, &frame.payload_bytes)
                        .is_ok()
                } else if crate::coordinate::is_coordinate_prefix(entry.path()) {
                    // `coordinate/v1` is a RESERVED prefix admitted through the
                    // same communal structural gate as newswire: a malformed
                    // Coordinate path returns false → UnsupportedSchema, and can
                    // never fall through to the alert schema check below.
                    crate::coordinate::inspect_verified_components(&entry, &frame.payload_bytes)
                        .is_ok()
                } else {
                    // Reaching here under `apps/` or `app-index/` means the
                    // path is malformed for its own family (a valid one would
                    // have been claimed above). Both are RESERVED prefixes,
                    // so refuse them outright — otherwise a valid alert
                    // payload rescues a malformed reserved path and lands an
                    // "alert" at a path no alert can own.
                    let is_malformed_reserved_path =
                        entry.path().components().next().is_some_and(|component| {
                            let component = component.as_ref();
                            component == crate::apps::index::APP_INDEX_COMPONENT
                                || component == crate::apps::entry::APPS_COMPONENT
                        });
                    if is_malformed_reserved_path {
                        false
                    } else if crate::profile::path::is_profile_prefixed(entry.path()) {
                        // Reserved prefix: only the exact card slot carrying a
                        // canonical card payload is admissible. A malformed
                        // profile path can NEVER fall through to the alert
                        // schema below.
                        crate::profile::path::classify_profile_path(entry.path()).is_some()
                            && crate::profile::card::decode_profile_card(&frame.payload_bytes)
                                .is_ok()
                    } else {
                        crate::model::decode_alert(&frame.payload_bytes).is_ok()
                    }
                }
            }
        }
    };
    if !schema_ok {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::UnsupportedSchema,
            component: ItemComponent::Schema,
        });
    }

    Ok(ValidItem {
        entry_id: entry_id(&frame.entry_bytes),
        evidence_digest: evidence_digest(
            &frame.entry_bytes,
            &frame.capability_bytes,
            &signature_array,
        ),
        object_digest: object_digest(&frame.payload_bytes),
        entry,
    })
}
