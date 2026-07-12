# Headless Multi-Node Test Network Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic, invisible five-node Riot integration-test network that proves public alert and checklist app-data writes propagate, relay, remain partition-isolated, and converge through the public mobile sync API.

**Architecture:** A test-only `TestNode` wraps an isolated `MobileProfile`; `TestNetwork` creates one shared namespace, clones one generated app fixture into every node, and exposes normalized public-read snapshots. A bounded pair pump exchanges only opaque frames between public `MobileSyncSession` handles, auto-accepts reviews only between named same-namespace test peers, records metadata-only reports, and explicitly cancels both sessions on every error path.

**Tech Stack:** Rust 2021, `riot-ffi` public API, `riot-core` app codecs in integration-test fixtures, Cargo integration tests, Clippy, rustfmt, Tarpaulin.

---

## File Map

- Create `crates/riot-ffi/tests/support/mod.rs`: test-only module boundary and re-exports.
- Create `crates/riot-ffi/tests/support/test_network.rs`: node/app fixtures, snapshots, sync reports/errors, bounded pair pump, and quiescence runner.
- Create `crates/riot-ffi/tests/multi_node_network.rs`: local-write, five-node partition/heal, forced-cap cleanup, and foreign-namespace tests.
- No production files, feature flags, existing tests, or UniFFI exports change.

### Task 1: Establish local node writes and normalized reads

**Files:**
- Create: `crates/riot-ffi/tests/support/mod.rs`
- Create: `crates/riot-ffi/tests/support/test_network.rs`
- Create: `crates/riot-ffi/tests/multi_node_network.rs`

- [ ] **Step 1: Write the failing local-write contract**

Create the integration-test entry point with the test-only module and a test that requires the fixture API:

```rust
mod support;

use support::{AppFixture, TestNetwork};

#[test]
fn five_nodes_write_alerts_and_checklist_data_locally() {
    let fixture = AppFixture::generate();
    let network = TestNetwork::five_nodes(fixture).expect("five-node network");

    for (index, node) in network.nodes().iter().enumerate() {
        let snapshot = node.snapshot().expect("local snapshot");
        assert_eq!(snapshot.alerts.len(), 1, "node {index} alert write");
        assert_eq!(snapshot.app_items.len(), 1, "node {index} app write");
        node.assert_point_reads_match(&snapshot)
            .expect("list and point reads agree");
    }
}
```

- [ ] **Step 2: Run the test and observe RED**

Run:

```bash
cargo test -p riot-ffi --test multi_node_network five_nodes_write_alerts_and_checklist_data_locally -- --nocapture
```

Expected: compilation fails because `support`, `AppFixture`, and `TestNetwork` do not exist.

- [ ] **Step 3: Add the support module boundary**

Create `support/mod.rs` with test-crate-only exports:

```rust
mod test_network;

pub(crate) use test_network::{AppFixture, TestNetwork};
```

- [ ] **Step 4: Implement the node fixture and public-read projection**

In `support/test_network.rs`, define:

```rust
use std::sync::Arc;

use riot_ffi::{
    open_local_profile, AlertCertainty, AlertDraftInput, AlertSeverity,
    AlertUrgency, MobileError, MobileProfile, PublicSpace,
};

#[derive(Clone)]
pub(crate) struct AppFixture {
    pub(crate) manifest_bytes: Vec<u8>,
    pub(crate) bundle_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlertSnapshot {
    pub(crate) entry_id: String,
    pub(crate) namespace_id: String,
    pub(crate) signer_id: String,
    pub(crate) headline: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeSnapshot {
    pub(crate) alerts: Vec<AlertSnapshot>,
    pub(crate) app_items: Vec<(String, Vec<u8>)>,
}

pub(crate) struct TestNode {
    pub(crate) name: &'static str,
    pub(crate) profile: Arc<MobileProfile>,
    pub(crate) space: PublicSpace,
    pub(crate) app_id: String,
}

pub(crate) struct TestNetwork {
    nodes: Vec<TestNode>,
}
```

Implement `AppFixture::generate()` with the same production codecs used by the
existing app contract:

```rust
impl AppFixture {
    pub(crate) fn generate() -> Self {
        use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
        use riot_core::apps::manifest::{encode_manifest, AppManifest};
        use riot_core::willow::generate_communal_author;

        let author = generate_communal_author().expect("app author");
        let manifest = AppManifest {
            name: "Checklist".into(),
            description: "Lets people add and check off shared to-dos.".into(),
            version: "1.0.0".into(),
            author: author.identity(),
            permissions: vec!["own-app-data".into()],
            entry_point: "index.html".into(),
        };
        let bundle = AppBundle {
            entry_point: "index.html".into(),
            resources: vec![AppResource {
                path: "index.html".into(),
                content_type: "text/html".into(),
                bytes: b"<html>checklist</html>".to_vec(),
            }],
        };
        Self {
            manifest_bytes: encode_manifest(&manifest).expect("encode manifest"),
            bundle_bytes: encode_app_bundle(&bundle).expect("encode bundle"),
        }
    }
}
```

`TestNetwork::five_nodes` must create A, create the public space, join B–E to
that exact `PublicSpace`, clone the exact fixture bytes into every `install_app`
call, assert the five returned app IDs are equal, then call `write_local_fact`
once per node.

Use these exact write helpers:

```rust
fn alert_input(name: &str) -> AlertDraftInput {
    AlertDraftInput {
        valid_from: None,
        expires_at: u64::MAX - 1,
        language: "en".into(),
        urgency: AlertUrgency::Immediate,
        severity: AlertSeverity::Severe,
        certainty: AlertCertainty::Observed,
        headline: format!("Network alert from node {name}"),
        description: format!("Test-network write from node {name}"),
        affected_area_claim: None,
        source_claims: vec![format!("node-{name}")],
        ai_assisted: false,
    }
}

fn app_key(name: &str) -> String {
    format!("items/node-{}", name.to_ascii_lowercase())
}

fn app_value(name: &str) -> Vec<u8> {
    format!("checklist value from node {name}").into_bytes()
}
```

`TestNode::snapshot()` must call only `list_current_entries()` and
`app_data_list(app_id, "items")` (no trailing slash: Riot rejects empty path
segments), map alerts to `AlertSnapshot`, sort alerts by full `entry_id`, map
app rows to `(key, value)`, and sort them by exact key.
`assert_point_reads_match()` must call `app_data_get` for every listed item and
return `Err(MobileError::Internal)` on disagreement; alert membership is checked
from the public list because no point-read alert API exists.

- [ ] **Step 5: Run the focused test and observe GREEN**

Run the Task 1 command again.

Expected: `five_nodes_write_alerts_and_checklist_data_locally ... ok`.

- [ ] **Step 6: Commit Task 1 only**

```bash
git add crates/riot-ffi/tests/support/mod.rs \
  crates/riot-ffi/tests/support/test_network.rs \
  crates/riot-ffi/tests/multi_node_network.rs
git diff --cached --name-only
git commit -m "test: add headless Riot node fixtures"
```

Before committing, the cached-name output must contain exactly those three paths.

### Task 2: Reconcile a partitioned five-node network to quiescence

**Files:**
- Modify: `crates/riot-ffi/tests/support/test_network.rs`
- Modify: `crates/riot-ffi/tests/support/mod.rs`
- Modify: `crates/riot-ffi/tests/multi_node_network.rs`

- [ ] **Step 1: Write the failing partition/heal test**

Add a test that uses exact edges and origin facts:

```rust
#[test]
fn partitioned_five_node_network_heals_relays_and_converges() {
    let network = TestNetwork::five_nodes(AppFixture::generate()).expect("network");
    let left = [Edge::new(0, 1), Edge::new(1, 2)];
    let right = [Edge::new(3, 4)];

    network.run_until_quiescent(&left, 10).expect("left partition");
    network.run_until_quiescent(&right, 10).expect("right partition");
    network.assert_partition_counts(&[3, 3, 3, 2, 2]).expect("partition counts");
    network.assert_components_isolated().expect("partition isolation");

    let connected = [
        Edge::new(0, 1), Edge::new(1, 2), Edge::new(2, 3), Edge::new(3, 4),
    ];
    let report = network
        .run_until_quiescent(&connected, 10)
        .expect("healed network convergence");
    assert!(report.rounds.len() <= 10);
    network.assert_all_expected_facts().expect("all five reads everywhere");
    network.assert_distinct_full_signers().expect("five distinct full signer IDs");
    network.assert_origin_relayed("A", "E").expect("A reached E");
    network.assert_origin_relayed("E", "A").expect("E reached A");

    let before = network.snapshots().expect("before no-op round");
    let no_op = network.run_one_round(&connected, false).expect("no-op round");
    assert!(no_op.pairs.iter().all(PairSyncReport::already_current));
    assert_eq!(network.snapshots().expect("after no-op round"), before);
}
```

