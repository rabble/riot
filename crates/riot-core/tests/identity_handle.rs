//! Earthstar-style self-certifying identity handles.
//!
//! A handle is `@<shortname>.<52-char base32>` for an author (subspace key) and
//! `<sigil><shortname>.<52-char base32>` for a space (namespace id), where the
//! sigil is `+` for a communal space (even final byte) and `-` for an owned one
//! (odd final byte). The 52-char suffix encodes the FULL 32-byte key, so the
//! handle is verifiable from the string alone — unlike the 8-hex display tag,
//! which is decorative and grindable. The shortname is decorative too: two
//! handles with different shortnames but the same suffix are the same identity.
//!
//! This file pins the contract before the implementation: round-trips,
//! self-certification, the sigil/parity cross-check, shortname validation, and
//! the hostile-input refusals.

use riot_core::identity::{AuthorHandle, SpaceHandle};
use riot_core::willow::NamespaceKind;

/// 32 bytes → ⌈256/5⌉ = 52 base32 chars, no padding.
const SUFFIX_CHARS: usize = 52;

/// A communal-shaped namespace id (even final byte).
const COMMUNAL_NS: [u8; 32] = {
    let mut b = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        b[i] = (i as u8).wrapping_mul(7).wrapping_add(3);
        i += 1;
    }
    b
};

/// An owned-shaped namespace id (odd final byte): flip the last bit of COMMUNAL_NS.
const OWNED_NS: [u8; 32] = {
    let mut b = COMMUNAL_NS;
    b[31] |= 0x01;
    b
};

/// A 32-byte subspace key. Arbitrary; the handle does not care that it is a key.
const SUBSPACE_KEY: [u8; 32] = {
    let mut b = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        b[i] = (i as u8).wrapping_mul(11).wrapping_add(5);
        i += 1;
    }
    b
};

// ---------------------------------------------------------------------------
// Author handles
// ---------------------------------------------------------------------------

#[test]
fn an_author_handle_round_trips_and_has_a_52_char_self_certifying_suffix() {
    let handle = AuthorHandle::for_subspace("ana", &SUBSPACE_KEY);
    let encoded = handle.encode();

    assert!(
        encoded.starts_with('@'),
        "an author handle begins with @, got {encoded:?}"
    );
    let suffix = encoded.rsplit('.').next().expect("there is a suffix");
    assert_eq!(
        suffix.chars().count(),
        SUFFIX_CHARS,
        "the suffix encodes the full 32-byte key"
    );
    assert!(
        suffix.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')),
        "the suffix is lowercase RFC 4648 base32 with no padding, got {suffix:?}"
    );

    let reparsed = AuthorHandle::parse(&encoded).expect("round-trip");
    assert_eq!(reparsed, handle, "round-trip is lossless");
    assert_eq!(reparsed.subspace_key(), &SUBSPACE_KEY);
    assert_eq!(reparsed.shortname(), "ana");
}

#[test]
fn the_author_suffix_is_the_identity_so_any_mutation_changes_or_rejects_it() {
    let handle = AuthorHandle::for_subspace("ana", &SUBSPACE_KEY);
    let encoded = handle.encode();

    // Flip the last suffix char. Because 52 base32 chars encode 32.5 bytes, the
    // final char carries trailing bits that must be zero for a canonical
    // encoding — so a mutation either (a) makes the encoding non-canonical and
    // is rejected outright, or (b) decodes to a *different* key. Either way the
    // handle no longer names the original identity, which is the property that
    // matters: the suffix is bound to a specific key.
    let mut tampered: Vec<char> = encoded.chars().collect();
    let last_idx = tampered.len() - 1;
    let last = tampered[last_idx];
    let replacement = if last == 'a' { 'b' } else { 'a' };
    tampered[last_idx] = replacement;
    let tampered: String = tampered.into_iter().collect();

    match AuthorHandle::parse(&tampered) {
        Ok(reparsed) => assert_ne!(
            reparsed.subspace_key(),
            &SUBSPACE_KEY,
            "a changed suffix must name a different identity"
        ),
        Err(_) => { /* non-canonical encoding rejected — also fine */ }
    }
}

#[test]
fn an_author_shortname_is_decorative_two_names_same_suffix_are_equal() {
    let a = AuthorHandle::for_subspace("ana", &SUBSPACE_KEY);
    let b = AuthorHandle::for_subspace("bob", &SUBSPACE_KEY);
    // Same identity, different decorative label.
    assert_eq!(a, b, "identity is the suffix, not the shortname");
    assert_ne!(a.shortname(), b.shortname());
}

