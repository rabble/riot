//! The seeded demo space, built from committed source content.
//!
//! One builder, shared by two callers that must never disagree: the packer
//! (`examples/pack_demo_space.rs`), which writes the committed bundle, and the
//! drift guard (`tests/demo_fixture_drift.rs`), which rebuilds it and compares
//! byte for byte. If the two had separate copies of this logic, the guard would
//! only prove that a copy agrees with itself.
//!
//! **Everything here is deterministic, and that is the whole point.** The
//! bundle is committed bytes; a rebuild that differed in a single byte — a
//! clock reading, a fresh key, a map iterated in hash order — would make the
//! drift guard fail at random and teach everyone to ignore it. So:
//!
//! * every timestamp is a fixed constant read out of `content.json`; nothing
//!   here calls a clock,
//! * every signing key is derived from a fixed seed in `content.json`, so each
//!   person's subspace id — and the key tag rendered beside their name — is
//!   stable across rebuilds,
//! * entries are emitted in the order the content file lists them.
//!
//! This lives behind `conformance` because deriving an author from a raw seed
//! is exactly the raw-secret constructor that feature exists to keep out of the
//! release graph (`identity::from_parts_for_tests`). A demo fixture is a
//! conformance fixture: the release `riot-ffi` graph never enables it, and it
//! only ever *loads* the committed bytes, through the ordinary import pipeline.
//!
//! The bundle is an ordinary RIOTE1 bundle of ordinary signed entries. It is
//! admitted through `inspect → plan_all → commit` like any peer's — there is no
//! privileged seed path, which is precisely why the drift guard can prove the
//! demo works by importing it.

use std::path::PathBuf;

use serde_json::Value;
use willow25::prelude::*;

use crate::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use crate::apps::endorse::{encode_endorsement, EndorsementMarker};
use crate::apps::entry::app_data_path;
use crate::apps::index::{
    app_index_bundle_path, app_index_endorsement_path, app_index_manifest_path, verify_app_pair,
};
use crate::apps::manifest::{encode_manifest, AppManifest};
use crate::apps::starter::{verify_starter_catalog, STARTER_CATALOG};
use crate::import::bundle::encode_bundle;
use crate::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
use crate::profile::card::{encode_profile_card, ProfileCard};
use crate::profile::path::profile_card_path;
use crate::willow::clock::snapshot_from_unix_seconds;
use crate::willow::identity::{AuthorIdentity, EvidenceAuthor, NamespaceKind};
use crate::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, Entry, NamespaceId, Path,
    SignedWillowEntry,
};

/// The committed bundle's file name inside [`demo_dir`].
pub const DEMO_BUNDLE_FILE: &str = "demo-space.riot-evidence";
/// The human-editable source the bundle is built from.
pub const DEMO_CONTENT_FILE: &str = "content.json";

/// `fixtures/demo/riverside/`, resolved from this crate's manifest directory so
/// the packer and the drift test agree regardless of the working directory they
/// are run from.
pub fn demo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/demo/riverside")
}

pub fn demo_content_path() -> PathBuf {
    demo_dir().join(DEMO_CONTENT_FILE)
}

pub fn demo_bundle_path() -> PathBuf {
    demo_dir().join(DEMO_BUNDLE_FILE)
}

/// Reads `content.json` and builds the complete signed RIOTE1 bundle.
///
/// The packer writes this; the drift guard compares it against the committed
/// bytes. Calling it twice must yield identical bytes — see the module note.
pub fn build_demo_bundle_from_source() -> Result<Vec<u8>, String> {
    let path = demo_content_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let content: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    build_demo_bundle(&content)
}

