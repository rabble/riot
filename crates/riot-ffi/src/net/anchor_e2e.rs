//! Slice 2 — Checkpoint-B-in-Rust: a phone pulls a community's committed data
//! from an in-process anchor over REAL loopback iroh `riot/sync/2 ReadCommitted`,
//! verifies every entry through the canonical gate, and imports the
//! store-admissible entries into a real phone profile's willow store.
//!
//! This is the on-host proof of the "leave the room, still sync" win, short of a
//! physical device: the anchor is seeded with committed O/C/W content (including
//! the owner-signed `/manifest`), then the NEW `sync_with_anchor` client dials
//! it, drives the ReadCommitted FSM to completion, and lands the entries in a
//! fresh durable phone profile — byte-identical, queryable through the normal
//! profile read path.
//!
//! The anchor is seeded through `AnchorRepository`'s committed-state writers,
//! which persist byte-identical rows to the composite `CommitHost` promotion
//! (mirroring `riot-anchor`'s own `commit_site_fixture` test helper): the
//! PrepareHost→push→CommitHost control dance is already proven in
//! `riot-anchor/tests/daemon_e2e.rs`; this test's job is the CLIENT half.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};

use riot_anchor::config::{assemble_service, resolve_config, Config};
use riot_anchor::daemon::{bind_local_anchor_endpoint, serve, EntropyFn};
use riot_anchor::repository::{AnchorRepository, StagedEntry};
use riot_anchor::sync_service::{encode_item, verify_anchor_item};

use riot_anchor_protocol::authority::SITE_MANIFEST_DIGEST_LABEL;
use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::digest::digest_v1;
use riot_anchor_protocol::records::{
    PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2, TransportFloor,
};

use riot_core::model::{Certainty, Severity, Urgency};
use riot_core::newswire::{
    create_signed_news_post, create_signed_space_descriptor, inspect_news_record, NewsPostV1,
    ProjectionClockV1, SignedNewswireRecord, SpaceDescriptorV1,
};
use riot_core::site::{
    encode_site_manifest, validate_site_manifest, RequireTransport, SiteDisplay, SiteLayout,
    SiteManifestV1, SiteMemberV1, SiteRole, SiteRule, TransportPolicyV1,
};
use riot_core::willow::site_paths::ARTICLES_COMPONENT;
use riot_core::willow::{
    create_signed_alert, encode_capability, encode_entry, generate_communal_author,
    generate_communal_author_for_namespace, generate_space_organizer_author, AlertDraft, Entry,
    Path, SignedWillowEntry, MANIFEST_COMPONENT,
};

use willow25::authorisation::WriteCapability;
use willow25::entry::{NamespaceSecret, SubspaceSecret};

use riot_transport::iroh::{addr_from_node_id, dialable_addr};

use crate::mobile_state::{hex, open_local_profile, open_local_profile_with_database};
use crate::net::anchor::AnchorPullError;
use crate::net::{
    bind_net_runtime, AnchorSyncError, AnchorSyncOutcome, NamespacePullOutcome, NetRuntime,
};

// ---------------------------------------------------------------------------
// Paths / clock.
// ---------------------------------------------------------------------------

fn unique_db(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "riot-ffi-net-{}-{}-{}.db",
        tag,
        std::process::id(),
        id
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1_700_000_000)
}

// ---------------------------------------------------------------------------
// A REAL owned-root composite site fixture, minted from public APIs (a shipping-
// crate port of riot-anchor's private `hosting_common` test helpers, which are
// not reachable across crates).
// ---------------------------------------------------------------------------

struct OwnedSiteRoot {
    namespace_secret: NamespaceSecret,
    root_signing_key: SigningKey,
    root_id: [u8; 32],
}

