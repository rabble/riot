//! Earthstar-style self-certifying, human-readable identity handles.
//!
//! A *handle* is a shareable string that names an identity (an author or a
//! space) by its full public key, with a decorative human label glued on the
//! front. It is the Earthstar model — `@author.<suffix>` / `+share.<suffix>` —
//! adopted as a Riot app-layer convention, not a protocol change.
//!
//! ```text
//! @ana.dez7q4nxqfkbw2tvfm3y2xrjxe4uq3jzttxhq3ydqvnek7wla
//! ^ ^^^ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//! | |  └ 52-char base32 suffix = the FULL 32-byte key (the identity)
//! | └──── shortname (decorative, non-unique, 3..=32 chars of [a-z0-9])
//! └────── sigil: @ = author (subspace key)
//!
//! +mesa.y4wm9c7b8hq3tgn2pfxd5rlbeo6vakj1smn3uhzwoxqricbate
//! ^      communal space (namespace id has an even final byte)
//! -my-site.7fq3...    owned space (namespace id has an odd final byte)
//! ```
//!
//! ## Why the suffix is the identity
//!
//! The suffix encodes the **entire 32-byte key** (⌈256/5⌉ = 52 base32 chars,
//! no padding). That makes the handle **self-certifying**: anyone who receives
//! the string alone — on a business card, in a QR code, pasted into a chat —
//! recovers the exact key with no registry, no network, no DID resolver. This
//! is the property Riot's 8-hex display tag deliberately does *not* have (32
//! bits is cheap to grind); the handle is the strong form for out-of-band
//! verification, the tag is the compact form for in-list rendering. They
//! coexist; this layer augments, it does not replace.
//!
//! ## Why the sigil for spaces matches the key byte
//!
//! Willow already encodes whether a namespace is communal or owned in the final
//! byte of the namespace id (even = communal, odd = owned). The `+`/`-` sigil
//! is just a human-readable rendering of that bit, and it is **cross-checked on
//! decode**: a `+` sigil on an odd-final-byte id is rejected. The sigil carries
//! no independent authority — it cannot lie about the kind.
//!
//! ## What the shortname is not
//!
//! The shortname is decorative. `@ana.X` and `@bob.X` are the same identity.
//! It exists so the string is recognisable to a human; it must never carry an
//! authorization decision, exactly like the 8-hex tag it sits beside.
//!
//! ## Scope
//!
//! This is a display/sharing convention only. It does not change crypto
//! (stays on ed25519 / willow25), does not replace Riot's per-community key
//! isolation, and makes no wire-level interop claim with Earthstar v11 (which
//! uses a different ciphersuite, cinn25519). The format is borrowed; the
//! identities are Riot's.

use crate::willow::NamespaceKind;
use crate::willow::WillowError;
use data_encoding::DecodeError;

/// Number of base32 characters that encode a 32-byte key: ⌈256/5⌉ = 52, with
/// no padding because 256 is not a multiple of 5.
const SUFFIX_CHARS: usize = 52;

/// Lowercase RFC 4648 base32, no padding. Used for the suffix.
const BASE32: data_encoding::Encoding = data_encoding::BASE32_NOPAD;

/// The identity handle for an author: `@<shortname>.<52-char base32 key>`.
///
/// Equality and hashing are by **subspace key only** — the shortname is
/// decorative, so two handles with the same key but different labels compare
/// equal. See [`AuthorHandle::shortname`] to recover the label.
#[derive(Clone)]
pub struct AuthorHandle {
    shortname: String,
    subspace_key: [u8; 32],
}

/// The identity handle for a space: `<sigil><shortname>.<52-char base32 id>`,
/// where the sigil is `+` for communal (even final byte) and `-` for owned
/// (odd final byte).
///
/// Equality and hashing are by **namespace id only** — the shortname is
/// decorative.
#[derive(Clone)]
pub struct SpaceHandle {
    shortname: String,
    namespace_id: [u8; 32],
    kind: NamespaceKind,
}

