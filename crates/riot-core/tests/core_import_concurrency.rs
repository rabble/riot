//! WU2 G2 evidence: the single Arc<Mutex> arbiter linearizes concurrent
//! access to a store's preview/plan lifecycle. Each test races real threads
//! against the same handles and asserts exactly one linearized winner with
//! no torn or double-applied state — never a timing-dependent assertion.

use std::sync::Barrier;

use riot_core::import::encode_bundle;
use riot_core::model::{AlertPayload, Certainty, Severity, Urgency};
use riot_core::session::{CommitOutcome, ImportContext, RiotSession, SessionError};
use riot_core::willow::{
    authorise_entry, build_alert_entry, encode_capability, encode_entry, EvidenceAuthor,
    SignedWillowEntry,
};

fn author() -> EvidenceAuthor {
    use willow25::entry::NamespaceSecret;
    let mut seed = *b"riot-cnc-namespace-secret-00001!";
    let ns = loop {
        let c = NamespaceSecret::from_bytes(&seed).corresponding_namespace_id();
        if c.is_communal() {
            break c;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    EvidenceAuthor::from_parts_for_tests(ns, b"riot-cnc-subspace-secret-000001!")
}

fn signed(author: &EvidenceAuthor, object: u8, tag: u8) -> SignedWillowEntry {
    let payload = riot_core::model::encode_alert(&AlertPayload {
        object_id: [object; 16],
        revision_id: [1; 16],
        created_at: 1_000,
        valid_from: None,
        expires_at: 2_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: format!("concurrency alert {tag}"),
        description: "Concurrency fixture.".into(),
        affected_area_claim: None,
        source_claims: vec!["fixture".into()],
        ai_assisted: false,
    })
    .expect("payload");
    let entry = build_alert_entry(author, &[object; 16], &[1; 16], 100, &payload).expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }
}

fn bundle(items: &[SignedWillowEntry]) -> Vec<u8> {
    encode_bundle(items).expect("encode bundle")
}

fn ctx() -> ImportContext {
    ImportContext::new("concurrency-route")
}

/// Race N threads each issuing a plan from the same live preview. `plan()`
/// supersedes the previously active plan under the arbiter lock, so exactly
/// one of the N plan handles must remain committable; every other handle
/// must observe a well-formed `PlanSuperseded` terminal, never a panic, a
/// double commit, or more than one live winner.
#[test]
fn core_import_concurrency_racing_plan_issuance_yields_exactly_one_committable_winner() {
    const THREADS: usize = 8;
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let preview = store
        .inspect(&bundle(&[signed(&a, 1, 1)]), ctx())
        .unwrap()
        .expect_preview();

    let barrier = Barrier::new(THREADS);
    let plans: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let preview = &preview;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    preview.plan_all().expect("issuance itself never fails")
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let commit_results: Vec<_> = plans.iter().map(|plan| plan.commit()).collect();
    let committed = commit_results
        .iter()
        .filter(|r| matches!(r, Ok(CommitOutcome::Committed(_))))
        .count();
    let superseded = commit_results
        .iter()
        .filter(|r| matches!(r, Err(SessionError::PlanSuperseded)))
        .count();
    assert_eq!(committed, 1, "exactly one racing plan must be committable");
    assert_eq!(
        superseded,
        THREADS - 1,
        "every other racing plan must be cleanly superseded, not stuck or panicking"
    );
    assert_eq!(store.generation().unwrap(), 1);
    assert_eq!(store.live_count().unwrap(), 1);
}

/// Race a `close()` against a `commit()` on the very same plan handle. The
/// arbiter lock makes these mutually exclusive: whichever acquires it first
/// determines the sole real effect (a receipt written, or a close with none)
/// and the other observes that plan's actual terminal disposition — never
/// both effects, and never an inconsistent generation.
#[test]
fn core_import_concurrency_racing_close_and_commit_on_one_plan_apply_exactly_once() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let plan = store
        .inspect(&bundle(&[signed(&a, 2, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap();

    let barrier = Barrier::new(2);
    let (commit_result, close_result) = std::thread::scope(|scope| {
        let commit_handle = {
            let plan = &plan;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                plan.commit()
            })
        };
        let close_handle = {
            let plan = &plan;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                plan.close()
            })
        };
        (commit_handle.join().unwrap(), close_handle.join().unwrap())
    });

    match (commit_result, close_result) {
        (Ok(CommitOutcome::Committed(_)), Err(SessionError::PlanConsumed)) => {
            assert_eq!(store.generation().unwrap(), 1, "commit won: one increment");
            assert_eq!(store.live_count().unwrap(), 1);
        }
        (Err(SessionError::PlanClosed), Ok(())) => {
            assert_eq!(store.generation().unwrap(), 0, "close won: no state change");
            assert_eq!(store.live_count().unwrap(), 0);
        }
        other => panic!("exactly one of {{commit wins, close wins}} must hold: {other:?}"),
    }
}

/// Race `inspect()` (which atomically replaces the live preview/plan) against
/// `commit()` on a plan from the preview being replaced. Either the commit
/// wins the lock first — and the *subsequent* inspect must snapshot the
/// post-commit generation as its `base_generation` — or the inspect wins
/// first — and the commit must then see its parent preview already consumed.
/// A torn read (inspect basing itself on a generation the commit is still
/// mid-write on) is exactly what the arbiter must prevent.
#[test]
fn core_import_concurrency_racing_inspect_and_commit_never_observes_a_torn_generation() {
    let a = author();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let plan = store
        .inspect(&bundle(&[signed(&a, 3, 1)]), ctx())
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap();

    let barrier = Barrier::new(2);
    let (commit_result, inspect_outcome) = std::thread::scope(|scope| {
        let commit_handle = {
            let plan = &plan;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                plan.commit()
            })
        };
        let inspect_handle = {
            let store = &store;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                store
                    .inspect(&bundle(&[signed(&a, 4, 2)]), ctx())
                    .unwrap()
                    .expect_preview()
            })
        };
        (
            commit_handle.join().unwrap(),
            inspect_handle.join().unwrap(),
        )
    });

    // Whichever interleaving occurred, the new preview's plan_all() must
    // succeed against the store's *actual current* generation: a torn read
    // would make this plan/commit fail with StalePreview.
    let post_receipt = inspect_outcome.plan_all().unwrap().commit().unwrap();
    assert!(matches!(post_receipt, CommitOutcome::Committed(_)));

    match commit_result {
        Ok(CommitOutcome::Committed(_)) => {
            assert_eq!(
                store.generation().unwrap(),
                2,
                "commit then inspect's own commit: two increments total"
            );
        }
        Err(SessionError::PreviewConsumed) => {
            assert_eq!(
                store.generation().unwrap(),
                1,
                "inspect won first: only its own later commit incremented generation"
            );
        }
        other => panic!("racing commit must either succeed or see PreviewConsumed: {other:?}"),
    }
}