/// Rejection-sample an owned namespace deterministically from `seed`. The owned
/// namespace id IS the raw Ed25519 verifying key, so the SAME secret authorises
/// the manifest entry and signs the ticket.
fn owned_site_root(seed: u8) -> OwnedSiteRoot {
    for n in 0u16..=4096 {
        let mut secret_bytes = [seed; 32];
        secret_bytes[0] = (n & 0xff) as u8;
        secret_bytes[1] = (n >> 8) as u8;
        let namespace_secret = NamespaceSecret::from_bytes(&secret_bytes);
        let namespace_id = namespace_secret.corresponding_namespace_id();
        if namespace_id.is_owned() {
            let root_signing_key = SigningKey::from_bytes(&secret_bytes);
            assert_eq!(
                root_signing_key.verifying_key().to_bytes(),
                *namespace_id.as_bytes(),
                "owned namespace id must equal the ed25519 verifying key",
            );
            return OwnedSiteRoot {
                namespace_secret,
                root_signing_key,
                root_id: *namespace_id.as_bytes(),
            };
        }
    }
    panic!("no owned namespace found for seed {seed}");
}

/// One genuinely-authorised communal alert item, projected to a verified staged
/// entry (mirrors `hosting_common::make_item`).
fn make_alert_item(headline: &str) -> StagedEntry {
    let author = generate_communal_author().expect("communal author");
    let draft = AlertDraft {
        valid_from: None,
        expires_at: u64::MAX - 1,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: headline.into(),
        description: "slice-2 e2e entry".into(),
        affected_area_claim: None,
        source_claims: vec!["src".into()],
        ai_assisted: false,
    };
    let alert = create_signed_alert(&author, draft).expect("signed alert");
    let signed = alert.signed;
    let item_bytes = encode_item(
        &signed.entry_bytes,
        &signed.capability_bytes,
        &signed.signature,
        &signed.payload_bytes,
    );
    verify_anchor_item(&item_bytes).expect("genuine alert item verifies")
}

/// A plain newswire community minted for the WIRE namespace: a signed
/// `SpaceDescriptorV1` plus a handful of real posts from DIFFERENT communal
/// authors — the "people talking" a follower walks into. Unlike a digest-only
/// alert, newswire records RETAIN their payload on import (`valid_newswire` in
/// `session.rs`), so after the pull the descriptor is discoverable and
/// `project_space` renders the posts and the people who wrote them.
struct NewswireWire {
    namespace_id: [u8; 32],
    descriptor_entry_id: [u8; 32],
    name: String,
    descriptor_staged: StagedEntry,
    posts_staged: Vec<StagedEntry>,
    /// Headlines in the order authored, so the test can assert real content
    /// crossed the wire (not just counts).
    headlines: Vec<String>,
}

/// Encode a signed newswire record as an anchor item and verify it, mirroring
/// `make_alert_item` but for the newswire families (descriptor + posts).
fn make_newswire_staged(record: &SignedNewswireRecord) -> StagedEntry {
    let s = &record.signed;
    let item = encode_item(
        &s.entry_bytes,
        &s.capability_bytes,
        &s.signature,
        &s.payload_bytes,
    );
    verify_anchor_item(&item).expect("newswire item verifies as an anchor item")
}

/// Seed a plain newswire community: one organizer-signed descriptor plus three
/// posts, each from a DISTINCT communal author, so the wire shows real people
/// talking. All live in one communal namespace (the descriptor's), which becomes
/// the site's W (open-wire) namespace.
fn make_newswire_wire() -> NewswireWire {
    let organizer = generate_space_organizer_author().expect("space organizer author");
    let namespace_id = *organizer.namespace_id().as_bytes();
    let name = "River City Wire".to_string();
    let descriptor = create_signed_space_descriptor(
        &organizer,
        SpaceDescriptorV1 {
            namespace_id,
            name: name.clone(),
            summary: "Neighbours reporting what's happening on the block.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
            editorial_roster: vec![],
            predecessor: None,
            successor: None,
        },
    )
    .expect("sign space descriptor");
    let verified = inspect_news_record(&descriptor.signed).expect("verify descriptor");

    let headlines = [
        "Rent strike meeting Thursday, 7pm",
        "Free breakfast at the corner church",
        "Bike lane reopens on 5th",
    ];
    let mut posts_staged = Vec::new();
    for headline in headlines {
        // A fresh communal author per post → distinct people on the wire.
        let writer = generate_communal_author_for_namespace(namespace_id).expect("communal writer");
        let post = create_signed_news_post(
            &writer,
            &verified,
            NewsPostV1 {
                space_descriptor_entry_id: descriptor.entry_id,
                headline: headline.into(),
                body: "Full report inside.".into(),
                language: "en".into(),
                event_time_unix_seconds: None,
                expires_at_unix_seconds: None,
                coarse_location: None,
                source_claims: vec!["neighbour".into()],
                operational_profile: None,
                ai_assisted: false,
            },
        )
        .expect("sign news post");
        posts_staged.push(make_newswire_staged(&post));
    }

    NewswireWire {
        namespace_id,
        descriptor_entry_id: descriptor.entry_id,
        name,
        descriptor_staged: make_newswire_staged(&descriptor),
        posts_staged,
        headlines: headlines.iter().map(|h| h.to_string()).collect(),
    }
}