impl AuthorHandle {
    /// Build a handle for a subspace key. The shortname is taken as-given; it
    /// is validated only at [`AuthorHandle::parse`] time (so a caller that
    /// already has a sanitised name can skip re-validation). Use
    /// [`AuthorHandle::parse`] for untrusted input.
    pub fn for_subspace(shortname: &str, subspace_key: &[u8; 32]) -> Self {
        Self {
            shortname: shortname.to_string(),
            subspace_key: *subspace_key,
        }
    }

    /// The decorative human label. Not part of the identity.
    pub fn shortname(&self) -> &str {
        &self.shortname
    }

    /// The 32-byte subspace key this handle names — the actual identity.
    pub fn subspace_key(&self) -> &[u8; 32] {
        &self.subspace_key
    }

    /// Render the handle to its canonical lowercase string form.
    pub fn encode(&self) -> String {
        format!("@{}.{}", self.shortname, encode_suffix(&self.subspace_key))
    }

    /// Parse an `@<shortname>.<suffix>` string. The shortname is validated
    /// (3..=32 chars of `[a-z0-9]`), the suffix must decode to exactly 32
    /// bytes, and the whole string must have no trailing/leading junk.
    /// Decode is case-insensitive; encode is always lowercase.
    pub fn parse(s: &str) -> Result<Self, WillowError> {
        let (shortname, suffix) = split_author(s)?;
        validate_shortname(shortname)?;
        let subspace_key = decode_suffix(suffix)?;
        Ok(Self {
            shortname: shortname.to_string(),
            subspace_key,
        })
    }
}

impl SpaceHandle {
    /// Build a handle for a namespace id. The communal/owned kind is **derived
    /// from the key's final byte** (even = communal, odd = owned), matching
    /// Willow's own rule — the kind is never caller-asserted.
    pub fn for_namespace(shortname: &str, namespace_id: &[u8; 32]) -> Self {
        let kind = namespace_kind_of(namespace_id);
        Self {
            shortname: shortname.to_string(),
            namespace_id: *namespace_id,
            kind,
        }
    }

    /// The decorative human label. Not part of the identity.
    pub fn shortname(&self) -> &str {
        &self.shortname
    }

    /// The 32-byte namespace id this handle names — the actual identity.
    pub fn namespace_id(&self) -> &[u8; 32] {
        &self.namespace_id
    }

    /// The space kind, derived from the namespace id's final byte.
    pub fn kind(&self) -> NamespaceKind {
        self.kind
    }

    /// Render the handle to its canonical lowercase string form, with the sigil
    /// matching the namespace kind.
    pub fn encode(&self) -> String {
        let sigil = match self.kind {
            NamespaceKind::Communal => '+',
            NamespaceKind::Owned => '-',
        };
        format!(
            "{}{}.{}",
            sigil,
            self.shortname,
            encode_suffix(&self.namespace_id)
        )
    }

    /// Parse a `<sigil><shortname>.<suffix>` string. The sigil must be `+` or
    /// `-` and **must match the namespace id's final-byte parity**; a
    /// mismatched sigil is rejected as a forgery of the kind.
    pub fn parse(s: &str) -> Result<Self, WillowError> {
        let (sigil, shortname, suffix) = split_space(s)?;
        validate_shortname(shortname)?;
        let namespace_id = decode_suffix(suffix)?;
        let actual_kind = namespace_kind_of(&namespace_id);
        match (sigil, actual_kind) {
            (b'+', NamespaceKind::Communal) | (b'-', NamespaceKind::Owned) => {}
            _ => return Err(WillowError::DecodeFailed),
        }
        Ok(Self {
            shortname: shortname.to_string(),
            namespace_id,
            kind: actual_kind,
        })
    }
}

// ---------------------------------------------------------------------------
// Equality is by identity, ignoring the decorative shortname.
// ---------------------------------------------------------------------------

impl PartialEq for AuthorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.subspace_key == other.subspace_key
    }
}
impl Eq for AuthorHandle {}

impl std::hash::Hash for AuthorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.subspace_key.hash(state);
    }
}

impl PartialEq for SpaceHandle {
    fn eq(&self, other: &Self) -> bool {
        self.namespace_id == other.namespace_id
    }
}
impl Eq for SpaceHandle {}
impl std::hash::Hash for SpaceHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.namespace_id.hash(state);
    }
}

