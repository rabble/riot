//! Composite-site Unit 1 — owned-namespace editorial admission at the bundle
//! chokepoint (`verify_frame` via the root-aware decode variant).
//!
//! The security core: an owned-namespace editorial entry authored under a
//! cryptographically-verified capability chain rooted at the FOLLOWED site is
//! ADMITTED; every forgery is REJECTED, and the rootless `decode_bundle`
//! (used by non-admission inspectors) keeps failing closed on owned caps.
//!
//! Each adversarial case asserts the STRONGEST isolated fact:
//!   * cases the Riot-side policy owns (the `is_owned()` invariant and the
//!     root binding) assert the exact `UnsupportedCapability` diagnostic, so a
//!     regression that dropped the predicate would surface even though
//!     willow25 might still reject for another reason;
//!   * cases willow25 owns (area nesting, time-range, chain/receiver
//!     signatures) assert that willow25's CHECKED assembly refuses to produce
//!     admissible wire bytes at all — we never hand-roll chain verification;
//!   * one case corrupts a genuinely-valid owned article's signature and feeds
//!     it to the gate, proving `verify_frame` still runs the willow25 verifier
//!     for owned entries rather than trusting the policy predicate alone.
//!
//! Adversarial caps are forged as a hostile peer would build them (raw
//! willow25 primitives), never only through the friendly minting API.

use minicbor::Encoder;
use riot_core::import::{
    decode_bundle, decode_bundle_with_root, encode_bundle, BundleDecodeOutcome, DiagnosticCode,
    ItemStatus, BUNDLE_CODEC_ID, BUNDLE_MAGIC,
};
use riot_core::session::{CommitOutcome, ImportContext, InspectOutcome, RiotSession};
use riot_core::sync::{ReconcileSession, SyncAction, SyncFrame};
use riot_core::willow::site_paths::{ARTICLES_COMPONENT, MANIFEST_COMPONENT};
use riot_core::willow::{
    encode_capability, encode_entry, entry_id, Entry, NamespaceId, OwnedMasthead, Path,
    SignedWillowEntry, SubspaceId,
};
use willow25::prelude::{Area, NamespaceSecret, SubspaceSecret, TimeRange, WriteCapability};

// ---------- construction helpers ----------

fn full_time_range() -> TimeRange {
    TimeRange::new(0u64.into(), Some(u64::MAX.into()))
}

fn section_area(subspace: SubspaceId, section: &[u8], times: TimeRange) -> Area {
    Area::new(
        Some(subspace),
        Path::from_slices(&[ARTICLES_COMPONENT, section]).expect("section area path"),
        times,
    )
}

fn article_path(section: &[u8], slug: &[u8]) -> Path {
    Path::from_slices(&[ARTICLES_COMPONENT, section, slug]).expect("article path")
}

fn build_entry(
    namespace: NamespaceId,
    subspace: SubspaceId,
    path: Path,
    timestamp: u64,
    payload: &[u8],
) -> Entry {
    Entry::builder()
        .namespace_id(namespace)
        .subspace_id(subspace)
        .path(path)
        .timestamp(timestamp)
        .payload(payload)
        .build()
}

/// Assemble the four canonical component byte strings from a CHECKED authorised
/// (entry, cap, secret) triple — the same shape `create_signed_alert` emits.
/// Panics if the capability does not authorise the entry (so a "valid" helper
/// can never silently produce unauthorised bytes).
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

/// An owned site whose secrets the test fully controls: the owner can sign
/// records at any path under its `Area::full()` owned cap.
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

// ---------- test-side hostile framer (mirrors public_bundle.rs) ----------

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

/// Assert the item was rejected with a specific diagnostic — pins WHICH gate
/// stage rejected it, so a case cannot pass for the wrong reason.
fn assert_rejected_code(item: &SignedWillowEntry, root: Option<[u8; 32]>, code: DiagnosticCode) {
    match status_with_root(item, root) {
        ItemStatus::Invalid(d) => assert_eq!(d.code, code, "wrong rejection stage"),
        ItemStatus::Valid(_) => panic!("expected rejection {code:?}, item was admitted"),
    }
}

// ---------- admission (Tasks 1 + 2) ----------

