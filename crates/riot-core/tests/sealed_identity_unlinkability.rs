//! Privacy invariants of the sealed author identity envelope.
//!
//! A sealed identity is the only form of a signer that crosses FFI and is what
//! backs an ephemeral one-off publishing identity and a sealed community. The
//! seal must be *unlinkable*: an observer holding neither wrapping key must not
//! be able to tell that two sealed blobs wrap the same author, and the blob's
//! length must not leak which author it wraps. Those properties rest entirely
//! on the fresh random nonce inside `seal_identity` — a refactor that made the
//! nonce deterministic (say, to cache seals) would silently make every seal of
//! a given author byte-identical and therefore trivially linkable. The existing
//! unit test proves the secret never appears in the blob and that it round
//! trips; it does not pin the randomization these privacy claims depend on.

use riot_core::willow::{generate_communal_author, EvidenceAuthor, WillowError};

#[test]
fn two_seals_of_one_author_under_one_key_differ_yet_both_recover_the_identity() {
    let author = generate_communal_author().expect("author");
    let key = [0x5a; 32];

    let first = author.seal_identity(&key).expect("first seal");
    let second = author.seal_identity(&key).expect("second seal");

    // The unlinkability primitive: identical author + identical key must still
    // produce non-equal ciphertext, so two seals cannot be matched by equality.
    assert_ne!(
        first, second,
        "seals of one author under one key must not be byte-identical"
    );

    // Both nonetheless decrypt to exactly the same signer.
    let from_first = EvidenceAuthor::open_sealed_identity(&key, &first).expect("open first");
    let from_second = EvidenceAuthor::open_sealed_identity(&key, &second).expect("open second");
    assert_eq!(from_first.identity(), author.identity());
    assert_eq!(from_second.identity(), author.identity());
}

#[test]
fn the_wrong_wrapping_key_is_refused_without_yielding_a_partial_author() {
    let author = generate_communal_author().expect("author");
    let key = [0x11; 32];
    let wrong_key = [0x22; 32];
    let sealed = author.seal_identity(&key).expect("seal");

    assert_eq!(
        EvidenceAuthor::open_sealed_identity(&wrong_key, &sealed).map(|a| a.identity()),
        Err(WillowError::SealedIdentityInvalid)
    );
    assert_eq!(
        EvidenceAuthor::open_sealed_identity(&key, &sealed)
            .expect("open with the right key")
            .identity(),
        author.identity()
    );
}

#[test]
fn a_tampered_seal_fails_the_authentication_tag_and_never_opens() {
    let author = generate_communal_author().expect("author");
    let key = [0x33; 32];
    let sealed = author.seal_identity(&key).expect("seal");

    // Flip a byte in the trailing ciphertext/tag region; AEAD must reject it.
    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    assert_eq!(
        EvidenceAuthor::open_sealed_identity(&key, &tampered).map(|a| a.identity()),
        Err(WillowError::SealedIdentityInvalid)
    );
}

#[test]
fn different_authors_seal_to_the_same_length_so_size_reveals_nothing() {
    let key = [0x44; 32];
    let one = generate_communal_author()
        .expect("author one")
        .seal_identity(&key)
        .expect("seal one");
    let two = generate_communal_author()
        .expect("author two")
        .seal_identity(&key)
        .expect("seal two");
    assert_eq!(
        one.len(),
        two.len(),
        "a fixed-size envelope must not let length leak the wrapped author"
    );
}
