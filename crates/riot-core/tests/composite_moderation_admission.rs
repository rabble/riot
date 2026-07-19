//! Composite-site Unit 3 — Task 4: admission of `/mod/` moderation records.
//!
//! Unit 1 opened the owned-namespace admission gate for `/articles/` only. Unit 3
//! extends the owned schema gate (`import/bundle.rs`) to also admit `/mod/`
//! records — authored by the site owner (`Area::full` owned cap) OR a
//! `/mod/`-scoped moderator cap. The reserved `/manifest` region stays refused at
//! this gate (Unit 2 validates the manifest on its own path).
//!
//! Cross-scope forgeries (an `/articles/`-scoped editor cap authoring a `/mod/`
//! entry, or a `/mod/` moderator cap authoring `/manifest`) are refused UPSTREAM
//! by willow25 area nesting — the friendly assembly API here refuses to even
//! produce such bytes (`into_authorised_entry` panics), and the mint-side
//! containment is proven in `willow/masthead.rs`
//! (`delegated_moderator_can_write_mod_but_not_manifest_or_root`). This file
//! proves the SCHEMA-gate change: `/mod/` now passes, `/manifest` still does not.

use minicbor::Encoder;
use riot_core::import::{
    decode_bundle_with_root, BundleDecodeOutcome, DiagnosticCode, ItemStatus, BUNDLE_CODEC_ID,
    BUNDLE_MAGIC,
};
use riot_core::willow::site_paths::MOD_COMPONENT;
use riot_core::willow::{
    encode_capability, encode_entry, Entry, NamespaceId, Path, SignedWillowEntry, SubspaceId,
    MANIFEST_COMPONENT,
};
use willow25::prelude::{Area, NamespaceSecret, SubspaceSecret, TimeRange, WriteCapability};

fn full_time_range() -> TimeRange {
    TimeRange::new(0u64.into(), Some(u64::MAX.into()))
}

fn build_entry(namespace: NamespaceId, subspace: SubspaceId, path: Path, payload: &[u8]) -> Entry {
    Entry::builder()
        .namespace_id(namespace)
        .subspace_id(subspace)
        .path(path)
        .timestamp(1_000u64)
        .payload(payload)
        .build()
}

/// Assemble the canonical component bytes from a CHECKED authorised triple.
/// Panics if the cap does not authorise the entry (so a "valid" helper can never
/// silently produce unauthorised bytes — the cross-scope cases can't be faked).
fn sign_into(
    entry: Entry,
    cap: &WriteCapability,
    secret: &SubspaceSecret,
    payload: &[u8],
) -> SignedWillowEntry {
    let authorised = entry
        .into_authorised_entry(cap, secret)
        .expect("entry must be authorised by the capability");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

struct OwnedSite {
    root: [u8; 32],
    namespace_id: NamespaceId,
    owner_secret: SubspaceSecret,
    owner_cap: WriteCapability,
}

fn manual_owned_site(namespace_seed: u8, owner_seed: u8) -> OwnedSite {
    let mut seed = [namespace_seed; 32];
    let namespace_secret = loop {
        let candidate = NamespaceSecret::from_bytes(&seed);
        if candidate.corresponding_namespace_id().is_owned() {
            break candidate;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    let namespace_id = namespace_secret.corresponding_namespace_id();
    let owner_secret = SubspaceSecret::from_bytes(&[owner_seed; 32]);
    let owner_id = owner_secret.corresponding_subspace_id();
    let owner_cap = WriteCapability::new_owned(&namespace_secret, owner_id);
    OwnedSite {
        root: *namespace_id.as_bytes(),
        namespace_id,
        owner_secret,
        owner_cap,
    }
}

fn frame_one(item: &SignedWillowEntry) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(2)?;
        e.u8(0)?.str(BUNDLE_CODEC_ID)?;
        e.u8(1)?.array(1)?;
        e.map(4)?;
        e.u8(0)?.bytes(&item.entry_bytes)?;
        e.u8(1)?.bytes(&item.capability_bytes)?;
        e.u8(2)?.bytes(&item.signature)?;
        e.u8(3)?.bytes(&item.payload_bytes)?;
        Ok(())
    })();
    r.expect("framing");
    buffer
}