impl std::fmt::Debug for AuthorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Debug shows the encoded form, which is what a developer wants when
        // inspecting a handle. The key is the identity, so it is not redacted
        // (it is public by construction).
        f.debug_tuple("AuthorHandle").field(&self.encode()).finish()
    }
}
impl std::fmt::Debug for SpaceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SpaceHandle").field(&self.encode()).finish()
    }
}

impl std::fmt::Display for AuthorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.encode())
    }
}
impl std::fmt::Display for SpaceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.encode())
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Derive the namespace kind from the final byte, mirroring Willow's own rule
/// (even = communal, odd = owned). This keeps the sigil bound to the key, not
/// to a caller's claim.
fn namespace_kind_of(namespace_id: &[u8; 32]) -> NamespaceKind {
    if namespace_id[31] & 0x01 == 0 {
        NamespaceKind::Communal
    } else {
        NamespaceKind::Owned
    }
}

/// Encode a 32-byte key as a 52-char lowercase base32 string, no padding.
///
/// `data-encoding`'s predefined `BASE32_NOPAD` uses the uppercase RFC 4648
/// alphabet; we lowercase the output so handles read as `@ana.vov2xk5l...`.
fn encode_suffix(key: &[u8; 32]) -> String {
    BASE32.encode(key).to_lowercase()
}

/// Decode a 52-char base32 suffix into exactly 32 bytes. Decode is
/// **case-insensitive** (the encoder always emits lowercase, but we accept
/// uppercase input too); padding and non-base32 characters are rejected.
fn decode_suffix(suffix: &str) -> Result<[u8; 32], WillowError> {
    let bytes = suffix.as_bytes();
    if bytes.len() != SUFFIX_CHARS {
        return Err(WillowError::DecodeFailed);
    }
    // Reject anything outside [a-zA-Z2-7]. Padding '=' and digits 0/1/8/9 are
    // not in the base32 alphabet, so this fails closed on junk.
    if !bytes
        .iter()
        .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'2'..=b'7'))
    {
        return Err(WillowError::DecodeFailed);
    }
    // BASE32_NOPAD wants uppercase; normalise to uppercase in place on the
    // stack. This is what makes decode case-insensitive.
    let mut upper = [0u8; SUFFIX_CHARS];
    upper.copy_from_slice(bytes);
    for b in &mut upper {
        b.make_ascii_uppercase();
    }
    let mut out = [0u8; 32];
    match BASE32.decode_mut(&upper, &mut out) {
        Ok(32) => Ok(out),
        // Any decode shortcoming (wrong length class, trailing bits) is a
        // malformed handle; map it to the single DecodeFailed variant,
        // matching Riot's convention for untrusted input.
        _ => Err(WillowError::DecodeFailed),
    }
}

/// Validate a shortname: 3..=32 chars of `[a-z0-9-]`, with no leading,
/// trailing, or consecutive hyphens. Uppercase, Unicode, underscores, dots,
/// and other punctuation are rejected — the shortname is a pasteable token,
/// not a free-form display name (display names live in the profile layer with
/// their own sanitiser). Hyphens are allowed because they read naturally for
/// space names (`my-site`, `lower-east-side`) and are URL-safe.
///
/// Public so the community registry and FFI can validate a user-chosen
/// shortname before persisting it.
pub fn validate_shortname(shortname: &str) -> Result<(), WillowError> {
    let len = shortname.chars().count();
    if !(3..=32).contains(&len) {
        return Err(WillowError::DecodeFailed);
    }
    if !shortname
        .chars()
        .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-'))
    {
        return Err(WillowError::DecodeFailed);
    }
    if shortname.starts_with('-') || shortname.ends_with('-') || shortname.contains("--") {
        return Err(WillowError::DecodeFailed);
    }
    Ok(())
}

/// Split `@<shortname>.<suffix>` into its parts, rejecting anything that is
/// not exactly one `@`, a shortname, one `.`, and a suffix.
fn split_author(s: &str) -> Result<(&str, &str), WillowError> {
    let rest = s.strip_prefix('@').ok_or(WillowError::DecodeFailed)?;
    split_at_single_dot(rest)
}