#[test]
fn owned_editorial_under_delegated_cap_is_admitted_with_correct_followed_root() {
    let masthead = OwnedMasthead::generate().expect("masthead");
    let root = *masthead.namespace_id().as_bytes();
    let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let cap = masthead
        .delegate_section(
            editor_id.clone(),
            section_area(editor_id.clone(), b"news", full_time_range()),
        )
        .expect("delegation under /articles must mint");
    let entry = build_entry(
        masthead.namespace_id().clone(),
        editor_id,
        article_path(b"news", b"post-1"),
        100,
        b"editorial body bytes",
    );
    let item = sign_into(entry, &cap, &editor, b"editorial body bytes");
    assert_admitted(&item, root);
}

#[test]
fn owner_direct_article_under_full_owned_cap_is_admitted() {
    let site = manual_owned_site(0x30, 0x03);
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        article_path(b"news", b"owner-post"),
        100,
        b"owner body",
    );
    let item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"owner body");
    assert_admitted(&item, site.root);
}

#[test]
fn owned_editorial_fails_closed_when_followed_root_is_absent() {
    // The Option MUST fail closed: an owned entry with no known followed root
    // is rejected by the policy predicate, never admitted on a default.
    let site = manual_owned_site(0x31, 0x04);
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    let item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"body");
    assert_rejected_code(&item, None, DiagnosticCode::UnsupportedCapability);
}

#[test]
fn owned_editorial_rejected_by_the_rootless_decode_bundle() {
    // `decode_bundle` (used by non-admission inspectors) stays fail-closed:
    // it is exactly `decode_bundle_with_root(.., None)`.
    let site = manual_owned_site(0x32, 0x05);
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    let item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"body");
    let bytes = frame_one(&item);
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(&bytes) else {
        panic!("item-level failure must not reject the artifact");
    };
    assert!(matches!(decoded.items[0].status, ItemStatus::Invalid(_)));
}

#[test]
fn owned_editorial_rejected_when_followed_root_is_a_different_site() {
    let site = manual_owned_site(0x33, 0x06);
    let other = manual_owned_site(0x77, 0x08);
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    let item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"body");
    assert_rejected_code(
        &item,
        Some(other.root),
        DiagnosticCode::UnsupportedCapability,
    );
}

#[test]
fn owned_namespace_manifest_path_is_unsupported_schema() {
    // Only `/articles/` is admitted in Unit 1. A valid owner-signed record at
    // the reserved `/manifest` path passes the policy predicate AND willow25
    // authorisation (owner `Area::full` cap), yet is refused at the schema
    // stage — `/manifest`/`/mod/` are reserved for later units.
    let site = manual_owned_site(0x34, 0x07);
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        Path::from_slices(&[MANIFEST_COMPONENT]).unwrap(),
        100,
        b"manifest body",
    );
    let item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"manifest body");
    assert_rejected_code(&item, Some(site.root), DiagnosticCode::UnsupportedSchema);
}

// ---------- adversarial: cases the Riot policy predicate owns ----------

#[test]
fn marker_bit_forgery_communal_cap_naming_owned_namespace_is_rejected() {
    // A communal genesis cap is unconditionally is_valid() and can NAME an
    // owned namespace id. The entry namespace is owned and the followed root
    // matches — ONLY the explicit `capability.is_owned()` invariant stops it,
    // so the rejection must land at the policy predicate.
    let site = manual_owned_site(0x35, 0x09);
    let attacker = SubspaceSecret::from_bytes(&[0x11; 32]);
    let attacker_id = attacker.corresponding_subspace_id();
    let communal = WriteCapability::new_communal(site.namespace_id.clone(), attacker_id.clone());
    let entry = build_entry(
        site.namespace_id.clone(),
        attacker_id,
        article_path(b"news", b"forged"),
        100,
        b"forged body",
    );
    let item = sign_into(entry, &communal, &attacker, b"forged body");
    assert_rejected_code(
        &item,
        Some(site.root),
        DiagnosticCode::UnsupportedCapability,
    );
}

