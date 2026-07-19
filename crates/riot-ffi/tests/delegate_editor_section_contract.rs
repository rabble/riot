//! Composite-site owner FFI — `delegate_editor_section` contract.
//!
//! The site owner mints a section-scoped, time-boxed EDITOR write capability so
//! an invited editor can publish `/articles/<section>` entries. This drives the
//! willow25 delegation machinery through the FFI boundary:
//!
//!   1. a minted editor cap, used with the editor's OWN subspace secret,
//!      authorises an `/articles/<section>` entry;
//!   2. the SAME cap CANNOT author `/manifest` or `/mod/` — the `/articles/`
//!      belt holds even for a hostile `section` string;
//!   3. a wrong wrapping key fails closed (no cap, no partial result);
//!   4. an entry timestamped past the expiry is not authorised.
//!
//! TIME UNIT (load-bearing): the FFI takes `expires_unix_seconds` (product wall
//! clock) but real Willow entries are stamped in TAI/J2000 MICROSECONDS
//! (`ClockSnapshot::tai_j2000_micros`, the "join-recency view"). The cap's
//! `TimeRange` MUST be built in micros or it can never contain a real entry.
//! Every entry these tests author is therefore stamped in `tai_j2000_micros` —
//! the production unit — so a unit regression fails this suite instead of
//! passing vacuously.
//!
//! SECURITY: the returned `capability_bytes` is a public DELEGATION chain — the
//! editor uses it with THEIR own secret. No owner/root subspace secret ever
//! crosses the boundary.

use riot_core::willow::site_paths::{ARTICLES_COMPONENT, MANIFEST_COMPONENT, MOD_COMPONENT};
use riot_core::willow::{
    decode_capability_canonic, system_snapshot, ClockSnapshot, Entry, NamespaceId, Path, SubspaceId,
};
use riot_ffi::{create_owned_site, delegate_editor_section, MobileError};
use willow25::prelude::SubspaceSecret;

const MICROS_PER_SEC: u64 = 1_000_000;

/// Lowercase hex, matching the FFI's own id encoding.
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn arr32(hexstr: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = hexstr.as_bytes();
    for (i, chunk) in bytes.chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16).unwrap() as u8;
        let lo = (chunk[1] as char).to_digit(16).unwrap() as u8;
        out[i] = (hi << 4) | lo;
    }
    out
}

/// A fresh owned site + a fresh editor keypair the test fully controls.
struct Fixture {
    sealed_root: Vec<u8>,
    wrapping_key: Vec<u8>,
    namespace_id: NamespaceId,
    editor_secret: SubspaceSecret,
    editor_id: SubspaceId,
    editor_hex: String,
}

fn fixture() -> Fixture {
    let wrapping_key = vec![0x5au8; 32];
    let site = create_owned_site(wrapping_key.clone()).expect("create owned site");
    let namespace_id = NamespaceId::from_bytes(&arr32(&site.namespace_id));
    let editor_secret = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor_secret.corresponding_subspace_id();
    let editor_hex = hex(editor_id.as_bytes());
    Fixture {
        sealed_root: site.sealed_root,
        wrapping_key,
        namespace_id,
        editor_secret,
        editor_id,
        editor_hex,
    }
}

fn clock() -> ClockSnapshot {
    system_snapshot().expect("clock")
}

/// Build an entry stamped in TAI/J2000 microseconds — the SAME unit the
/// production write path uses. `micros` is the willow25 `Timestamp` value.
fn entry_at(namespace: &NamespaceId, subspace: &SubspaceId, path: &[&[u8]], micros: u64) -> Entry {
    Entry::builder()
        .namespace_id(namespace.clone())
        .subspace_id(subspace.clone())
        .path(Path::from_slices(path).expect("path"))
        .timestamp(micros)
        .payload(b"editorial body bytes")
        .build()
}

#[test]
fn minted_editor_cap_authorises_articles_section_entry() {
    let fx = fixture();
    let snap = clock();
    let invite = delegate_editor_section(
        fx.sealed_root.clone(),
        fx.wrapping_key.clone(),
        fx.editor_hex.clone(),
        "news".to_string(),
        snap.unix_seconds + 3_600,
    )
    .expect("delegation must mint");

    assert_eq!(invite.section, "news");
    assert_eq!(invite.expires_unix_seconds, snap.unix_seconds + 3_600);

    let cap = decode_capability_canonic(&invite.capability_bytes).expect("decode cap");

    // The editor, with THEIR OWN secret, authorises an /articles/news entry
    // stamped in the PRODUCTION unit (TAI/J2000 micros), well within the box.
    let good = entry_at(
        &fx.namespace_id,
        &fx.editor_id,
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        snap.tai_j2000_micros + 60 * MICROS_PER_SEC,
    );
    assert!(
        good.into_authorised_entry(&cap, &fx.editor_secret).is_ok(),
        "editor cap must authorise a real (micros-stamped) /articles/news entry"
    );

    // NO-SECRET-LEAK: what crosses the boundary is a public delegation chain —
    // the wrapping key never appears in the encoded capability, and a DIFFERENT
    // signer (someone who lacks the editor secret) cannot wield the cap.
    assert!(
        !invite
            .capability_bytes
            .windows(32)
            .any(|w| w == fx.wrapping_key.as_slice()),
        "the wrapping key must never appear in the delegated capability bytes"
    );
    let impostor = SubspaceSecret::from_bytes(&[0x99u8; 32]);
    let impostor_entry = entry_at(
        &fx.namespace_id,
        &impostor.corresponding_subspace_id(),
        &[ARTICLES_COMPONENT, b"news", b"post-2"],
        snap.tai_j2000_micros + 60 * MICROS_PER_SEC,
    );
    assert!(
        impostor_entry
            .into_authorised_entry(&cap, &impostor)
            .is_err(),
        "the delegation binds to the editor subspace; a non-editor signer is refused"
    );
}

