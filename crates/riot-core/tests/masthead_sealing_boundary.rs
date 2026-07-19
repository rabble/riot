//! The owned-masthead at-rest sealing envelope boundary.
//!
//! `OwnedMasthead::seal` wraps the owner's root secret material (namespace root
//! secret ‖ owner subspace secret) under an app-provided 32-byte key using
//! XChaCha20-Poly1305 with a fixed `[MAGIC ‖ nonce ‖ ciphertext+tag]` envelope.
//! `open_sealed` must restore ONLY when the whole envelope and the AEAD tag
//! validate, and must never return a partially constructed masthead. The inline
//! unit tests cover the happy roundtrip (and that no secret is in cleartext) plus
//! a wrong key. This suite adds the tamper/malformed-envelope boundary that a
//! secret-at-rest construction must enforce:
//!
//!   * any single-bit tamper in the tag/ciphertext or the nonce is rejected
//!     (AEAD authenticity);
//!   * a corrupted magic prefix is rejected (envelope/domain check);
//!   * a truncated or over-long blob is rejected (length guard);
//!   * every rejection returns `SealedMastheadInvalid` — a typed, fail-closed
//!     error, never a panic and never a restored masthead.
//!
//! A round-trip open of the untampered blob under the same key confirms the
//! rejections are caused by the tamper, not by an always-failing open.

use riot_core::willow::{OwnedMasthead, WillowError};

const KEY: [u8; 32] = [0x5a; 32];

fn sealed_blob() -> Vec<u8> {
    OwnedMasthead::generate()
        .expect("generate masthead")
        .seal(&KEY)
        .expect("seal")
}

/// The untampered blob opens — so any failure below is due to the tamper.
#[test]
fn untampered_blob_opens_under_the_same_key() {
    let sealed = sealed_blob();
    assert!(
        OwnedMasthead::open_sealed(&KEY, &sealed).is_ok(),
        "a clean blob must open under the sealing key (control)"
    );
}

#[test]
fn tag_tamper_is_rejected() {
    let mut sealed = sealed_blob();
    // The last byte lives in the Poly1305 tag; flipping it must fail the AEAD.
    let last = sealed.len() - 1;
    sealed[last] ^= 0xff;
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&KEY, &sealed),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "a flipped tag byte must be rejected"
    );
}

#[test]
fn ciphertext_tamper_is_rejected() {
    let mut sealed = sealed_blob();
    // A byte in the middle falls in the ciphertext body; any change breaks the tag.
    let mid = sealed.len() / 2;
    sealed[mid] ^= 0xff;
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&KEY, &sealed),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "a flipped ciphertext byte must be rejected"
    );
}

#[test]
fn nonce_tamper_is_rejected() {
    let mut sealed = sealed_blob();
    // The 8-byte magic ("RIOTMH\x01\0") is followed by the 24-byte nonce at
    // indices 8..32; perturbing a nonce byte changes the keystream so the tag no
    // longer validates (this is decryption failure, distinct from the magic check).
    sealed[10] ^= 0xff;
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&KEY, &sealed),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "a perturbed nonce must be rejected"
    );
}

#[test]
fn corrupted_magic_is_rejected() {
    let mut sealed = sealed_blob();
    // The first byte is part of the fixed magic; corrupting it fails the envelope
    // check before any decryption is attempted.
    sealed[0] ^= 0xff;
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&KEY, &sealed),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "a corrupted magic prefix must be rejected"
    );
}

#[test]
fn truncated_blob_is_rejected() {
    let sealed = sealed_blob();
    let short = &sealed[..sealed.len() - 1];
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&KEY, short),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "a truncated blob must be rejected by the length guard"
    );
    // An empty input is likewise rejected, not panicked on.
    assert!(matches!(
        OwnedMasthead::open_sealed(&KEY, &[]),
        Err(WillowError::SealedMastheadInvalid)
    ));
}

#[test]
fn over_long_blob_is_rejected() {
    let mut sealed = sealed_blob();
    sealed.push(0x00); // one trailing byte beyond the fixed envelope size
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&KEY, &sealed),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "an over-long blob must be rejected by the length guard"
    );
}