#[test]
fn cross_namespace_cap_reuse_is_rejected_by_root_binding() {
    // A cap genuinely owned-rooted at site A, whose editor tries to author into
    // the FOLLOWED site B's owned namespace. The genesis namespace key != the
    // followed root, so the policy predicate rejects before verification.
    let site_a = manual_owned_site(0x36, 0x0a);
    let site_b = manual_owned_site(0x66, 0x0b);
    // Build raw bytes: an entry in B's namespace carrying A's owner cap. willow25
    // would refuse to sign this (A's cap does not authorise a B entry), so we
    // hand-assemble it — the policy predicate must reject it regardless.
    let entry = build_entry(
        site_b.namespace_id.clone(),
        site_a.owner_secret.corresponding_subspace_id(),
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    let item = SignedWillowEntry {
        entry_bytes: encode_entry(&entry),
        capability_bytes: encode_capability(&site_a.owner_cap),
        signature: [0u8; 64],
        payload_bytes: b"body".to_vec(),
    };
    assert_rejected_code(
        &item,
        Some(site_b.root),
        DiagnosticCode::UnsupportedCapability,
    );
}

// ---------- adversarial: cases willow25 owns (no admissible bytes exist) ----------

#[test]
fn over_broad_write_outside_granted_area_cannot_be_authorised() {
    // A legitimate `/articles/news` editor cannot even ASSEMBLE an authorised
    // entry into `/articles/sports` — willow25's area nesting refuses it.
    let masthead = OwnedMasthead::generate().expect("masthead");
    let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let cap = masthead
        .delegate_section(
            editor_id.clone(),
            section_area(editor_id.clone(), b"news", full_time_range()),
        )
        .unwrap();
    let entry = build_entry(
        masthead.namespace_id().clone(),
        editor_id,
        article_path(b"sports", b"intruder"),
        100,
        b"body",
    );
    assert!(
        entry.into_authorised_entry(&cap, &editor).is_err(),
        "a /articles/news cap must not authorise a /articles/sports write"
    );
}

#[test]
fn expired_cap_outside_time_range_cannot_be_authorised() {
    let masthead = OwnedMasthead::generate().expect("masthead");
    let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let cap = masthead
        .delegate_section(
            editor_id.clone(),
            section_area(
                editor_id.clone(),
                b"news",
                TimeRange::new(1_000u64.into(), Some(2_000u64.into())),
            ),
        )
        .unwrap();
    let entry = build_entry(
        masthead.namespace_id().clone(),
        editor_id,
        article_path(b"news", b"late"),
        5_000,
        b"body",
    );
    assert!(
        entry.into_authorised_entry(&cap, &editor).is_err(),
        "a cap must not authorise an entry outside its time_range"
    );
}

#[test]
fn receiver_mismatch_non_receiver_signer_cannot_be_authorised() {
    // A fully valid delegated cap to editor A, but signed by editor B. B's
    // secret is not the cap's receiver, so willow25 refuses to assemble.
    let masthead = OwnedMasthead::generate().expect("masthead");
    let editor_a = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_a_id = editor_a.corresponding_subspace_id();
    let cap = masthead
        .delegate_section(
            editor_a_id.clone(),
            section_area(editor_a_id.clone(), b"news", full_time_range()),
        )
        .unwrap();
    let editor_b = SubspaceSecret::from_bytes(&[0x42; 32]);
    let entry = build_entry(
        masthead.namespace_id().clone(),
        editor_a_id,
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    assert!(
        entry.into_authorised_entry(&cap, &editor_b).is_err(),
        "an entry signed by a non-receiver key must not authorise"
    );
}

#[test]
fn forged_delegation_chain_bad_link_signature_cannot_be_authorised() {
    // A delegation link signed by a key that is NOT the delegator: willow25's
    // is_valid() rejects the chain, so no authorised entry can be built.
    let masthead = OwnedMasthead::generate().expect("masthead");
    let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let mut forged = masthead.owner_write_capability();
    let wrong_signer = SubspaceSecret::from_bytes(&[0x99; 32]);
    let _ = forged.try_delegate(
        &wrong_signer,
        section_area(editor_id.clone(), b"news", full_time_range()),
        editor_id.clone(),
    );
    let entry = build_entry(
        masthead.namespace_id().clone(),
        editor_id,
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    assert!(
        entry.into_authorised_entry(&forged, &editor).is_err(),
        "a chain with a bad delegation-link signature must not authorise"
    );
}

#[test]
fn gate_still_runs_willow25_verification_for_admitted_owned_shape() {
    // Defense in depth: a genuinely-valid owned article whose signature is then
    // corrupted must be rejected at verification even though it passes the
    // policy predicate — proving `verify_frame` still runs verify_entry for
    // owned entries and does not trust the predicate alone.
    let site = manual_owned_site(0x37, 0x0c);
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        article_path(b"news", b"x"),
        100,
        b"body",
    );
    let mut item = sign_into(entry, &site.owner_cap, &site.owner_secret, b"body");
    item.signature[0] ^= 0x01;
    assert_rejected_code(&item, Some(site.root), DiagnosticCode::DoesNotAuthorise);
}

// ---------- session import path (Task 2 — the followed-root carrier) ----------

/// An owner-signed article item for a manually-controlled owned site.
fn owned_article_item(
    site: &OwnedSite,
    section: &[u8],
    slug: &[u8],
    payload: &[u8],
) -> SignedWillowEntry {
    let entry = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        article_path(section, slug),
        100,
        payload,
    );
    sign_into(entry, &site.owner_cap, &site.owner_secret, payload)
}

#[test]
fn owned_editorial_is_committed_and_live_via_followed_root_import() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let site = manual_owned_site(0x40, 0x0d);
    let item = owned_article_item(&site, b"news", b"post-1", b"editorial body");
    let bundle = encode_bundle(std::slice::from_ref(&item)).expect("encode");

    let preview = match store
        .inspect(
            &bundle,
            ImportContext::with_followed_root("follow-site", site.root),
        )
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("owned editorial rejected: {r:?}"),
    };
    let plan = preview.plan_all().expect("plan_all");
    match plan.commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("owned editorial entry was dropped, not committed"),
    }
    assert_eq!(store.live_count().expect("live_count"), 1);
}