/// Build one owner-signed entry in the owned namespace at `path` carrying
/// `payload`, verified into staged form (used for `/manifest` and `/articles`).
fn make_owned_entry(root: &OwnedSiteRoot, seed: u8, path: Path, payload: &[u8]) -> StagedEntry {
    let owner = SubspaceSecret::from_bytes(&[seed ^ 0x5A; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let cap = WriteCapability::new_owned(&root.namespace_secret, owner_id.clone());
    let entry = Entry::builder()
        .namespace_id(root.namespace_secret.corresponding_namespace_id())
        .subspace_id(owner_id)
        .path(path)
        .timestamp(1_000u64)
        .payload(payload)
        .build();
    let authorised = entry
        .into_authorised_entry(&cap, &owner)
        .expect("owner authorises entry");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let item_bytes = encode_item(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        payload,
    );
    verify_anchor_item(&item_bytes).expect("owned item verifies")
}

fn site_manifest_bytes(root: [u8; 32], c: [u8; 32], w: [u8; 32], version: u64) -> Vec<u8> {
    let manifest = SiteManifestV1 {
        root,
        members: vec![
            SiteMemberV1 {
                ns: root,
                role: SiteRole::Masthead,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::FrontArticles,
            },
            SiteMemberV1 {
                ns: c,
                role: SiteRole::Comments,
                rule: SiteRule::CommunalOpen,
                display: SiteDisplay::UnderArticles,
            },
            SiteMemberV1 {
                ns: w,
                role: SiteRole::OpenWire,
                rule: SiteRule::CommunalOpen,
                display: SiteDisplay::WireColumn,
            },
        ],
        moderation_path: vec![b"mod".to_vec()],
        transport_policy: TransportPolicyV1 {
            allow: vec![],
            require: RequireTransport::None,
        },
        version,
        layout: SiteLayout::SiteDefault,
        sections: vec![],
    };
    encode_site_manifest(&manifest).expect("encode site manifest")
}

/// Root-sign a `RootSignedTicketCoreEnvelopeV2` with the owned root's key, at the
/// given transport floor.
fn sign_ticket(
    root: &OwnedSiteRoot,
    namespaces: [[u8; 32]; 3],
    manifest_digest: [u8; 32],
    manifest_version: u64,
    floor: TransportFloor,
    issued: u64,
    expiry: u64,
) -> Vec<u8> {
    let core = PublicSiteTicketV2Core {
        root_id: root.root_id,
        o_namespace_id: namespaces[0],
        c_namespace_id: namespaces[1],
        w_namespace_id: namespaces[2],
        manifest_digest,
        manifest_version,
        min_sync_version: 2,
        manifest_required_transport: floor,
        transport_floor: floor,
        transport_epoch: 1,
        issued_unix_seconds: issued,
        expiry_unix_seconds: expiry,
    };
    let mut envelope = RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: [0u8; 64],
    };
    let preimage = envelope.signing_preimage().expect("ticket preimage");
    envelope.root_signature = root.root_signing_key.sign(&preimage).to_bytes();
    envelope.encode_canonical().expect("encode ticket envelope")
}

struct SiteFixture {
    root_id: [u8; 32],
    namespaces: [[u8; 32]; 3],
    manifest_version: u64,
    manifest_digest: [u8; 32],
    manifest_payload_bytes: Vec<u8>,
    manifest_staged: StagedEntry,
    articles_staged: StagedEntry,
    c_staged: StagedEntry,
    /// The W (open-wire) namespace is a real, walk-into-able newswire community:
    /// a descriptor + posts from distinct people, not a single digest-only alert.
    wire: NewswireWire,
    ticket_envelope_bytes: Vec<u8>,
}

/// Mint a full owned-root site fixture: O carries the owner-signed `/manifest`
/// AND a store-admissible owner-signed `/articles` entry; C carries one communal
/// alert (comments); W is a plain newswire community (descriptor + posts from
/// distinct authors) — the walk-into-able wire. The ticket is root-signed at
/// `floor`.
fn make_site_fixture(seed: u8, version: u64, floor: TransportFloor, now: u64) -> SiteFixture {
    let root = owned_site_root(seed);
    let c = make_alert_item("wire-comment");
    let wire = make_newswire_wire();
    let namespaces = [root.root_id, c.namespace_id, wire.namespace_id];

    let payload = site_manifest_bytes(root.root_id, c.namespace_id, wire.namespace_id, version);
    let manifest_digest = digest_v1(SITE_MANIFEST_DIGEST_LABEL, &payload);
    let manifest_staged = make_owned_entry(
        &root,
        seed,
        Path::from_slices(&[MANIFEST_COMPONENT]).expect("manifest path"),
        &payload,
    );
    let articles_staged = make_owned_entry(
        &root,
        seed,
        Path::from_slices(&[ARTICLES_COMPONENT, b"post-1"]).expect("articles path"),
        b"owned editorial article body",
    );
    let ticket_envelope_bytes = sign_ticket(
        &root,
        namespaces,
        manifest_digest,
        version,
        floor,
        now.saturating_sub(100),
        now + 3600,
    );

    SiteFixture {
        root_id: root.root_id,
        namespaces,
        manifest_version: version,
        manifest_digest,
        manifest_payload_bytes: payload,
        manifest_staged,
        articles_staged,
        c_staged: c,
        wire,
        ticket_envelope_bytes,
    }
}

/// Persist a fixture as the anchor's COMMITTED state — the community row, the
/// committed manifest (what ReadCommitted equality reads), and every committed
/// entry per ordered O/C/W namespace. O gets TWO committed entries (`/manifest`
/// + `/articles`).
fn commit_site(repo: &mut AnchorRepository, site: &SiteFixture, now: u64) {
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&site.root_id, now)
        .expect("insert community");
    tx.upsert_manifest(
        &site.root_id,
        site.manifest_version,
        &site.manifest_digest,
        &site.manifest_payload_bytes,
    )
    .expect("upsert manifest");
    tx.insert_committed_entry(&site.root_id, 0, &site.manifest_staged)
        .expect("commit O manifest");
    tx.insert_committed_entry(&site.root_id, 0, &site.articles_staged)
        .expect("commit O article");
    tx.insert_committed_entry(&site.root_id, 1, &site.c_staged)
        .expect("commit C entry");
    // W: the newswire community — its descriptor, then each post.
    tx.insert_committed_entry(&site.root_id, 2, &site.wire.descriptor_staged)
        .expect("commit W descriptor");
    for post in &site.wire.posts_staged {
        tx.insert_committed_entry(&site.root_id, 2, post)
            .expect("commit W post");
    }
    tx.commit().expect("commit site");
}