// ---------------------------------------------------------------------------
// Space handles: communal (+) and owned (-)
// ---------------------------------------------------------------------------

#[test]
fn a_communal_space_handle_uses_the_plus_sigil_and_round_trips() {
    let handle = SpaceHandle::for_namespace("mesa", &COMMUNAL_NS);
    assert_eq!(handle.kind(), NamespaceKind::Communal);

    let encoded = handle.encode();
    assert!(
        encoded.starts_with('+'),
        "a communal space begins with +, got {encoded:?}"
    );
    let suffix = encoded.rsplit('.').next().expect("there is a suffix");
    assert_eq!(suffix.chars().count(), SUFFIX_CHARS);

    let reparsed = SpaceHandle::parse(&encoded).expect("round-trip");
    assert_eq!(reparsed, handle);
    assert_eq!(reparsed.namespace_id(), &COMMUNAL_NS);
    assert_eq!(reparsed.kind(), NamespaceKind::Communal);
}

#[test]
fn an_owned_space_handle_uses_the_minus_sigil_and_round_trips() {
    let handle = SpaceHandle::for_namespace("my-site", &OWNED_NS);
    assert_eq!(handle.kind(), NamespaceKind::Owned);

    let encoded = handle.encode();
    assert!(
        encoded.starts_with('-'),
        "an owned space begins with -, got {encoded:?}"
    );

    let reparsed = SpaceHandle::parse(&encoded).expect("round-trip");
    assert_eq!(reparsed, handle);
    assert_eq!(reparsed.namespace_id(), &OWNED_NS);
    assert_eq!(reparsed.kind(), NamespaceKind::Owned);
}

#[test]
fn a_space_sigil_must_match_the_namespace_id_parity() {
    // A `+` (communal) sigil on an owned-shaped id (odd final byte) is a lie.
    let forged = format!(
        "+mesa.{}",
        data_encoding::BASE32_NOPAD.encode(&OWNED_NS).to_lowercase()
    );
    assert!(SpaceHandle::parse(&forged).is_err());

    // A `-` (owned) sigil on a communal-shaped id (even final byte) is a lie.
    let forged = format!(
        "-mesa.{}",
        data_encoding::BASE32_NOPAD
            .encode(&COMMUNAL_NS)
            .to_lowercase()
    );
    assert!(SpaceHandle::parse(&forged).is_err());
}

// ---------------------------------------------------------------------------
// Case handling: encode lowercase, decode case-insensitive
// ---------------------------------------------------------------------------

#[test]
fn decode_accepts_uppercase_suffix_and_encode_always_emits_lowercase() {
    let handle = AuthorHandle::for_subspace("ana", &SUBSPACE_KEY);
    let encoded = handle.encode();
    assert_eq!(
        encoded,
        encoded.to_lowercase(),
        "encode always emits lowercase"
    );

    // Only the suffix is case-insensitive; the shortname is defined as
    // lowercase, so we uppercase just the suffix to exercise that path.
    let suffix = encoded.rsplit('.').next().unwrap();
    let upper = format!("@ana.{}", suffix.to_uppercase());
    let reparsed = AuthorHandle::parse(&upper).expect("uppercase suffix accepted");
    assert_eq!(
        reparsed, handle,
        "case-insensitive decode recovers the identity"
    );
}

// ---------------------------------------------------------------------------
// Shortname validation
// ---------------------------------------------------------------------------

#[test]
fn shortname_must_be_3_to_32_chars_of_lowercase_alphanumeric() {
    // Too short.
    assert!(AuthorHandle::for_subspace("ab", &SUBSPACE_KEY)
        .encode()
        .rsplit('.')
        .next()
        .is_some()); // constructor does not validate; parse does
    assert!(AuthorHandle::parse("@ab.a").is_err());

    // Valid boundary: exactly 3.
    assert!(
        AuthorHandle::parse(&AuthorHandle::for_subspace("abc", &SUBSPACE_KEY).encode()).is_ok()
    );

    // Valid boundary: exactly 32 chars.
    let long_name = "a".repeat(32);
    assert!(
        AuthorHandle::parse(&AuthorHandle::for_subspace(&long_name, &SUBSPACE_KEY).encode())
            .is_ok()
    );

    // Too long: 33 chars.
    let too_long = "a".repeat(33);
    let encoded = AuthorHandle::for_subspace(&too_long, &SUBSPACE_KEY).encode();
    assert!(AuthorHandle::parse(&encoded).is_err());

    // Uppercase, underscores, dots, Unicode, and bare punctuation rejected.
    // (Hyphens ARE allowed — see the hyphen-constraints test below.)
    let bad_cases = ["Ana", "ana_b", "ana.b", "ana!", "中"];
    for bad in bad_cases {
        let suffix = data_encoding::BASE32_NOPAD
            .encode(&SUBSPACE_KEY)
            .to_lowercase();
        let forged = format!("@{bad}.{suffix}");
        assert!(
            AuthorHandle::parse(&forged).is_err(),
            "shortname {bad:?} should be rejected"
        );
    }
}