/// Split `<sigil><shortname>.<suffix>` into its parts, returning the sigil
/// byte plus the two fields.
fn split_space(s: &str) -> Result<(u8, &str, &str), WillowError> {
    let (sigil, rest) = split_sigil(s)?;
    let (shortname, suffix) = split_at_single_dot(rest)?;
    Ok((sigil, shortname, suffix))
}

fn split_sigil(s: &str) -> Result<(u8, &str), WillowError> {
    // We accept exactly '+' or '-'. The byte is returned so the caller can
    // cross-check it against the decoded id's parity.
    match s.as_bytes().first() {
        Some(b'+') => Ok((b'+', &s[1..])),
        Some(b'-') => Ok((b'-', &s[1..])),
        _ => Err(WillowError::DecodeFailed),
    }
}

/// Split `<shortname>.<suffix>` on the single dot that separates them. Exactly
/// one dot is required: none is malformed, two or more leaves a dot in the
/// suffix (which then fails base32 decode). No whitespace is tolerated.
fn split_at_single_dot(rest: &str) -> Result<(&str, &str), WillowError> {
    // Reject if there is no dot, or if there is whitespace anywhere — handles
    // must be a single token with no interior or surrounding spaces.
    if rest.chars().any(char::is_whitespace) {
        return Err(WillowError::DecodeFailed);
    }
    let mut parts = rest.splitn(2, '.');
    let shortname = parts.next().unwrap_or("");
    let suffix = parts.next().ok_or(WillowError::DecodeFailed)?;
    if shortname.is_empty() || suffix.is_empty() {
        return Err(WillowError::DecodeFailed);
    }
    Ok((shortname, suffix))
}

/// Kept so the unused-import lint stays honest if `DecodeError` is ever needed
/// for richer error mapping. Currently the decoder maps every failure to the
/// single `DecodeFailed` variant, matching Riot's convention for malformed
/// untrusted input (see `share.rs` / `ticket.rs`).
#[allow(dead_code)]
type _DecodeError = DecodeError;

#[cfg(test)]
mod tests {
    use super::*;

    fn communal_ns() -> [u8; 32] {
        let mut b = [0u8; 32];
        for (i, b) in b.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(3);
        }
        b
    }

    #[test]
    fn namespace_kind_of_matches_final_byte_parity() {
        let mut communal = communal_ns();
        assert!(matches!(
            namespace_kind_of(&communal),
            NamespaceKind::Communal
        ));
        communal[31] |= 0x01;
        assert!(matches!(namespace_kind_of(&communal), NamespaceKind::Owned));
    }

    #[test]
    fn suffix_is_exactly_52_lowercase_chars() {
        let suffix = encode_suffix(&[0xab; 32]);
        assert_eq!(suffix.chars().count(), 52);
        assert!(suffix.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')));
    }

    #[test]
    fn decode_rejects_padding_and_wrong_length() {
        // 51 chars.
        let short = &encode_suffix(&[0xab; 32])[..51];
        assert!(decode_suffix(short).is_err());
        // 53 chars.
        let long = format!("{}{}", encode_suffix(&[0xab; 32]), "a");
        assert!(decode_suffix(&long).is_err());
        // With '=' padding.
        let padded = format!("{}=", encode_suffix(&[0xab; 32]));
        assert!(decode_suffix(&padded).is_err());
    }

    #[test]
    fn shortname_validation_boundaries() {
        assert!(validate_shortname("abc").is_ok());
        assert!(validate_shortname(&"a".repeat(32)).is_ok());
        assert!(validate_shortname("ab").is_err());
        assert!(validate_shortname(&"a".repeat(33)).is_err());
        assert!(validate_shortname("Ana").is_err());
        assert!(validate_shortname("a_b").is_err());
        assert!(validate_shortname("my-site").is_ok());
        assert!(validate_shortname("-mysite").is_err());
        assert!(validate_shortname("mysite-").is_err());
        assert!(validate_shortname("my--site").is_err());
    }
}
