//! Composite-site Unit 3 — Task 2: owner-signed moderation records.
//!
//! An owner signs a moderation record (Task 1 codec) as the payload of an owned
//! entry at `O:/mod/…` via `OwnedMasthead::authorise_owner_entry`; the signature
//! verifies through the real willow25 verifier. The Riot-side read guard
//! (`read_moderation_record`) refuses a moderation payload carried at any path
//! outside `/mod/` — a record body at `/articles/` or `/manifest` is not a
//! moderation record no matter who signed it.

use riot_core::site::{
    encode_moderation_record, read_moderation_record, ModerationRecord, ModerationRecordError,
    Revoke,
};
use riot_core::willow::{
    verify_entry, Entry, OwnedMasthead, Path, ARTICLES_COMPONENT, MANIFEST_COMPONENT, MOD_COMPONENT,
};

fn a_revoke() -> ModerationRecord {
    ModerationRecord::Revoke(Revoke {
        author_key: [7u8; 32],
        effective_ts: 1_234,
    })
}

fn owner_entry_at(m: &OwnedMasthead, path: &Path, payload: &[u8]) -> Entry {
    Entry::builder()
        .namespace_id(m.namespace_id().clone())
        .subspace_id(m.owner_subspace_id())
        .path(path.clone())
        .timestamp(1_000u64)
        .payload(payload)
        .build()
}

#[test]
fn owner_signs_a_mod_record_at_mod_path_and_it_verifies_and_reads_back() {
    let m = OwnedMasthead::generate().unwrap();
    let record = a_revoke();
    let payload = encode_moderation_record(&record).expect("encode");

    let path = Path::from_slices(&[MOD_COMPONENT, b"revoke", b"id-1"]).expect("path");
    let entry = owner_entry_at(&m, &path, &payload);
    let authorised = m
        .authorise_owner_entry(entry.clone())
        .expect("owner authorises a /mod/ entry");

    // The signature is real (willow25 verifier), not asserted by fiat.
    assert!(
        verify_entry(&entry, authorised.authorisation_token()),
        "owner-signed /mod/ entry must verify"
    );

    // The read guard accepts it and round-trips the record.
    assert_eq!(
        read_moderation_record(&path, &payload).expect("read"),
        record,
        "a /mod/ entry's moderation payload reads back to the signed record"
    );
}

#[test]
fn a_mod_payload_at_a_non_mod_path_is_refused_by_the_read_guard() {
    let m = OwnedMasthead::generate().unwrap();
    let payload = encode_moderation_record(&a_revoke()).expect("encode");

    // Same valid, owner-signable payload — but carried at the wrong path. The
    // owner CAN cryptographically sign anywhere (Area::full), so the guarantee
    // that "this is a moderation record" must come from the path guard, not the
    // signature.
    for bad in [
        vec![MANIFEST_COMPONENT],
        vec![ARTICLES_COMPONENT, b"news".as_slice()],
        vec![], // root
    ] {
        let path = Path::from_slices(&bad).expect("path");
        let entry = owner_entry_at(&m, &path, &payload);
        // It still signs+verifies (proving the path guard, not the sig, is what rejects).
        let authorised = m.authorise_owner_entry(entry.clone()).expect("signs");
        assert!(verify_entry(&entry, authorised.authorisation_token()));
        assert_eq!(
            read_moderation_record(&path, &payload),
            Err(ModerationRecordError::NotUnderMod),
            "a moderation payload outside /mod/ must be refused as NotUnderMod"
        );
    }
}