#[test]
fn hyphens_are_allowed_in_shortnames_but_not_leading_trailing_or_doubled() {
    // Interior single hyphens are fine.
    assert!(
        AuthorHandle::parse(&AuthorHandle::for_subspace("my-site", &SUBSPACE_KEY).encode()).is_ok()
    );
    assert!(AuthorHandle::parse(
        &AuthorHandle::for_subspace("lower-east-side", &SUBSPACE_KEY).encode()
    )
    .is_ok());

    let suffix = data_encoding::BASE32_NOPAD
        .encode(&SUBSPACE_KEY)
        .to_lowercase();
    // Leading, trailing, and consecutive hyphens rejected.
    for bad in ["-mysite", "mysite-", "my--site"] {
        assert!(
            AuthorHandle::parse(&format!("@{bad}.{suffix}")).is_err(),
            "shortname {bad:?} should be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// Hostile / malformed inputs
// ---------------------------------------------------------------------------

#[test]
fn foreign_schemes_missing_sigils_and_truncated_suffixes_are_rejected() {
    let good_suffix = data_encoding::BASE32_NOPAD
        .encode(&SUBSPACE_KEY)
        .to_lowercase();

    // No sigil at all.
    assert!(AuthorHandle::parse(&format!("ana.{good_suffix}")).is_err());
    assert!(SpaceHandle::parse(&format!("mesa.{good_suffix}")).is_err());

    // Wrong sigil for the type.
    assert!(AuthorHandle::parse(&format!("+ana.{good_suffix}")).is_err());
    assert!(AuthorHandle::parse(&format!("-ana.{good_suffix}")).is_err());
    assert!(SpaceHandle::parse(&format!("@mesa.{good_suffix}")).is_err());

    // Foreign-looking scheme.
    assert!(AuthorHandle::parse(&format!("riot://ana.{good_suffix}")).is_err());

    // No dot separator.
    assert!(AuthorHandle::parse(&format!("@{good_suffix}")).is_err());

    // Truncated suffix.
    assert!(AuthorHandle::parse(&format!("@ana.{}", &good_suffix[..10])).is_err());

    // Too-long suffix (53 chars) or padded suffix (trailing '=').
    assert!(AuthorHandle::parse(&format!("@ana.{good_suffix}x")).is_err());
    assert!(AuthorHandle::parse(&format!("@ana.{good_suffix}=")).is_err());

    // Non-base32 chars in the suffix.
    assert!(AuthorHandle::parse(&format!("@ana.{}", good_suffix.replacen('a', "1", 1))).is_err());

    // Empty string.
    assert!(AuthorHandle::parse("").is_err());
    assert!(SpaceHandle::parse("").is_err());
}

#[test]
fn a_space_handle_shortname_is_decorative_too() {
    let a = SpaceHandle::for_namespace("mesa", &COMMUNAL_NS);
    let b = SpaceHandle::for_namespace("renamed", &COMMUNAL_NS);
    assert_eq!(
        a, b,
        "a space's identity is its namespace id, not its label"
    );
}

// ---------------------------------------------------------------------------
// Self-certification end-to-end: the key recovered from the string is the input
// ---------------------------------------------------------------------------

#[test]
fn a_stranger_can_recover_the_exact_key_from_the_handle_string_alone() {
    // The whole point: nothing but the handle string is needed to verify.
    let handle = AuthorHandle::for_subspace("ana", &SUBSPACE_KEY);
    let on_a_business_card = handle.encode();

    let recovered = AuthorHandle::parse(&on_a_business_card).expect("verified from string alone");
    assert_eq!(recovered.subspace_key(), &SUBSPACE_KEY);

    let space = SpaceHandle::for_namespace("mesa", &COMMUNAL_NS);
    let space_str = space.encode();
    let recovered_space = SpaceHandle::parse(&space_str).expect("verified from string alone");
    assert_eq!(recovered_space.namespace_id(), &COMMUNAL_NS);
}