// ---------------------------------------------------------------------------
// The in-process anchor: seed committed state, then run `serve` on a dedicated
// OS thread with its own multi-thread runtime. The phone's `sync_with_anchor`
// runs on the calling thread (no ambient tokio runtime — as on a device), so its
// own `block_on` runtime never nests inside the anchor's.
// ---------------------------------------------------------------------------

fn daemon_config(db_path: &std::path::Path) -> Config {
    let args = vec!["--db".to_string(), db_path.to_string_lossy().into_owned()];
    let env = vec![
        ("RIOT_ANCHOR_OPERATOR_KEY_HEX".to_string(), "07".repeat(32)),
        ("RIOT_ANCHOR_ENDPOINT_KEY_HEX".to_string(), "08".repeat(32)),
        (
            "RIOT_ANCHOR_HTTPS_ORIGIN".to_string(),
            "https://anchor.test".to_string(),
        ),
        (
            "RIOT_ANCHOR_DISPLAY_LABEL".to_string(),
            "Slice2 anchor".to_string(),
        ),
        ("RIOT_ANCHOR_FAILURE_DOMAIN".to_string(), "test".to_string()),
    ];
    resolve_config(&args, &env).expect("test daemon config resolves")
}

struct RunningAnchor {
    addr: iroh::EndpointAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl RunningAnchor {
    /// Bind + serve the anchor over its pre-seeded database.
    fn start(db_path: std::path::PathBuf, endpoint_secret: [u8; 32]) -> Self {
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("anchor runtime builds");
            rt.block_on(async move {
                let endpoint = bind_local_anchor_endpoint(endpoint_secret)
                    .await
                    .expect("anchor endpoint binds");
                let addr = dialable_addr(&endpoint).await;
                addr_tx.send(addr).expect("send anchor addr");
                let (daemon_config, service) = assemble_service(daemon_config(&db_path));
                let mut byte = 0x70u8;
                let entropy: EntropyFn = Box::new(move || {
                    let value = [byte; 32];
                    byte = byte.wrapping_add(1);
                    value
                });
                let _ = serve(endpoint, daemon_config, service, entropy, async move {
                    let _ = shutdown_rx.await;
                })
                .await;
            });
        });
        let addr = addr_rx.recv().expect("anchor reports its address");
        RunningAnchor {
            addr,
            shutdown: Some(shutdown_tx),
            handle: Some(handle),
        }
    }
}