fn status_with_root(item: &SignedWillowEntry, root: Option<[u8; 32]>) -> ItemStatus {
    let bytes = frame_one(item);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle_with_root(&bytes, root) else {
        panic!("item-level failures must not reject the whole artifact");
    };
    decoded.items.into_iter().next().expect("one item").status
}

fn assert_admitted(item: &SignedWillowEntry, root: [u8; 32]) {
    match status_with_root(item, Some(root)) {
        ItemStatus::Valid(_) => {}
        ItemStatus::Invalid(d) => panic!("expected admission, got {:?}/{:?}", d.code, d.component),
    }
}

fn assert_rejected_code(item: &SignedWillowEntry, root: [u8; 32], code: DiagnosticCode) {
    match status_with_root(item, Some(root)) {
        ItemStatus::Invalid(d) => assert_eq!(d.code, code, "wrong rejection stage"),
        ItemStatus::Valid(_) => panic!("expected rejection {code:?}, item was admitted"),
    }
}

/// A `/mod/`-scoped moderator cap delegated from the owner (mirrors
/// `OwnedMasthead::delegate_moderation`, but with test-controlled secrets).
fn delegate_mod_cap(site: &OwnedSite, moderator_seed: u8) -> (WriteCapability, SubspaceSecret) {
    let moderator_secret = SubspaceSecret::from_bytes(&[moderator_seed; 32]);
    let moderator_id = moderator_secret.corresponding_subspace_id();
    let mod_area = Area::new(
        Some(moderator_id.clone()),
        Path::from_slices(&[MOD_COMPONENT]).expect("mod area path"),
        full_time_range(),
    );
    let mut cap = site.owner_cap.clone();
    cap.try_delegate(&site.owner_secret, mod_area, moderator_id)
        .expect("owner delegates a /mod/-scoped moderator cap");
    (cap, moderator_secret)
}

#[test]
fn owner_signed_mod_record_is_admitted() {
    let site = manual_owned_site(1, 2);
    let owner_id = site.owner_secret.corresponding_subspace_id();
    let entry = build_entry(
        site.namespace_id.clone(),
        owner_id,
        Path::from_slices(&[MOD_COMPONENT, b"revoke", b"id-1"]).expect("mod path"),
        b"opaque moderation payload",
    );
    let item = sign_into(
        entry,
        &site.owner_cap,
        &site.owner_secret,
        b"opaque moderation payload",
    );
    assert_admitted(&item, site.root);
}

#[test]
fn moderator_delegated_mod_record_is_admitted() {
    let site = manual_owned_site(3, 4);
    let (mod_cap, moderator_secret) = delegate_mod_cap(&site, 5);
    let moderator_id = moderator_secret.corresponding_subspace_id();
    let entry = build_entry(
        site.namespace_id.clone(),
        moderator_id,
        Path::from_slices(&[MOD_COMPONENT, b"tombstone", b"id-9"]).expect("mod path"),
        b"opaque moderation payload",
    );
    let item = sign_into(
        entry,
        &mod_cap,
        &moderator_secret,
        b"opaque moderation payload",
    );
    assert_admitted(&item, site.root);
}

#[test]
fn owner_signed_manifest_path_is_still_unsupported_schema() {
    // /manifest is a reserved region: even the owner's Area::full cap (which
    // authorises the write) is refused at the SCHEMA gate — Unit 2 validates the
    // manifest on an independent path, it is never admitted through this bundle
    // gate. This is the containment that Task 4's /mod/ opening must not weaken.
    let site = manual_owned_site(6, 7);
    let owner_id = site.owner_secret.corresponding_subspace_id();
    let entry = build_entry(
        site.namespace_id.clone(),
        owner_id,
        Path::from_slices(&[MANIFEST_COMPONENT]).expect("manifest path"),
        b"whatever",
    );
    let item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"whatever");
    assert_rejected_code(&item, site.root, DiagnosticCode::UnsupportedSchema);
}