- [ ] **Step 2: Run the new test and observe RED**

Run:

```bash
cargo test -p riot-ffi --test multi_node_network partitioned_five_node_network_heals_relays_and_converges -- --nocapture
```

Expected: compilation fails on missing `Edge`, report, pair pump, and quiescence methods.

- [ ] **Step 3: Define metadata-only sync contracts**

Add exact report/error types to `support/test_network.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Edge { pub(crate) left: usize, pub(crate) right: usize }

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct PairSyncReport {
    pub(crate) left_name: &'static str,
    pub(crate) right_name: &'static str,
    pub(crate) left_to_right_frames: usize,
    pub(crate) right_to_left_frames: usize,
    pub(crate) left_reviews: usize,
    pub(crate) right_reviews: usize,
    pub(crate) left_accepts: usize,
    pub(crate) right_accepts: usize,
    pub(crate) transitions: usize,
    pub(crate) left_terminal: bool,
    pub(crate) right_terminal: bool,
    pub(crate) left_terminal_kind: Option<riot_ffi::SyncOutcomeKind>,
    pub(crate) right_terminal_kind: Option<riot_ffi::SyncOutcomeKind>,
}

impl PairSyncReport {
    pub(crate) fn already_current(&self) -> bool {
        self.left_reviews + self.right_reviews + self.left_accepts + self.right_accepts == 0
    }
}

#[derive(Debug)]
pub(crate) enum SyncErrorCause {
    Mobile(MobileError),
    Invariant(&'static str),
    TransitionLimit { limit: usize },
}

#[derive(Debug)]
pub(crate) struct SyncError {
    pub(crate) cause: SyncErrorCause,
    pub(crate) edge: (&'static str, &'static str),
    pub(crate) transition: usize,
    // One pair for pair-pump errors; every last-round pair for round-cap errors.
    pub(crate) partial_pairs: Vec<PairSyncReport>,
    pub(crate) cleanup_errors: Vec<(&'static str, MobileError)>,
}

pub(crate) struct RoundReport { pub(crate) pairs: Vec<PairSyncReport> }
pub(crate) struct NetworkRunReport { pub(crate) rounds: Vec<RoundReport> }
```

Then expand `support/mod.rs` only after those types exist:

```rust
mod test_network;

pub(crate) use test_network::{
    AppFixture, Edge, NetworkRunReport, NodeSnapshot, PairSyncReport, SyncError,
    SyncErrorCause, TestNetwork, TestNode,
};
```

Do not store opaque frames, import bundles, app values, payloads, or random IDs in any report/error.

- [ ] **Step 4: Implement the bounded pair pump**

Implement `sync_pair(left, right)` as `pump_pair(left, right, 16)`. `pump_pair` must:

1. Require non-empty stable peer names before opening sessions. Record whether
   their full namespace IDs match, but allow a mismatch to reach the public
   protocol so the negative test observes its real `MobileError::InvalidInput`.
2. Open left, then right; if right open fails, cancel left before returning.
3. Count successful `begin`, `receive_frame`, and `accept_import` calls as transitions; do not count `take_outbound_frame` or cleanup.
4. Call `begin` only on left. For each outcome:
   - `ReviewImport`: require the recorded full namespace IDs to match, increment that endpoint's review counter, require `import_bundle_bytes.is_some()`, call `accept_import`, increment accepts, and process the returned outcome.
   - `FrameReady`: require and take its queued frame, count its direction, mark terminal from `outcome.terminal`, record `FrameReady` as its terminal kind when terminal, and deliver the frame even when terminal.
   - `Complete`: require `terminal == true`, mark that endpoint terminal, record `Complete` as its terminal kind, and queue no frame.
   - `Rejected`: return a structured invariant error because this all-accept harness never intentionally rejects.
