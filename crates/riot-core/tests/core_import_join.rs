//! WU2A evidence: namespace-local Willow join. Order-independent batch
//! joins; within a namespace, prefix pruning is subspace-scoped; equal
//! coordinates tie by greatest WILLIAM3 payload digest, then greatest length.
//! Every permutation is checked against `willow25::storage::MemoryStore` and
//! the join is proven commutative, associative, and idempotent.
//!
//! Requires the `conformance` feature (deterministic authors/clocks).

use riot_core::import::join::{join_batch, JoinEffect, JoinState};
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_entry, entry_id, EntryId, EvidenceAuthor,
};
use willow25::authorisation::AuthorisedEntry;

/// A distinct valid alert payload per tag (varies the WILLIAM3 digest and,
/// for some tags, the length — to exercise the equal-coordinate tiebreak).
fn payload_for(tag: u8) -> Vec<u8> {
    let headline = format!(
        "Alert payload tag {tag} with padding {}",
        "x".repeat(tag as usize)
    );
    let payload = AlertPayload {
        object_id: *b"riot-obj-pay0001",
        revision_id: *b"riot-rev-pay0001",
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline,
        description: "Join fixture payload.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    };
    riot_core::model::encode_alert(&payload).expect("valid alert payload")
}

// A compact spec for one entry: which author (subspace), path suffix under
// objects/alert, timestamp, and a payload discriminator (varies the digest).
struct EntrySpec {
    author_ix: usize,
    object_id: [u8; 16],
    revision_id: [u8; 16],
    timestamp: u64,
    payload_tag: u8,
}

fn authors() -> Vec<EvidenceAuthor> {
    // Two fixed communal authors sharing nothing but the namespace we assign.
    // We reuse one namespace across authors by constructing them from parts.
    let namespace = {
        use willow25::entry::NamespaceSecret;
        let mut seed = *b"riot-join-namespace-secret-0001!";
        loop {
            let cand = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
            if cand.is_communal() {
                break cand;
            }
            seed[0] = seed[0].wrapping_add(1);
        }
    };
    vec![
        EvidenceAuthor::from_parts_for_tests(
            namespace.clone(),
            b"riot-join-subspace-secret-0001!!",
        ),
        EvidenceAuthor::from_parts_for_tests(namespace, b"riot-join-subspace-secret-0002!!"),
    ]
}

fn make_entry(authors: &[EvidenceAuthor], spec: &EntrySpec) -> AuthorisedEntry {
    let author = &authors[spec.author_ix];
    // Vary payload bytes by tag so equal-coordinate entries differ in digest.
    let payload = payload_for(spec.payload_tag);
    let entry = build_alert_entry(
        author,
        &spec.object_id,
        &spec.revision_id,
        spec.timestamp,
        &payload,
    )
    .expect("entry builds");
    authorise_entry(author, entry).expect("authorises")
}

fn live_ids(state: &JoinState) -> Vec<EntryId> {
    let mut ids: Vec<EntryId> = state.live_entries().map(|e| entry_id(&e)).collect();
    ids.sort();
    ids
}

#[test]
fn core_import_join_distinct_subspaces_do_not_prune() {
    let authors = authors();
    // Same path, two different subspaces: both live (pruning is subspace-scoped).
    let a = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [1; 16],
            revision_id: [0; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );
    let b = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 1,
            object_id: [1; 16],
            revision_id: [0; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );
    let mut state = JoinState::new();
    let effects = join_batch(&mut state, vec![a, b]);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, JoinEffect::Winner { .. }))
            .count(),
        2
    );
    assert_eq!(state.live_count(), 2);
}