#[test]
fn minted_editor_cap_cannot_author_manifest_or_mod() {
    let fx = fixture();
    let snap = clock();
    let invite = delegate_editor_section(
        fx.sealed_root.clone(),
        fx.wrapping_key.clone(),
        fx.editor_hex.clone(),
        "news".to_string(),
        snap.unix_seconds + 3_600,
    )
    .expect("delegation must mint");
    let cap = decode_capability_canonic(&invite.capability_bytes).expect("decode cap");

    // Entries are stamped WITHIN the time box (micros) so the ONLY reason they
    // are refused is the /articles/ belt — not an incidental time mismatch.
    let in_box = snap.tai_j2000_micros + 60 * MICROS_PER_SEC;

    // BELT: the same cap cannot reach /manifest ...
    let manifest = entry_at(
        &fx.namespace_id,
        &fx.editor_id,
        &[MANIFEST_COMPONENT],
        in_box,
    );
    assert!(
        manifest
            .into_authorised_entry(&cap, &fx.editor_secret)
            .is_err(),
        "editor cap must NOT author /manifest"
    );

    // ... nor /mod/.
    let moderation = entry_at(
        &fx.namespace_id,
        &fx.editor_id,
        &[MOD_COMPONENT, b"x"],
        in_box,
    );
    assert!(
        moderation
            .into_authorised_entry(&cap, &fx.editor_secret)
            .is_err(),
        "editor cap must NOT author /mod/"
    );
}

#[test]
fn hostile_section_string_cannot_escape_the_articles_belt() {
    let fx = fixture();
    let snap = clock();
    // A hostile section is still a single path COMPONENT under /articles/; it
    // cannot traverse up to /manifest or /mod/.
    let invite = delegate_editor_section(
        fx.sealed_root.clone(),
        fx.wrapping_key.clone(),
        fx.editor_hex.clone(),
        "../mod".to_string(),
        snap.unix_seconds + 3_600,
    )
    .expect("hostile section is still one component under /articles/");
    let cap = decode_capability_canonic(&invite.capability_bytes).expect("decode cap");

    let in_box = snap.tai_j2000_micros + 60 * MICROS_PER_SEC;

    // The cap is scoped to /articles/<"../mod">, NOT to /mod/.
    let real_mod = entry_at(
        &fx.namespace_id,
        &fx.editor_id,
        &[MOD_COMPONENT, b"x"],
        in_box,
    );
    assert!(
        real_mod
            .into_authorised_entry(&cap, &fx.editor_secret)
            .is_err(),
        "hostile section must not grant real /mod/ access"
    );
    let manifest = entry_at(
        &fx.namespace_id,
        &fx.editor_id,
        &[MANIFEST_COMPONENT],
        in_box,
    );
    assert!(
        manifest
            .into_authorised_entry(&cap, &fx.editor_secret)
            .is_err(),
        "hostile section must not grant /manifest access"
    );
}

#[test]
fn wrong_wrapping_key_fails_closed() {
    let fx = fixture();
    let snap = clock();
    let result = delegate_editor_section(
        fx.sealed_root.clone(),
        vec![0x00u8; 32], // wrong key
        fx.editor_hex.clone(),
        "news".to_string(),
        snap.unix_seconds + 3_600,
    );
    assert!(
        matches!(result, Err(MobileError::InvalidInput)),
        "wrong wrapping key must fail closed with no cap"
    );
}

#[test]
fn malformed_editor_subspace_id_is_rejected() {
    let fx = fixture();
    let snap = clock();
    let result = delegate_editor_section(
        fx.sealed_root.clone(),
        fx.wrapping_key.clone(),
        "not-hex".to_string(),
        "news".to_string(),
        snap.unix_seconds + 3_600,
    );
    assert!(
        matches!(result, Err(MobileError::InvalidInput)),
        "a non-hex / wrong-length editor id must be InvalidInput"
    );
}

#[test]
fn already_elapsed_expiry_mints_nothing() {
    let fx = fixture();
    let snap = clock();
    // expires in the past (product wall-clock seconds) => no cap, no inverted box.
    let result = delegate_editor_section(
        fx.sealed_root.clone(),
        fx.wrapping_key.clone(),
        fx.editor_hex.clone(),
        "news".to_string(),
        snap.unix_seconds.saturating_sub(1),
    );
    assert!(
        matches!(result, Err(MobileError::InvalidInput)),
        "an already-elapsed expiry must mint nothing"
    );
}

#[test]
fn entry_past_expiry_is_not_authorised() {
    let fx = fixture();
    let snap = clock();
    // A short 60-second box.
    let invite = delegate_editor_section(
        fx.sealed_root.clone(),
        fx.wrapping_key.clone(),
        fx.editor_hex.clone(),
        "news".to_string(),
        snap.unix_seconds + 60,
    )
    .expect("delegation must mint");
    let cap = decode_capability_canonic(&invite.capability_bytes).expect("decode cap");

    // An entry stamped two hours out (micros) is well past the 60-second box.
    let expired = entry_at(
        &fx.namespace_id,
        &fx.editor_id,
        &[ARTICLES_COMPONENT, b"news", b"late"],
        snap.tai_j2000_micros + 7_200 * MICROS_PER_SEC,
    );
    assert!(
        expired
            .into_authorised_entry(&cap, &fx.editor_secret)
            .is_err(),
        "an entry past the expiry must not be authorised"
    );
}