5. Continue until both endpoints are terminal. Before every counted API call, fail with `TransitionLimit` if the next count exceeds `max_transitions`.
6. Run all internal checks through `Result`, not `assert!` or `panic!`.
7. On every error, best-effort `cancel()` both handles and append cleanup failures without replacing the primary cause.
8. On success, open and cancel a fresh session on each profile to prove release.

The implementation may use a local `Endpoint` enum and one queued `(Endpoint, Vec<u8>)`; frame bytes live only on the stack and never enter reports.

- [ ] **Step 5: Implement alternating rounds and assertions**

`run_one_round(edges, reverse)` clones the edge order, reverses it when
requested, calls `sync_pair` for each, and prints one deterministic line per
edge containing only round/edge labels and numeric counters.
`run_until_quiescent` alternates forward/reverse order by round and returns
after a complete round where every pair is `already_current`; it returns
`TransitionLimit { limit: max_rounds }` with every `PairSyncReport` from the
last complete round in `SyncError.partial_pairs` if no quiescent round appears.

Implement assertions by comparing `NodeSnapshot` values and the recorded expected full alert IDs/signers plus exact app keys/values. `assert_components_isolated` must prove A–C lack D/E facts and D–E lack A/B/C facts before the bridge. `assert_origin_relayed("A", "E")` and its reverse check both the origin alert entry ID and origin app key/value on the destination.
`assert_distinct_full_signers` must collect the five authored alert signer IDs,
assert every ID has the public API's full 64-hex-character representation,
sort/deduplicate them, and require exactly five unique values.

- [ ] **Step 6: Run the partition test and observe GREEN**

Run the Task 2 command again.

Expected: the partition and connected rounds complete, all five nodes converge, and the final round reports zero reviews/accepts.

- [ ] **Step 7: Commit Task 2 only**

```bash
git add crates/riot-ffi/tests/support/mod.rs \
  crates/riot-ffi/tests/support/test_network.rs \
  crates/riot-ffi/tests/multi_node_network.rs
git diff --cached --name-only
git commit -m "test: prove five-node Riot convergence"
```

### Task 3: Prove failure isolation and session recovery

**Files:**
- Modify: `crates/riot-ffi/tests/support/test_network.rs`
- Modify: `crates/riot-ffi/tests/multi_node_network.rs`

- [ ] **Step 1: Write the forced-cap RED test**

```rust
#[test]
fn transition_cap_cancels_both_sessions_without_mutation() {
    let network = TestNetwork::five_nodes(AppFixture::generate()).expect("network");
    let before = network.snapshots().expect("before cap");
    let error = support::pump_pair(&network.nodes()[0], &network.nodes()[1], 1)
        .expect_err("begin consumes transition one; delivery exceeds cap");
    assert!(matches!(error.cause, SyncErrorCause::TransitionLimit { limit: 1 }));
    let partial = &error.partial_pairs[0];
    assert_eq!(partial.left_reviews + partial.right_reviews, 0);
    assert_eq!(partial.left_accepts + partial.right_accepts, 0);
    assert_eq!(network.snapshots().expect("after cap"), before);
    network.assert_sessions_reopen(&[0, 1]).expect("sessions released");
}
```

Run:

```bash
cargo test -p riot-ffi --test multi_node_network transition_cap_cancels_both_sessions_without_mutation -- --nocapture
```

Expected: RED until `pump_pair` is re-exported `pub(crate)` and cap cleanup/recovery is observable.

- [ ] **Step 2: Make forced-cap cleanup GREEN**

Re-export `pump_pair` from `support/mod.rs`. Ensure `begin()` consumes transition 1, the limit check fires before initial-frame delivery, both sessions are canceled, and `assert_sessions_reopen` opens then cancels a new session for each requested node.

Run the Step 1 command; expected: PASS.

- [ ] **Step 3: Write the seeded foreign-namespace RED test**

