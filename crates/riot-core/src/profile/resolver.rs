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

/// The character that separates a name from its key tag in the rendered form.
/// A name may not contain it — see [`sanitize_display_name`].
const TAG_SEPARATOR: char = '·';

/// True for a character a name may not carry into a rendering.
///
/// Two classes, for two different attacks:
///
/// * **Unicode `Cc` (control) and `Cf` (format).** `Cf` is what matters: the
///   bidi controls, above all `U+202E` RIGHT-TO-LEFT OVERRIDE, reorder what a
///   reader SEES without changing the bytes — a name can flip the tag that
///   follows it into gibberish, or make it read as someone else's. `Cc` is
///   caught by [`char::is_control`]; `Cf` has no std predicate, so the ranges
///   are enumerated here. Enumeration is the deliberate choice: a full
///   category table would mean a new pinned dependency, and these are the
///   ranges that carry the attack.
/// * **The separator itself.** Without this, a name is free to grow its own
///   `· deadbeef`-shaped suffix and forge a tag boundary.
fn is_forbidden_in_a_name(c: char) -> bool {
    if c.is_control() {
        // Cc — NUL and the C0/C1 ranges. A control that is ALSO whitespace
        // (`\n`, `\t`) is spared here on purpose: dropping it would weld the
        // words on either side together (`"Ana\nBeatriz"` → `"AnaBeatriz"`),
        // and it costs nothing to let it fall through to the whitespace
        // collapsing below, which turns it into a single ordinary space.
        return !c.is_whitespace();
    }
    if c == TAG_SEPARATOR {
        return true;
    }
    // Cf — the ranges that can reorder or hide the rendered text.
    matches!(c,
        '\u{00ad}'                  // SOFT HYPHEN
        | '\u{061c}'                // ARABIC LETTER MARK
        | '\u{200b}'..='\u{200f}'   // ZWSP, ZWNJ, ZWJ, LRM, RLM
        | '\u{202a}'..='\u{202e}'   // LRE, RLE, PDF, LRO, RLO
        | '\u{2060}'..='\u{2064}'   // WORD JOINER and friends
        | '\u{2066}'..='\u{2069}'   // LRI, RLI, FSI, PDI
        | '\u{feff}'                // ZERO WIDTH NO-BREAK SPACE / BOM
        | '\u{fff9}'..='\u{fffb}'   // interlinear annotation
    )
}

/// The name half of a rendering, made safe to sit next to a tag.
///
/// The profile-card codec is deliberately POLICY-FREE — it checks shape and
/// bounds, never content, and the admission gates check ownership, never
/// content. So a name arrives here as an ARBITRARY string, and every rule
/// about what a name may look like ON SCREEN lives here, at the render
/// boundary, where it can be applied to a synced name and a locally typed one
/// by the same code.
///
/// The rule: drop every [`is_forbidden_in_a_name`] character, then collapse
/// runs of whitespace to a single space and trim. A name that sanitizes away
/// to nothing — `"·"`, a lone `U+202E` — renders as [`FALLBACK_DISPLAY_NAME`]
/// rather than as a blank where a name should be, because a nameless-looking
/// row is itself an impersonation surface.
///
/// Whitespace collapsing is not cosmetic: without it a stripped name can leave
/// a run of spaces that reads as a gap, and the reader's eye supplies a
/// boundary the string does not have.
pub fn sanitize_display_name(name: Option<&str>) -> String {
    let Some(name) = name else {
        return FALLBACK_DISPLAY_NAME.to_string();
    };
    let stripped: String = name
        .chars()
        .filter(|c| !is_forbidden_in_a_name(*c))
        .collect();

    let mut sanitized = String::with_capacity(stripped.len());
    for word in stripped.split_whitespace() {
        if !sanitized.is_empty() {
            sanitized.push(' ');
        }
        sanitized.push_str(word);
    }

    if sanitized.is_empty() {
        return FALLBACK_DISPLAY_NAME.to_string();
    }
    sanitized
}