/// Builds the bundle from already-parsed content. Split out from
/// [`build_demo_bundle_from_source`] so the file read is the only I/O.
pub fn build_demo_bundle(content: &Value) -> Result<Vec<u8>, String> {
    let space = field(content, "space")?;
    let namespace_id = namespace_from_seed(text(space, "namespace_secret_seed")?)?;

    // Everyone who signs anything in this space, keyed by the short id the rest
    // of the content file refers to them by. A BTreeMap, not a HashMap: nothing
    // in this builder may depend on iteration order, and the cheapest way to
    // guarantee that is for no unordered map to exist at all.
    let mut people: std::collections::BTreeMap<String, EvidenceAuthor> =
        std::collections::BTreeMap::new();
    let mut entries: Vec<SignedWillowEntry> = Vec::new();

    // --- Profile cards -----------------------------------------------------
    // Written first because every later entry is attributed to one of these
    // subspaces, and without a card the demo renders `member · <tag>` — which
    // is the honest fallback, but not the demo.
    //
    // The two endorsing GROUPS get cards too. The demo script reads their names
    // out loud ("Endorsed by Eastside Tenant Council and Courtyard Mutual Aid"),
    // and a name only exists on screen if a profile card carries it: an endorser
    // subspace with no card renders as `member · <tag>`, which would silently
    // gut Beat 2. A group here is just a subspace with a name, exactly like a
    // person — the trust story is the endorsement marker, not the card.
    for member in list(content, "members")? {
        let id = text(member, "id")?.to_string();
        let author = author_from_seed(&namespace_id, text(member, "subspace_secret_seed")?)?;
        let card = ProfileCard {
            display_name: text(member, "display_name")?.to_string(),
        };
        let payload = encode_profile_card(&card).map_err(|e| format!("encode card {id}: {e}"))?;
        let path = profile_card_path(author.subspace_id().as_bytes())
            .map_err(|e| format!("card path {id}: {e}"))?;
        entries.push(sign_at(
            &author,
            &path,
            &payload,
            willow_micros(number(member, "profile_at_unix")?)?,
        )?);
        if people.insert(id.clone(), author).is_some() {
            return Err(format!("duplicate member id '{id}' in content.json"));
        }
    }

    // --- The six alerts ----------------------------------------------------
    for alert in list(content, "alerts")? {
        let author = person(&people, text(alert, "author")?)?;
        let object_id = hex16(text(alert, "object_id")?)?;
        let revision_id = hex16(text(alert, "revision_id")?)?;
        let created_at = number(alert, "created_at_unix")?;
        let payload = AlertPayload {
            object_id,
            revision_id,
            created_at,
            valid_from: None,
            expires_at: number(alert, "expires_at_unix")?,
            language: text(alert, "language")?.to_string(),
            urgency: urgency(text(alert, "urgency")?)?,
            severity: severity(text(alert, "severity")?)?,
            certainty: certainty(text(alert, "certainty")?)?,
            headline: text(alert, "headline")?.to_string(),
            description: text(alert, "description")?.to_string(),
            affected_area_claim: None,
            source_claims: strings(alert, "source_claims")?,
            ai_assisted: false,
        };
        let payload_bytes = encode_alert(&payload).map_err(|e| format!("encode alert: {e}"))?;
        // The alert path is BOUND to the payload's own object/revision ids —
        // the import gate re-derives it and rejects any entry whose path does
        // not describe what is underneath it.
        let entry = build_alert_entry(
            author,
            &object_id,
            &revision_id,
            willow_micros(created_at)?,
            &payload_bytes,
        )
        .map_err(|e| format!("build alert entry: {e}"))?;
        entries.push(sign_entry(author, entry, &payload_bytes)?);
    }

    // --- Shift Signup: one real manifest/bundle pair ------------------------
    let app = field(content, "app")?;
    let carrier = person(&people, text(app, "carrier")?)?;
    let app_author = person(&people, text(app, "author")?)?;
    let entry_point = text(app, "entry_point")?.to_string();

    let mut resources: Vec<AppResource> = list(app, "resources")?
        .iter()
        .map(|resource| {
            Ok(AppResource {
                path: text(resource, "path")?.to_string(),
                content_type: text(resource, "content_type")?.to_string(),
                bytes: text(resource, "text")?.as_bytes().to_vec(),
            })
        })
        .collect::<Result<_, String>>()?;
    // Sorted by path, as `pack_checklist` sorts: the encoded bundle's bytes —
    // and therefore the content-derived app_id — must not depend on the order
    // someone happened to type the resources into the content file.
    resources.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    let app_bundle = AppBundle {
        entry_point: entry_point.clone(),
        resources,
    };
    let app_bundle_bytes =
        encode_app_bundle(&app_bundle).map_err(|e| format!("encode app bundle: {e}"))?;

    let manifest = AppManifest {
        name: text(app, "name")?.to_string(),
        description: text(app, "description")?.to_string(),
        version: text(app, "version")?.to_string(),
        author: AuthorIdentity {
            namespace_id: *namespace_id.as_bytes(),
            subspace_id: *app_author.subspace_id().as_bytes(),
            namespace_kind: NamespaceKind::Communal,
            signing_key_id: *app_author.subspace_id().as_bytes(),
        },
        permissions: strings(app, "permissions")?,
        entry_point,
    };
    let manifest_bytes = encode_manifest(&manifest).map_err(|e| format!("encode manifest: {e}"))?;

    // The same invariant publish, install, and scan all enforce. If the pair
    // does not verify here it would be silently dropped by the directory scan
    // on the phone, and the app would simply never appear.
    let app_id = verify_app_pair(&manifest_bytes, &app_bundle_bytes)
        .map_err(|e| format!("shift-signup pair does not verify: {e}"))?;

    let published_at = willow_micros(number(app, "published_at_unix")?)?;
    entries.push(sign_at(
        carrier,
        &app_index_manifest_path(&app_id).map_err(|e| format!("manifest path: {e}"))?,
        &manifest_bytes,
        published_at,
    )?);
    entries.push(sign_at(
        carrier,
        &app_index_bundle_path(&app_id).map_err(|e| format!("bundle path: {e}"))?,
        &app_bundle_bytes,
        published_at,
    )?);

    // --- Two endorsements ---------------------------------------------------
    // Each marker sits at the endorser's OWN slot and is signed by that
    // endorser: the import gate checks that the subspace in the path equals the
    // subspace that signed, so nobody can endorse in somebody else's name.
    for endorsement in list(content, "endorsements")? {
        let endorser_id = text(endorsement, "endorser")?;
        let endorser = person(&people, endorser_id)?;
        let marker = EndorsementMarker {
            app_id,
            note: text(endorsement, "note")?.to_string(),
            retracted: false,
        };
        let payload =
            encode_endorsement(&marker).map_err(|e| format!("encode endorsement: {e}"))?;
        let path = app_index_endorsement_path(&app_id, endorser.subspace_id().as_bytes())
            .map_err(|e| format!("endorsement path: {e}"))?;
        entries.push(sign_at(
            endorser,
            &path,
            &payload,
            willow_micros(number(endorsement, "at_unix")?)?,
        )?);
    }

    // --- The half-done checklist -------------------------------------------
    let checklist = field(content, "checklist")?;
    let checklist_app_id = checklist_app_id(text(checklist, "app_id")?)?;
    for item in list(checklist, "items")? {
        let author = person(&people, text(item, "author")?)?;
        let updated_by = person(&people, text(item, "updated_by")?)?;
        // `updated_by_id`, never a name. A stored name is a snapshot no later
        // rename can repair (commit 26e45e7); the id resolves through the
        // profile resolver at render time. The hex form is what crosses the JS
        // bridge, so it is what the app reads back.
        let value = serde_json::json!({
            "text": text(item, "text")?,
            "done": boolean(item, "done")?,
            "updated_by_id": to_hex(updated_by.subspace_id().as_bytes()),
            "updated_at": number(item, "updated_at_ms")?,
        });
        let payload = serde_json::to_vec(&value).map_err(|e| format!("encode item: {e}"))?;
        let path = app_data_path(&checklist_app_id, text(item, "key")?)
            .map_err(|e| format!("item path: {e}"))?;
        entries.push(sign_at(
            author,
            &path,
            &payload,
            willow_micros(number(item, "at_unix")?)?,
        )?);
    }

    encode_bundle(&entries).map_err(|e| format!("encode RIOTE1 bundle: {e:?}"))
}