#[test]
fn owned_editorial_import_without_followed_root_admits_nothing() {
    // Fail-closed at the session boundary: a plain import with no site-follow
    // context must not admit or commit owned content.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let site = manual_owned_site(0x41, 0x0e);
    let item = owned_article_item(&site, b"news", b"post-1", b"editorial body");
    let bundle = encode_bundle(std::slice::from_ref(&item)).expect("encode");

    match store
        .inspect(&bundle, ImportContext::new("plain-file-import"))
        .expect("inspect")
    {
        // The owned item is rejected at `verify_frame`, so it never becomes an
        // eligible entry — `plan_all` reports there is nothing to plan, and
        // nothing can commit. A whole-bundle rejection is equally fail-closed.
        InspectOutcome::Preview(p) => {
            assert!(
                p.plan_all().is_err(),
                "no owned entry may be eligible without a followed root"
            );
        }
        InspectOutcome::Rejected(_) => {}
    }
    assert_eq!(store.live_count().expect("live_count"), 0);
}

// ---------- sync admission path (Task 2 — gate 3) ----------

fn outbound(action: SyncAction) -> SyncFrame {
    match action {
        SyncAction::Send(frame) => frame,
        other => panic!("expected outbound frame, got {other:?}"),
    }
}

#[test]
fn owned_editorial_is_admitted_over_sync_under_the_session_namespace_root() {
    // The reconcile session's namespace is the locally-chosen followed site.
    // For an owned namespace it IS the followed root, so a received bundle of
    // owned editorial entries authored under that root's cap passes gate 3.
    let site = manual_owned_site(0x42, 0x0f);
    let owned_entry = owned_article_item(&site, b"news", b"post-1", b"editorial body");
    let bundle = encode_bundle(std::slice::from_ref(&owned_entry)).expect("encode");

    let mut receiver = ReconcileSession::new(site.root, vec![]).unwrap();
    let mut sender = ReconcileSession::new(site.root, vec![owned_entry.clone()]).unwrap();
    let hello = outbound(receiver.begin().unwrap());
    let summary = outbound(sender.receive(hello).unwrap());
    let _request = outbound(receiver.receive(summary).unwrap());

    // The receiver requested the missing owned entry; delivering it must be
    // accepted for import (gate 3 admits owned editorial under the root).
    match receiver.receive(SyncFrame::Entries {
        namespace_id: site.root,
        bundle_bytes: bundle,
    }) {
        Ok(SyncAction::ImportBundle(_)) => {}
        other => panic!("owned editorial rejected at the sync gate: {other:?}"),
    }
}

// ---------- cross-gate consistency KEYSTONE (Task 5) ----------
//
// The two REAL owned-admission surfaces — the session import gate and the sync
// reconcile gate — both route through the ONE shared `admissible_capability`
// predicate via `decode_bundle_with_root`. This proves they agree in BOTH
// directions: a valid owned article is admitted identically at every surface,
// and a forgery is rejected identically at every surface (no surface stricter
// or looser than another). The bundle chokepoint is the third witness.

fn bundle_admits(item: &SignedWillowEntry, root: [u8; 32]) -> bool {
    matches!(status_with_root(item, Some(root)), ItemStatus::Valid(_))
}

fn session_admits(item: &SignedWillowEntry, root: [u8; 32]) -> bool {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    // Hand-framed bytes so a forgery (which the producer-side encode preflight
    // would refuse) can still be fed to the gate as a hostile peer would.
    let bytes = frame_one(item);
    match store
        .inspect(&bytes, ImportContext::with_followed_root("keystone", root))
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p.plan_all().is_ok(),
        InspectOutcome::Rejected(_) => false,
    }
}