/// The ONE sanctioned way to display a person. A self-claimed name is never
/// shown bare: it always carries the first [`KEY_TAG_BYTES`] bytes of its
/// subspace id as a lowercase-hex tag — `Ana · a3f91122`. Two people can both
/// claim "Ana"; their tags differ, and nothing merges them. Someone with no
/// profile renders in the SAME shape as `member · a3f91122`, so no surface
/// ever needs a second layout for the nameless.
///
/// The name goes through [`sanitize_display_name`] first, and that is load
/// bearing. Be precise about what each half buys.
///
/// **What this DOES defeat.**
///
/// * A **name-forged tag boundary**. The codec admits any string, so a name
///   may itself be `"Ana · a3f91122"` — and the naive rendering of that name
///   under the attacker's own key is `"Ana · a3f91122 · deadbeef"`, which
///   BEGINS with the exact string honest Ana renders to. Truncate it in a
///   narrow row, or just read to the first tag as every human does, and the
///   impersonation is perfect and free. Stripping the separator from the name
///   means the rendered string carries exactly one `·`, and the text after it
///   is always the key's, never the name's.
/// * **Bidi and control tricks**. A name carrying `U+202E` can visually
///   reorder the tag that follows it; NUL and other `Cc`/`Cf` characters can
///   hide or garble it. They are dropped, so what the reader sees after the
///   separator is what the key actually says.
///
/// **What this does NOT defeat.** A determined attacker who grinds keypairs
/// until one's 32-bit tag collides with Ana's — which is cheap. Nothing here
/// is a signature over "I am Ana": the name is self-claimed and unverified,
/// and so is the tag that follows it. The tag makes the impersonator do work;
/// it does not make the impersonation impossible.
///
/// The defenses that actually hold are elsewhere: pinning a FULL subspace id
/// (organizer lists, app trust markers) and comparing ids in person. Do not
/// mistake this tag for either of them, and do not build an authorization
/// decision on top of it.
pub fn render_display_name(name: Option<&str>, subspace_id: &[u8; SUBSPACE_ID_BYTES]) -> String {
    let tag = key_tag(subspace_id);
    let name = sanitize_display_name(name);
    format!("{name} {TAG_SEPARATOR} {tag}")
}

/// What a person with no profile card is called. Rendering the nameless in the
/// same `<name> · <tag>` shape as everyone else is deliberate — no surface
/// needs a second layout for them, and nobody is singled out for not having
/// picked a name.
///
/// It is also what a name that SANITIZES away to nothing becomes — see
/// [`sanitize_display_name`]. A name of `"·"` is not a name, and the row it
/// draws must still say something.
///
/// Exposed so every surface says the same word: the FFI's `WhoAmI`, which
/// carries `display_name` and `tag` as separate fields for a renderer that
/// reassembles them, gets it from [`sanitize_display_name`], which is the same
/// path [`render_display_name`] takes. A hardcoded copy could drift.
pub const FALLBACK_DISPLAY_NAME: &str = "member";

/// The key-derived tag alone: the first [`KEY_TAG_BYTES`] bytes of the
/// subspace id as lowercase hex. This is the SAME derivation
/// [`render_display_name`] appends, factored out for the one caller that needs
/// the parts separately — the FFI's `WhoAmI`, which hands `{display_name, tag}`
/// to a native/JS renderer that reassembles them.
///
/// It exists so the tag is derived in exactly one place. A second
/// implementation could drift from the rendered form and quietly show a person
/// a tag that does not match the one in their own name.
///
/// The caveats on [`render_display_name`] apply verbatim: the tag is not a
/// signature, is cheap to grind, and must not carry an authorization decision.
pub fn key_tag(subspace_id: &[u8; SUBSPACE_ID_BYTES]) -> String {
    let mut tag = String::with_capacity(KEY_TAG_BYTES * 2);
    for byte in &subspace_id[..KEY_TAG_BYTES] {
        // Writing to a String is infallible.
        let _ = write!(tag, "{byte:02x}");
    }
    tag
}