/// The checklist's app id is CONTENT-derived, so the pin in `content.json` can
/// go stale the moment anyone repacks the checklist. Deriving it from the
/// committed starter catalog and comparing makes that a loud failure with the
/// right answer in it, instead of a demo whose checklist entries quietly land
/// under an app id nothing will ever look up.
fn checklist_app_id(pinned_hex: &str) -> Result<[u8; 32], String> {
    let pinned = hex32(pinned_hex)?;
    let derived = verify_starter_catalog(STARTER_CATALOG)
        .first()
        .map(|app| app.app_id)
        .ok_or("the starter catalog verified no apps")?;
    if derived != pinned {
        return Err(format!(
            "checklist app_id pin is stale: content.json says {pinned_hex}, the committed starter \
             catalog derives {}. Update fixtures/demo/riverside/content.json and re-pack.",
            to_hex(&derived)
        ));
    }
    Ok(pinned)
}

// ---------------------------------------------------------------------------
// Deterministic identities.
// ---------------------------------------------------------------------------

/// The demo namespace, derived from a fixed seed. A namespace id must be
/// communal (even final byte); the seed in `content.json` was chosen so it is.
fn namespace_from_seed(seed_hex: &str) -> Result<NamespaceId, String> {
    let seed = hex32(seed_hex)?;
    let namespace_id = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
    if !namespace_id.is_communal() {
        return Err(format!(
            "namespace_secret_seed {seed_hex} derives a non-communal namespace id; pick another"
        ));
    }
    Ok(namespace_id)
}