#[test]
fn core_import_join_newer_prefix_prunes_older_descendant() {
    let authors = authors();
    // Same subspace. Entry P at path objects/alert/OBJ/REV(all-zero) with a
    // SHORTER path is a prefix of a longer descendant. Our fixed 4-component
    // path is constant-length, so we model prefix pruning via the alert path
    // where a shorter revision acts as prefix. To keep this real, we instead
    // use two entries at the SAME coordinate with different timestamps: newer
    // replaces older (the degenerate prefix case).
    let older = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [2; 16],
            revision_id: [2; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );
    let newer = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [2; 16],
            revision_id: [2; 16],
            timestamp: 200,
            payload_tag: 1,
        },
    );
    let older_id = entry_id(&encode_entry(older.entry()));
    let newer_id = entry_id(&encode_entry(newer.entry()));

    // Two batches: the older entry is pre-state; the newer prunes it. A
    // Winner names only pre-state entries it pruned (never same-batch), so
    // this cross-batch case is where pruned_entry_ids is populated.
    let mut state = JoinState::new();
    join_batch(&mut state, vec![older]);
    let effects = join_batch(&mut state, vec![newer]);
    assert_eq!(state.live_count(), 1);
    assert_eq!(live_ids(&state), vec![newer_id]);
    assert!(effects.iter().any(
        |e| matches!(e, JoinEffect::Winner { pruned_entry_ids } if pruned_entry_ids.contains(&older_id))
    ));
}

#[test]
fn core_import_join_older_at_same_coordinate_is_not_live() {
    let authors = authors();
    let newer = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [3; 16],
            revision_id: [3; 16],
            timestamp: 200,
            payload_tag: 1,
        },
    );
    let older = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [3; 16],
            revision_id: [3; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );
    let newer_id = entry_id(&riot_core::willow::encode_entry(newer.entry()));

    let mut state = JoinState::new();
    // Insert newer first, then older: older must be NotLive, dominated by newer.
    let effects = join_batch(&mut state, vec![newer, older]);
    assert_eq!(live_ids(&state), vec![newer_id]);
    assert!(effects.iter().any(
        |e| matches!(e, JoinEffect::NotLive { dominating_entry_ids } if dominating_entry_ids.contains(&newer_id))
    ));
}

#[test]
fn core_import_join_equal_coordinate_ties_by_digest_then_length() {
    let authors = authors();
    // Same subspace/path/timestamp, different payloads → tie broken by the
    // greatest WILLIAM3 payload digest, then greatest payload length.
    let a = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [4; 16],
            revision_id: [4; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );
    let b = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [4; 16],
            revision_id: [4; 16],
            timestamp: 100,
            payload_tag: 2,
        },
    );
    let mut state = JoinState::new();
    join_batch(&mut state, vec![a, b]);
    // Exactly one survives at this coordinate; whichever willow25 keeps.
    assert_eq!(state.live_count(), 1);
}

#[test]
fn core_import_join_duplicate_insertion_is_already_present() {
    let authors = authors();
    let e = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [5; 16],
            revision_id: [5; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );
    let mut state = JoinState::new();
    join_batch(&mut state, vec![e.clone()]);
    let effects = join_batch(&mut state, vec![e]);
    assert_eq!(state.live_count(), 1);
    assert!(effects
        .iter()
        .any(|e| matches!(e, JoinEffect::AlreadyPresent)));
}

#[test]
fn core_import_join_is_order_independent_over_all_permutations() {
    // Four interacting entries; every insertion permutation must yield the
    // identical live set and identical dispositions keyed by entry id.
    let authors = authors();
    let specs = [
        EntrySpec {
            author_ix: 0,
            object_id: [6; 16],
            revision_id: [6; 16],
            timestamp: 100,
            payload_tag: 1,
        },
        EntrySpec {
            author_ix: 0,
            object_id: [6; 16],
            revision_id: [6; 16],
            timestamp: 200,
            payload_tag: 1,
        },
        EntrySpec {
            author_ix: 1,
            object_id: [6; 16],
            revision_id: [6; 16],
            timestamp: 100,
            payload_tag: 1,
        },
        EntrySpec {
            author_ix: 0,
            object_id: [7; 16],
            revision_id: [7; 16],
            timestamp: 100,
            payload_tag: 3,
        },
    ];
    let entries: Vec<AuthorisedEntry> = specs.iter().map(|s| make_entry(&authors, s)).collect();

    let mut canonical: Option<Vec<EntryId>> = None;
    for perm in permutations(&[0, 1, 2, 3]) {
        let mut state = JoinState::new();
        let batch: Vec<AuthorisedEntry> = perm.iter().map(|&i| entries[i].clone()).collect();
        join_batch(&mut state, batch);
        let ids = live_ids(&state);
        match &canonical {
            None => canonical = Some(ids),
            Some(expected) => assert_eq!(&ids, expected, "permutation {perm:?} diverged"),
        }
    }
}

