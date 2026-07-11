//! Writing a profile, reading everyone's back, and the ONE sanctioned way to
//! render a name.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use willow25::groupings::Keylike;

use crate::session::{commit_at, EvidenceStore};
use crate::willow::identity::EvidenceAuthor;

use super::card::{decode_profile_card, encode_profile_card, ProfileCard};
use super::path::{classify_profile_path, profile_card_path, profile_prefix, SUBSPACE_ID_BYTES};
use super::ProfileError;

/// How many leading subspace bytes the rendered key tag shows. Four bytes —
/// eight hex characters — is short enough to read aloud and compare in
/// person, which is the only comparison that actually settles an identity.
pub const KEY_TAG_BYTES: usize = 4;

/// Writes the person's own card into their own slot. Signed and committed
/// through the same `inspect → plan_all → commit` pipeline as every other
/// entry — no privileged write path, so the import gates (canonical payload,
/// signer subspace == path slot) apply to a local write exactly as they do to
/// a synced one.
///
/// One slot per person, last-write-wins: rewriting the name means committing
/// to the same path at a strictly later Willow timestamp. An equal-or-older
/// timestamp is a stale write and is rejected.
pub fn write_profile_card(
    store: &EvidenceStore,
    author: &EvidenceAuthor,
    card: &ProfileCard,
    willow_timestamp_micros: u64,
) -> Result<(), ProfileError> {
    let payload = encode_profile_card(card)?;
    let path = profile_card_path(author.subspace_id().as_bytes())?;
    commit_at(store, author, &path, &payload, willow_timestamp_micros)
        .map_err(|_| ProfileError::StoreRejected)
}

/// Every display name this device knows: `subspace_id → display_name`. The
/// names are returned RAW — callers must pass them through
/// [`render_display_name`] before showing them to anyone.
///
/// Defense in depth: an entry whose payload fails to decode, or whose author
/// subspace does not match its path slot, is SKIPPED rather than erroring.
/// The import gates in `session.rs` already reject both, but a scan must
/// never be the thing that a single malformed entry can break — one bad card
/// must not blank out everybody else's name.
pub fn resolve_display_names(
    store: &EvidenceStore,
) -> Result<BTreeMap<[u8; SUBSPACE_ID_BYTES], String>, ProfileError> {
    let prefix = profile_prefix()?;
    let entries = store
        .entries_with_prefix(&prefix)
        .map_err(|_| ProfileError::StoreRejected)?;

    let mut names = BTreeMap::new();
    for (_id, entry, payload) in entries {
        let Some(slot_subspace) = classify_profile_path(entry.path()) else {
            continue;
        };
        if entry.subspace_id().as_bytes() != &slot_subspace {
            continue;
        }
        let Some(payload) = payload else { continue };
        let Ok(card) = decode_profile_card(&payload) else {
            continue;
        };
        names.insert(slot_subspace, card.display_name);
    }
    Ok(names)
}

/// The ONE sanctioned way to display a person. A self-claimed name is never
/// shown bare: it always carries the first [`KEY_TAG_BYTES`] bytes of its
/// subspace id as a lowercase-hex tag — `Ana · a3f91122`. Two people can both
/// claim "Ana"; their tags differ, and nothing merges them. Someone with no
/// profile renders in the SAME shape as `member · a3f91122`, so no surface
/// ever needs a second layout for the nameless.
///
/// Be clear about what this buys. The tag defeats a CASUAL impersonator — the
/// person who simply types "Ana" and hopes nobody looks. It does NOT defeat a
/// determined one: a motivated attacker can grind keypairs until one's 32-bit
/// tag matches Ana's, which is cheap. Nothing here is a signature over "I am
/// Ana"; the name is self-claimed and unverified, and so is the tag that
/// follows it.
///
/// The defenses that actually hold are elsewhere: pinning a FULL subspace id
/// (organizer lists, app trust markers) and comparing ids in person. Do not
/// mistake this tag for either of them, and do not build an authorization
/// decision on top of it.
pub fn render_display_name(name: Option<&str>, subspace_id: &[u8; SUBSPACE_ID_BYTES]) -> String {
    let mut tag = String::with_capacity(KEY_TAG_BYTES * 2);
    for byte in &subspace_id[..KEY_TAG_BYTES] {
        // Writing to a String is infallible.
        let _ = write!(tag, "{byte:02x}");
    }
    match name {
        Some(name) => format!("{name} · {tag}"),
        None => format!("member · {tag}"),
    }
}