impl Drop for RunningAnchor {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

/// Checkpoint B (in Rust): the phone pulls a committed community's O/C/W over
/// iroh sync/2 ReadCommitted, verifies every entry, and imports the
/// store-admissible content into a fresh durable profile byte-identical.
#[test]
fn phone_pulls_and_imports_committed_community_from_anchor() {
    let now = now_secs();
    let site = make_site_fixture(0x33, 7, TransportFloor::RequireNone, now);

    // Seed committed state BEFORE the daemon opens the database (the daemon then
    // opens the same path — exactly the daemon_e2e pre-seed pattern).
    let anchor_db = unique_db("anchor");
    {
        let mut repo = AnchorRepository::open(&anchor_db).expect("open anchor db");
        commit_site(&mut repo, &site, now);
    }
    let anchor = RunningAnchor::start(anchor_db.clone(), [170u8; 32]);

    // The manifest is a genuine, owner-signed, valid site manifest (defense: the
    // canonical manifest gate accepts what the anchor served).
    validate_site_manifest(&to_signed(&site.manifest_staged), &site.root_id)
        .expect("seeded manifest validates through the canonical gate");

    // Fresh DURABLE phone profile (signed_entries_in_namespace is durable-only).
    let phone_db = unique_db("phone");
    let profile = open_local_profile_with_database(phone_db.to_string_lossy().into_owned())
        .expect("phone profile opens");

    let net = NetRuntime::bind_follower().expect("phone net runtime binds");
    let outcome = net
        .sync_with_anchor(
            &profile.inner,
            anchor.addr.clone(),
            &site.ticket_envelope_bytes,
            now,
        )
        .expect("sync_with_anchor succeeds");

    assert_eq!(
        outcome.root,
        hex(&site.root_id),
        "outcome carries the community root (lowercase hex)"
    );
    assert_eq!(
        outcome.namespaces.len(),
        3,
        "one outcome per O/C/W namespace"
    );

    // Per-namespace expectations. O: 2 pulled+verified (manifest + article), 1
    // imported (the reserved /manifest is verified but deliberately NOT a willow
    // store entry — validated on its own path). C: 1 alert. W: the newswire
    // community — descriptor + one post per author.
    let post_count = site.wire.headlines.len() as u32;
    let w_entries = 1 + post_count; // descriptor + posts
    let by_ns = |ns: [u8; 32]| {
        let id = hex(&ns);
        outcome
            .namespaces
            .iter()
            .find(|o| o.namespace_id == id)
            .expect("namespace present in outcome")
    };
    let o = by_ns(site.namespaces[0]);
    assert_eq!(o.verified, 2, "O: manifest + article verified");
    assert_eq!(o.rejected, 0, "O: nothing failed the gate");
    assert_eq!(o.imported, 1, "O: only the article is store-admissible");
    assert!(o.refusal.is_none(), "O: ReadCommitted completed");
    let c = by_ns(site.namespaces[1]);
    assert_eq!(
        (c.verified, c.imported, c.rejected),
        (1, 1, 0),
        "C imported"
    );
    assert!(c.refusal.is_none());
    let w = by_ns(site.namespaces[2]);
    assert_eq!(
        (w.verified, w.imported, w.rejected),
        (w_entries, w_entries, 0),
        "W: the whole newswire community imported (descriptor + every post)"
    );
    assert!(w.refusal.is_none());

    assert_eq!(outcome.total_verified(), 3 + w_entries);
    assert_eq!(outcome.total_imported(), 2 + w_entries);

    // Queryable through the normal profile read path, BYTE-IDENTICAL to what the
    // anchor committed. Entry ids are content-addressed over the canonical entry
    // bytes (which cryptographically bind the payload digest), so an id present
    // in the live store proves the exact bytes landed — the uniform proof across
    // owned + communal families.
    with_store(&profile, |store| {
        let live = store.live_entry_ids().expect("live ids");
        assert!(
            live.contains(&site.articles_staged.entry_id),
            "O: the owned /articles entry landed byte-identical"
        );
        assert!(
            live.contains(&site.c_staged.entry_id),
            "C: the communal comment entry landed byte-identical"
        );
        assert!(
            live.contains(&site.wire.descriptor_staged.entry_id),
            "W: the newswire descriptor landed byte-identical"
        );
        for post in &site.wire.posts_staged {
            assert!(
                live.contains(&post.entry_id),
                "W: every newswire post landed byte-identical"
            );
        }
        // The reserved /manifest is verified but is NOT a willow content entry
        // (validated on its own path) — it must NOT be in the store.
        assert!(
            !live.contains(&site.manifest_staged.entry_id),
            "the /manifest is never admitted as store content"
        );
        assert_eq!(
            live.len() as u32,
            2 + w_entries,
            "exactly the article + comment + newswire community entries landed"
        );

        // STRONGEST form for the owned article: every signed component read back
        // VERBATIM from durable storage (owned entries retain their payload on the
        // signed-offer read path).
        let o_signed = store
            .signed_entries_in_namespace(&site.namespaces[0])
            .expect("query O")
            .expect("durable store answers");
        assert_eq!(o_signed.len(), 1, "O store holds only the /articles entry");
        assert_eq!(
            o_signed[0],
            to_signed(&site.articles_staged),
            "the owned article is byte-identical (entry+cap+sig+payload)"
        );
    });

    // THE WALK-IN (backbone UX slice): after the durable pull, discovery surfaces
    // the WIRE as a real newswire community the follower can open — its descriptor
    // is present (newswire payloads are retained on import), so the community
    // carries its own name, its post count, and the count of people who wrote
    // them. This is the "leave the room, still connected" win: a pull yields a
    // place with people talking, not an empty shell.
    let pulled_ns: Vec<String> = site.namespaces.iter().map(|n| hex(n)).collect();
    let candidates = crate::mobile_state::discover_synced_communities(&profile.inner, pulled_ns)
        .expect("discover synced communities after the pull");
    let wire_hex = hex(&site.wire.namespace_id);
    let wire_candidate = candidates
        .iter()
        .find(|c| c.namespace_id == wire_hex)
        .expect("the pulled wire is an adoptable newswire community");
    assert_eq!(
        wire_candidate.name.as_deref(),
        Some(site.wire.name.as_str()),
        "the community carries its own signed name"
    );
    assert_eq!(
        wire_candidate.descriptor_entry_id.as_deref(),
        Some(hex(&site.wire.descriptor_entry_id).as_str()),
        "discovery hands back the descriptor its wire projects from"
    );
    assert_eq!(
        wire_candidate.post_count, post_count,
        "every post is on the wire"
    );
    assert_eq!(
        wire_candidate.contributor_count, post_count,
        "one distinct person per post — real people you can see"
    );

    // And the wire actually PROJECTS those posts with their authors — the follower
    // opens it and reads what these people wrote.
    with_store(&profile, |store| {
        let clock = ProjectionClockV1::system().expect("projection clock");
        let projection =
            riot_core::newswire::project_space(store, site.wire.descriptor_entry_id, clock)
                .expect("the pulled wire projects");
        let posts: Vec<_> = projection
            .open_wire
            .iter()
            .chain(projection.front_page.iter())
            .chain(projection.earlier.iter())
            .collect();
        assert_eq!(
            posts.len(),
            site.wire.headlines.len(),
            "the wire shows every post that crossed the internet"
        );
        let authors: std::collections::BTreeSet<[u8; 32]> =
            posts.iter().map(|p| p.author_id).collect();
        assert_eq!(
            authors.len(),
            site.wire.headlines.len(),
            "distinct people wrote them"
        );
        for headline in &site.wire.headlines {
            assert!(
                posts
                    .iter()
                    .any(|p| p.headline.as_deref() == Some(headline.as_str())),
                "the real headline '{headline}' survived the crossing"
            );
        }
    });

    drop(anchor);
    let _ = std::fs::remove_file(&anchor_db);
    let _ = std::fs::remove_file(&phone_db);
}

/// Fail-closed: a valid root-signed ticket for a community the anchor never
/// committed imports NOTHING and does not crash — every namespace refuses at
/// ReadCommitted open, zero entries land.
#[test]
fn phone_pull_for_uncommitted_community_imports_nothing() {
    let now = now_secs();

    // The anchor commits community A only.
    let committed = make_site_fixture(0x41, 3, TransportFloor::RequireNone, now);
    let anchor_db = unique_db("anchor-uncommitted");
    {
        let mut repo = AnchorRepository::open(&anchor_db).expect("open anchor db");
        commit_site(&mut repo, &committed, now);
    }
    let anchor = RunningAnchor::start(anchor_db.clone(), [171u8; 32]);

    // A DIFFERENT community's valid ticket — never committed to this anchor.
    let orphan = make_site_fixture(0x55, 2, TransportFloor::RequireNone, now);

    let phone_db = unique_db("phone-uncommitted");
    let profile = open_local_profile_with_database(phone_db.to_string_lossy().into_owned())
        .expect("phone profile opens");
    let net = NetRuntime::bind_follower().expect("phone net runtime binds");

    let outcome = net
        .sync_with_anchor(
            &profile.inner,
            anchor.addr.clone(),
            &orphan.ticket_envelope_bytes,
            now,
        )
        .expect("sync_with_anchor returns an outcome, not a crash");

    assert_eq!(outcome.root, hex(&orphan.root_id));
    assert_eq!(outcome.total_imported(), 0, "nothing imported");
    assert_eq!(outcome.total_verified(), 0, "nothing was even served");
    for ns in &outcome.namespaces {
        assert!(
            ns.refusal.is_some(),
            "each uncommitted namespace refuses at ReadCommitted open: {ns:?}"
        );
    }
    // The phone store is untouched.
    with_store(&profile, |store| {
        for ns in orphan.namespaces {
            let entries = store.signed_entries_in_namespace(&ns).expect("query");
            assert!(
                entries.map(|e| e.is_empty()).unwrap_or(true),
                "no orphan entries landed"
            );
        }
    });

    drop(anchor);
    let _ = std::fs::remove_file(&anchor_db);
    let _ = std::fs::remove_file(&phone_db);
}

/// Security requirement 1: the transport-floor gate refuses a `require:arti`
/// ticket BEFORE any dial. The phone provides iroh but not arti, so the dial
/// fails closed — no connection, no anchor even needed.
#[test]
fn require_arti_ticket_is_refused_before_any_dial() {
    let now = now_secs();
    let site = make_site_fixture(0x63, 5, TransportFloor::RequireArti, now);

    // An in-memory profile is enough: the gate refuses before touching it.
    let profile = open_local_profile().expect("profile opens");
    let net = NetRuntime::bind_follower().expect("net runtime binds");

    // A syntactically-valid but UNREACHABLE anchor address; the gate must refuse
    // before it is ever used.
    let bogus = addr_from_node_id(
        SigningKey::from_bytes(&[5u8; 32])
            .verifying_key()
            .to_bytes(),
    )
    .expect("addr from node id");

    let result = net.sync_with_anchor(&profile.inner, bogus, &site.ticket_envelope_bytes, now);
    match result {
        Err(AnchorPullError::DialRefused(_)) => {}
        other => panic!("require:arti ticket must be refused before dial, got {other:?}"),
    }
}

/// Slice 3a: the EXPORTED UniFFI surface is real and callable host-side. The
/// exported `MobileNetRuntime` is constructed through `bind_net_runtime` and its
/// exported `sync_with_anchor` is driven across the FFI boundary, proving:
///   1. the security-critical transport-floor gate holds THROUGH the boundary —
///      a `require:arti` ticket refuses before any dial and surfaces the flat
///      FFI `AnchorSyncError::DialRefused`;
///   2. a malformed anchor hint is refused at the boundary as `BadAnchorAddress`;
///   3. the outcome record projects ids to lowercase hex.
///
/// No device, no live anchor — the gate/hint checks return before any socket.
#[test]
fn exported_mobile_net_runtime_is_callable_over_ffi() {
    let now = now_secs();
    let site = make_site_fixture(0x7a, 4, TransportFloor::RequireArti, now);
    let profile = open_local_profile().expect("profile opens");
    let net = bind_net_runtime().expect("FFI net runtime binds through the exported entry");

    // A syntactically-valid node hint (64 hex chars) the gate never dials.
    let hint = hex(&SigningKey::from_bytes(&[9u8; 32])
        .verifying_key()
        .to_bytes());
    let refused = net.sync_with_anchor(
        Arc::clone(&profile),
        hint,
        site.ticket_envelope_bytes.clone(),
        now,
    );
    match refused {
        Err(AnchorSyncError::DialRefused { .. }) => {}
        other => panic!("require:arti must refuse THROUGH the FFI boundary, got {other:?}"),
    }

    // A malformed anchor hint is refused at the boundary, before the gate.
    let bad = net.sync_with_anchor(
        Arc::clone(&profile),
        "not-a-node-hint".to_string(),
        site.ticket_envelope_bytes.clone(),
        now,
    );
    assert!(
        matches!(bad, Err(AnchorSyncError::BadAnchorAddress { .. })),
        "a malformed anchor hint is refused at the FFI boundary: {bad:?}"
    );

    // The FFI outcome record round-trips: build one and confirm the hex/u32
    // projection is what native code receives.
    let outcome = AnchorSyncOutcome {
        root: hex(&site.root_id),
        namespaces: vec![NamespacePullOutcome {
            namespace_id: hex(&site.namespaces[0]),
            verified: 2,
            imported: 1,
            refusal: None,
            rejected: 0,
        }],
    };
    assert_eq!(outcome.root.len(), 64, "root is 64 lowercase-hex chars");
    assert_eq!(outcome.total_verified(), 2);
    assert_eq!(outcome.total_imported(), 1);
}

// ---------------------------------------------------------------------------
// Small helpers to reach the phone store's read path in-crate.
// ---------------------------------------------------------------------------

fn to_signed(staged: &StagedEntry) -> SignedWillowEntry {
    // Re-derive the four signed components from the committed item bytes — the
    // exact bytes the anchor served and the phone imported.
    let verified = riot_anchor::sync_service::verify_anchor_item_parts(&staged.item_bytes)
        .expect("staged item re-verifies");
    SignedWillowEntry {
        entry_bytes: verified.entry_bytes,
        capability_bytes: verified.capability_bytes,
        signature: verified.signature,
        payload_bytes: verified.payload_bytes,
    }
}

fn with_store<T>(
    profile: &crate::mobile_api::MobileProfile,
    f: impl FnOnce(&riot_core::session::EvidenceStore) -> T,
) -> T {
    // Run read-back assertions against the active profile's real store, through
    // the same accessor every FFI read uses.
    crate::mobile_state::with_active(&profile.inner, |p| Ok(f(&p.store))).expect("active profile")
}