fn sync_admits(item: &SignedWillowEntry, root: [u8; 32]) -> bool {
    // Drive a receiver to request the item by id, then deliver the raw bytes.
    let id = entry_id(&item.entry_bytes);
    let mut receiver = ReconcileSession::new(root, vec![]).unwrap();
    receiver.begin().unwrap();
    // A peer advertises the id; the empty receiver requests it (AwaitingEntries).
    let _request = receiver.receive(SyncFrame::Summary {
        namespace_id: root,
        entry_ids: vec![id],
    });
    matches!(
        receiver.receive(SyncFrame::Entries {
            namespace_id: root,
            bundle_bytes: frame_one(item),
        }),
        Ok(SyncAction::ImportBundle(_))
    )
}

#[test]
fn valid_owned_article_and_marker_bit_forgery_decide_identically_at_every_gate() {
    let site = manual_owned_site(0x50, 0x10);
    let root = site.root;

    // A valid owner-signed article — must be admitted everywhere.
    let valid = owned_article_item(&site, b"news", b"post-1", b"editorial body");

    // A marker-bit forgery: a communal cap NAMING the owned namespace id, in the
    // owned namespace, at an /articles/ path. Well-formed and signed, but only
    // the `is_owned()` invariant stops it — must be rejected everywhere.
    let attacker = SubspaceSecret::from_bytes(&[0x21; 32]);
    let attacker_id = attacker.corresponding_subspace_id();
    let communal = WriteCapability::new_communal(site.namespace_id.clone(), attacker_id.clone());
    let forged_entry = build_entry(
        site.namespace_id.clone(),
        attacker_id,
        article_path(b"news", b"forged"),
        100,
        b"forged body",
    );
    let forgery = sign_into(forged_entry, &communal, &attacker, b"forged body");

    for (label, item, expected) in [
        ("valid owned article", &valid, true),
        ("marker-bit forgery", &forgery, false),
    ] {
        let b = bundle_admits(item, root);
        let s = session_admits(item, root);
        let y = sync_admits(item, root);
        assert_eq!(b, expected, "{label}: bundle chokepoint disagreed");
        assert_eq!(s, expected, "{label}: session import gate disagreed");
        assert_eq!(y, expected, "{label}: sync reconcile gate disagreed");
        assert!(
            b == s && s == y,
            "{label}: admission gates diverged (bundle={b}, session={s}, sync={y})"
        );
    }
}

// ---------- classification predicate (Task 4 backing) ----------

#[test]
fn is_owned_editorial_entry_recognises_only_owned_articles() {
    use riot_core::willow::decode_entry_canonic;
    use riot_core::willow::site_paths::is_owned_editorial_entry;

    let site = manual_owned_site(0x60, 0x11);
    // An owned `/articles/` entry: recognised.
    let article = owned_article_item(&site, b"news", b"post-1", b"body");
    let article_entry = decode_entry_canonic(&article.entry_bytes).unwrap();
    assert!(is_owned_editorial_entry(&article_entry));

    // An owned entry at `/manifest`: owned but NOT under /articles — not editorial.
    let manifest = build_entry(
        site.namespace_id.clone(),
        site.owner_secret.corresponding_subspace_id(),
        Path::from_slices(&[MANIFEST_COMPONENT]).unwrap(),
        100,
        b"m",
    );
    let manifest = sign_into(manifest, &site.owner_cap, &site.owner_secret, b"m");
    let manifest_entry = decode_entry_canonic(&manifest.entry_bytes).unwrap();
    assert!(!is_owned_editorial_entry(&manifest_entry));

    // A communal alert entry: not owned, so never editorial.
    let author = riot_core::willow::generate_communal_author().unwrap();
    let alert = riot_core::willow::create_signed_alert(
        &author,
        riot_core::willow::AlertDraft {
            valid_from: None,
            expires_at: 2_000_000_000,
            language: "en".into(),
            urgency: riot_core::model::Urgency::Immediate,
            severity: riot_core::model::Severity::Severe,
            certainty: riot_core::model::Certainty::Observed,
            headline: "h".into(),
            description: "d".into(),
            affected_area_claim: None,
            source_claims: vec!["a field observer".into()],
            ai_assisted: false,
        },
    )
    .unwrap();
    let alert_entry = decode_entry_canonic(&alert.signed.entry_bytes).unwrap();
    assert!(!is_owned_editorial_entry(&alert_entry));
}