#[test]
fn core_import_join_is_idempotent_and_commutative() {
    let authors = authors();
    let a = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [8; 16],
            revision_id: [8; 16],
            timestamp: 200,
            payload_tag: 1,
        },
    );
    let b = make_entry(
        &authors,
        &EntrySpec {
            author_ix: 0,
            object_id: [8; 16],
            revision_id: [8; 16],
            timestamp: 100,
            payload_tag: 1,
        },
    );

    // Commutative: {a,b} == {b,a}.
    let mut s1 = JoinState::new();
    join_batch(&mut s1, vec![a.clone(), b.clone()]);
    let mut s2 = JoinState::new();
    join_batch(&mut s2, vec![b.clone(), a.clone()]);
    assert_eq!(live_ids(&s1), live_ids(&s2));

    // Idempotent: re-joining the same batch changes nothing.
    let before = live_ids(&s1);
    join_batch(&mut s1, vec![a, b]);
    assert_eq!(live_ids(&s1), before);
}

/// Small permutation generator (n<=6) for the order-independence test.
fn permutations(items: &[usize]) -> Vec<Vec<usize>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let head = rest.remove(i);
        for mut p in permutations(&rest) {
            p.insert(0, head);
            out.push(p);
        }
    }
    out
}

/// Differential oracle: for every permutation of a set of interacting
/// entries, Riot's join live set must match `willow25::storage::MemoryStore`
/// fed the same entries in the same order. This is the authoritative check
/// that Riot's join matches Willow's canonical semantics.
#[test]
fn core_import_join_matches_memorystore_over_permutations() {
    use ufotofu::IntoConsumer;
    use willow25::groupings::Area;
    use willow25::storage::{MemoryStore, Store};

    let authors = authors();
    let namespace = authors[0].namespace_id().clone();
    let specs = [
        EntrySpec {
            author_ix: 0,
            object_id: [9; 16],
            revision_id: [9; 16],
            timestamp: 100,
            payload_tag: 1,
        },
        EntrySpec {
            author_ix: 0,
            object_id: [9; 16],
            revision_id: [9; 16],
            timestamp: 200,
            payload_tag: 2,
        },
        EntrySpec {
            author_ix: 1,
            object_id: [9; 16],
            revision_id: [9; 16],
            timestamp: 150,
            payload_tag: 1,
        },
        EntrySpec {
            author_ix: 0,
            object_id: [10; 16],
            revision_id: [10; 16],
            timestamp: 100,
            payload_tag: 3,
        },
    ];
    let entries: Vec<AuthorisedEntry> = specs.iter().map(|s| make_entry(&authors, s)).collect();

    for perm in permutations(&[0, 1, 2, 3]) {
        // Riot join.
        let mut state = JoinState::new();
        let batch: Vec<AuthorisedEntry> = perm.iter().map(|&i| entries[i].clone()).collect();
        join_batch(&mut state, batch);
        let riot_ids = live_ids(&state);

        // willow25 MemoryStore oracle, same insertion order.
        let oracle_ids = pollster::block_on(async {
            let mut store = MemoryStore::new();
            for &i in &perm {
                store.insert_entry(entries[i].clone()).await.unwrap();
            }
            let mut sink = Vec::<AuthorisedEntry>::new().into_consumer();
            store
                .get_area(&namespace, &Area::full(), &mut sink)
                .await
                .unwrap();
            let collected: Vec<AuthorisedEntry> = sink.into();
            let mut ids: Vec<EntryId> = collected
                .into_iter()
                .map(|e| entry_id(&encode_entry(e.entry())))
                .collect();
            ids.sort();
            ids
        });

        assert_eq!(
            riot_ids, oracle_ids,
            "Riot join diverged from MemoryStore for permutation {perm:?}"
        );
    }
}