fn author_from_seed(namespace_id: &NamespaceId, seed_hex: &str) -> Result<EvidenceAuthor, String> {
    let seed = hex32(seed_hex)?;
    Ok(EvidenceAuthor::from_parts_for_tests(
        namespace_id.clone(),
        &seed,
    ))
}

fn person<'a>(
    people: &'a std::collections::BTreeMap<String, EvidenceAuthor>,
    id: &str,
) -> Result<&'a EvidenceAuthor, String> {
    people
        .get(id)
        .ok_or_else(|| format!("content.json names '{id}', who is not in `members`"))
}

// ---------------------------------------------------------------------------
// Signing.
// ---------------------------------------------------------------------------

fn sign_at(
    author: &EvidenceAuthor,
    path: &Path,
    payload: &[u8],
    willow_timestamp_micros: u64,
) -> Result<SignedWillowEntry, String> {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path.clone())
        .timestamp(willow_timestamp_micros)
        .payload(payload)
        .build();
    sign_entry(author, entry, payload)
}

fn sign_entry(
    author: &EvidenceAuthor,
    entry: Entry,
    payload: &[u8],
) -> Result<SignedWillowEntry, String> {
    let authorised = authorise_entry(author, entry).map_err(|e| format!("authorise: {e}"))?;
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    Ok(SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    })
}

/// A fixed UTC second, as the Willow (TAI/J2000 µs) timestamp an entry carries.
/// The only time source in this module — and it reads from `content.json`, not
/// from a clock.
fn willow_micros(unix_seconds: u64) -> Result<u64, String> {
    let seconds = i64::try_from(unix_seconds).map_err(|_| "timestamp out of range".to_string())?;
    Ok(snapshot_from_unix_seconds(seconds, 0)
        .map_err(|e| format!("timestamp {unix_seconds}: {e}"))?
        .tai_j2000_micros)
}

// ---------------------------------------------------------------------------
// Content-file accessors. Every one of these fails loudly and by name: a typo
// in the content file must say which key, not panic somewhere downstream.
// ---------------------------------------------------------------------------

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("content.json: missing '{key}'"))
}

fn list<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| format!("content.json: '{key}' must be an array"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("content.json: '{key}' must be a string"))
}

fn number(value: &Value, key: &str) -> Result<u64, String> {
    field(value, key)?
        .as_u64()
        .ok_or_else(|| format!("content.json: '{key}' must be a non-negative integer"))
}

fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| format!("content.json: '{key}' must be a boolean"))
}

fn strings(value: &Value, key: &str) -> Result<Vec<String>, String> {
    list(value, key)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("content.json: every '{key}' item must be a string"))
        })
        .collect()
}

fn urgency(name: &str) -> Result<Urgency, String> {
    match name {
        "immediate" => Ok(Urgency::Immediate),
        "expected" => Ok(Urgency::Expected),
        "future" => Ok(Urgency::Future),
        "past" => Ok(Urgency::Past),
        "unknown" => Ok(Urgency::Unknown),
        _ => Err(format!("content.json: unknown urgency '{name}'")),
    }
}

fn severity(name: &str) -> Result<Severity, String> {
    match name {
        "extreme" => Ok(Severity::Extreme),
        "severe" => Ok(Severity::Severe),
        "moderate" => Ok(Severity::Moderate),
        "minor" => Ok(Severity::Minor),
        "unknown" => Ok(Severity::Unknown),
        _ => Err(format!("content.json: unknown severity '{name}'")),
    }
}

fn certainty(name: &str) -> Result<Certainty, String> {
    match name {
        "observed" => Ok(Certainty::Observed),
        "likely" => Ok(Certainty::Likely),
        "possible" => Ok(Certainty::Possible),
        "unlikely" => Ok(Certainty::Unlikely),
        "unknown" => Ok(Certainty::Unknown),
        _ => Err(format!("content.json: unknown certainty '{name}'")),
    }
}

// ---------------------------------------------------------------------------
// Hex.
// ---------------------------------------------------------------------------

fn hex32(hex: &str) -> Result<[u8; 32], String> {
    let bytes = decode_hex(hex)?;
    <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| format!("expected 32 bytes (64 hex chars), got '{hex}'"))
}

fn hex16(hex: &str) -> Result<[u8; 16], String> {
    let bytes = decode_hex(hex)?;
    <[u8; 16]>::try_from(bytes.as_slice())
        .map_err(|_| format!("expected 16 bytes (32 hex chars), got '{hex}'"))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err(format!("hex string has an odd length: '{hex}'"));
    }
    hex.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = nibble(pair[0])?;
            let lo = nibble(pair[1])?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

fn nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(format!("invalid hex digit '{}'", b as char)),
    }
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