```rust
#[test]
fn foreign_namespace_is_rejected_without_reads_or_writes() {
    let fixture = AppFixture::generate();
    let network = TestNetwork::five_nodes(fixture.clone()).expect("network");
    let foreign = TestNode::foreign("F", fixture).expect("seeded foreign node");
    let local_before = network.nodes()[0].snapshot().expect("local before");
    let foreign_before = foreign.snapshot().expect("foreign before");

    let error = support::pump_pair(&network.nodes()[0], &foreign, 16)
        .expect_err("foreign namespace must fail");
    assert!(matches!(error.cause, SyncErrorCause::Mobile(MobileError::InvalidInput)));
    let partial = &error.partial_pairs[0];
    assert_eq!(partial.left_reviews + partial.right_reviews, 0);
    assert_eq!(partial.left_accepts + partial.right_accepts, 0);
    assert_eq!(network.nodes()[0].snapshot().expect("local after"), local_before);
    assert_eq!(foreign.snapshot().expect("foreign after"), foreign_before);
    network.nodes()[0].assert_point_reads_match(&local_before).expect("local points");
    foreign.assert_point_reads_match(&foreign_before).expect("foreign points");
    network.assert_sessions_reopen(&[0]).expect("local released");
    foreign.assert_session_reopens().expect("foreign released");
}
```

Run:

```bash
cargo test -p riot-ffi --test multi_node_network foreign_namespace_is_rejected_without_reads_or_writes -- --nocapture
```

Expected: RED until `TestNode::foreign` creates its own public space, installs cloned app bytes, signs the exact F alert fixture, writes `items/node-f`, and the pump preserves the underlying `MobileError::InvalidInput`.

- [ ] **Step 4: Make namespace isolation GREEN**

Implement `TestNode::foreign` with the same app fixture but a distinct space and the exact shared `write_local_fact("F")` path. Remove any pre-pump same-namespace rejection from `pump_pair` for this explicitly negative test: auto-accept remains guarded by checking full namespaces immediately before any `accept_import`; the actual opaque hello exchange must surface `MobileError::InvalidInput`. Preserve the partial zero-review/zero-accept report and cancel both sessions on return.

Run the Step 3 command; expected: PASS with both non-empty snapshots unchanged.

- [ ] **Step 5: Run all three network behaviors together**

```bash
cargo test -p riot-ffi --test multi_node_network -- --nocapture
```

Expected: all four tests pass; output contains only stable node/edge labels and counts, never opaque bytes, values, or random IDs.

- [ ] **Step 6: Commit Task 3 only**

```bash
git add crates/riot-ffi/tests/support/mod.rs \
  crates/riot-ffi/tests/support/test_network.rs \
  crates/riot-ffi/tests/multi_node_network.rs
git diff --cached --name-only
git commit -m "test: cover multi-node sync failure recovery"
```

### Task 4: Run repository quality and coverage gates

**Files:**
- Modify only if a gate reveals a defect: the three new test files above.

- [ ] **Step 1: Ensure the mandated coverage command is installed**

Run:

```bash
cargo tarpaulin --version || cargo install cargo-tarpaulin --locked
cargo tarpaulin --version
```

Expected: `cargo-tarpaulin` reports its installed version. This setup step was
explicitly authorized after the plan gate found the local command missing.

- [ ] **Step 2: Format and verify formatting**

```bash
cargo fmt --all
cargo fmt --all -- --check
```

Expected: check exits 0 with no diff.

- [ ] **Step 3: Run the focused integration suite**

```bash
cargo test -p riot-ffi --test multi_node_network -- --nocapture
```

Expected: all local-write, convergence, forced-cap, and namespace tests pass.

- [ ] **Step 4: Run the full workspace test gate**

```bash
cargo test --workspace --all-features
```

Expected: all workspace tests pass.

- [ ] **Step 5: Run strict Clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: exit 0 with no warnings.

- [ ] **Step 6: Run the source-of-truth coverage gate**

```bash
cargo tarpaulin --fail-under 100
```

Expected: exit 0 at or above the 100% line threshold configured in `.coverage-thresholds.json`; success, namespace-error, and forced-cap harness branches have direct tests even if Tarpaulin excludes integration-test support from instrumentation.

- [ ] **Step 7: Inspect scope and commit formatting-only changes if needed**

```bash
git status --short
git diff --check
git diff --cached --name-only
```

Expected: no production files changed and no unrelated user/concurrent changes staged. If rustfmt changed one of the three new files, stage only those exact paths and commit:

```bash
git add crates/riot-ffi/tests/support/mod.rs \
  crates/riot-ffi/tests/support/test_network.rs \
  crates/riot-ffi/tests/multi_node_network.rs
git commit -m "style: format multi-node sync tests"
```
