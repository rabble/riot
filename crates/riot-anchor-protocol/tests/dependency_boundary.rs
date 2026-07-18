//! Structural guard: `riot-anchor-protocol` must stay the dependency-neutral
//! canonical wire layer.
//!
//! The design (System Shape) requires this crate to depend on `riot-core` with
//! default features disabled and on nothing that pulls SQLite, HTTP, an async
//! runtime, a P2P transport, FFI, or a server adapter. This test enforces that
//! in two independent layers:
//!
//!   1. `direct_dependencies_are_an_allowlist` — checks this crate's OWN manifest
//!      dependencies against a small allowlist and asserts the `riot-core` edge
//!      keeps `default-features = false`. Trivially correct; catches the most
//!      likely regression (someone adds `tokio`/`iroh` directly, or flips the
//!      feature back on).
//!   2. `no_forbidden_package_in_transitive_closure` — walks the resolved
//!      dependency graph from this crate with a feature-aware closure that honors
//!      `optional` deps and `default-features = false`, so workspace feature
//!      unification (which turns `riot-core/sqlite` on for the FFI target) does
//!      NOT create a false positive here. The walk under-follows nothing it
//!      should follow: it processes each package with the union of all feature
//!      sets that reach it.
//!
//! Both parse `cargo metadata` rather than a fixed dependency list, so they
//! track the real graph.

use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::process::Command;

const CRATE: &str = "riot-anchor-protocol";

/// Packages that must never appear in this crate's normal-dependency closure.
/// Storage engines, async runtimes, P2P transport, HTTP servers/clients, FFI.
const FORBIDDEN: &[&str] = &[
    "rusqlite",
    "libsqlite3-sys",
    "iroh",
    "iroh-base",
    "tokio",
    "hyper",
    "http-body-util",
    "rustls",
    "tokio-rustls",
    "tower",
    "tower-http",
    "uniffi",
    "riot-transport",
    "riot-client-net",
    "riot-ffi",
    "riot-anchor",
];

/// Direct normal dependencies this crate is allowed to declare. Keeping this an
/// explicit allowlist means any new direct dependency is a deliberate, reviewed
/// change to this test, not a silent graph expansion.
const DIRECT_ALLOWLIST: &[&str] = &["riot-core", "minicbor", "blake3"];

fn cargo_metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata json")
}

fn packages(meta: &Value) -> &Vec<Value> {
    meta["packages"].as_array().expect("packages array")
}

fn find_package<'a>(meta: &'a Value, name: &str) -> &'a Value {
    packages(meta)
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("package `{name}` not found in cargo metadata"))
}

/// Normal (non-dev, non-build) manifest dependencies of a package.
fn normal_deps(pkg: &Value) -> impl Iterator<Item = &Value> {
    pkg["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["kind"].is_null())
}

#[test]
fn direct_dependencies_are_an_allowlist() {
    let meta = cargo_metadata();
    let me = find_package(&meta, CRATE);

    for dep in normal_deps(me) {
        let name = dep["name"].as_str().expect("dep name");
        assert!(
            DIRECT_ALLOWLIST.contains(&name),
            "`{CRATE}` gained an unapproved direct dependency `{name}`. If this is \
             intentional, add it to DIRECT_ALLOWLIST in this test (and confirm it \
             does not pull a forbidden crate)."
        );
        if name == "riot-core" {
            assert_eq!(
                dep["uses_default_features"],
                Value::Bool(false),
                "`{CRATE}` must depend on riot-core with default-features = false so \
                 the client SQLite feature never enters this crate's graph."
            );
        }
    }
}

#[test]
fn no_forbidden_package_in_transitive_closure() {
    let meta = cargo_metadata();

    // id -> package manifest value
    let mut pkg_by_id: HashMap<&str, &Value> = HashMap::new();
    for p in packages(&meta) {
        pkg_by_id.insert(p["id"].as_str().unwrap(), p);
    }
    // id -> resolve node (gives the name->resolved-id edges for this package)
    let mut node_by_id: HashMap<&str, &Value> = HashMap::new();
    for n in meta["resolve"]["nodes"].as_array().expect("resolve nodes") {
        node_by_id.insert(n["id"].as_str().unwrap(), n);
    }

    let start_id = find_package(&meta, CRATE)["id"].as_str().unwrap();

    // Feature-aware closure. Process each package with the union of every feature
    // set that reaches it; re-enqueue if a later path brings new features. This
    // is monotone (feature sets only grow) so it terminates, and it never skips a
    // dep an active feature would enable — soundness against false negatives.
    let mut features_seen: HashMap<&str, HashSet<String>> = HashMap::new();
    let mut reachable: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<(&str, HashSet<String>)> = VecDeque::new();

    // The start crate itself is built with its default features on.
    queue.push_back((start_id, HashSet::from(["default".to_string()])));

    while let Some((id, incoming)) = queue.pop_front() {
        let entry = features_seen.entry(id).or_default();
        let before = entry.len();
        for f in &incoming {
            entry.insert(f.clone());
        }
        // Skip re-processing only if this path added nothing new.
        if entry.len() == before && before != 0 && reachable.contains(pkg_name(&pkg_by_id, id)) {
            continue;
        }
        let active: HashSet<String> = entry.clone();

        let pkg = pkg_by_id[id];
        let name = pkg["name"].as_str().unwrap();
        reachable.insert(name.to_string());
        assert!(
            !FORBIDDEN.contains(&name),
            "`{CRATE}` transitively depends on forbidden crate `{name}`. The anchor \
             protocol crate must stay free of storage/HTTP/async-runtime/transport/FFI \
             dependencies (design: System Shape)."
        );

        let enabled_optional = expand_features(pkg, &active);
        let node = node_by_id[id];

        for dep in normal_deps(pkg) {
            let dep_name = dep["name"].as_str().unwrap();
            // The name used for the implicit feature / `dep:` token / resolve edge
            // is the rename if present, else the crate name.
            let edge_key = dep["rename"].as_str().unwrap_or(dep_name);
            let optional = dep["optional"].as_bool().unwrap_or(false);
            let enabled =
                !optional || enabled_optional.contains(edge_key) || active.contains(edge_key);
            if !enabled {
                continue;
            }
            // Resolve this dep to a concrete package id via the resolve node.
            let Some(child_id) = resolve_edge(node, edge_key, dep_name) else {
                continue;
            };
            let mut child_features: HashSet<String> = HashSet::new();
            if dep["uses_default_features"].as_bool().unwrap_or(true) {
                child_features.insert("default".to_string());
            }
            for f in dep["features"].as_array().into_iter().flatten() {
                if let Some(s) = f.as_str() {
                    child_features.insert(s.to_string());
                }
            }
            queue.push_back((child_id, child_features));
        }
    }

    // Vacuous-pass guard: the walk must actually traverse, not silently no-op.
    assert!(
        reachable.contains("riot-core"),
        "closure walk did not reach riot-core — the traversal is broken, so a green \
         result here would be meaningless."
    );
}

fn pkg_name<'a>(pkg_by_id: &HashMap<&'a str, &'a Value>, id: &str) -> &'a str {
    pkg_by_id[id]["name"].as_str().unwrap()
}

/// Expand a package's active feature set through its `features` map to the set of
/// optional-dependency names that become enabled. Handles `dep:x`, `x/feat`, and
/// weak `x?/feat` tokens, plus implicit same-name features, to a fixpoint.
fn expand_features(pkg: &Value, active: &HashSet<String>) -> HashSet<String> {
    let empty = serde_json::Map::new();
    let features = pkg["features"].as_object().unwrap_or(&empty);

    let mut active: HashSet<String> = active.clone();
    let mut enabled_optional: HashSet<String> = HashSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        let snapshot: Vec<String> = active.iter().cloned().collect();
        for f in snapshot {
            let Some(tokens) = features.get(&f).and_then(|v| v.as_array()) else {
                // `f` is not a declared feature: it may be the implicit feature of
                // an optional dependency of the same name → enable that dep.
                if enabled_optional.insert(f.clone()) {
                    changed = true;
                }
                continue;
            };
            for t in tokens.iter().filter_map(|t| t.as_str()) {
                if let Some(rest) = t.strip_prefix("dep:") {
                    if enabled_optional.insert(rest.to_string()) {
                        changed = true;
                    }
                } else if let Some((left, _sub)) = t.split_once('/') {
                    let weak = left.ends_with('?');
                    let dep = left.trim_end_matches('?');
                    if !weak && enabled_optional.insert(dep.to_string()) {
                        changed = true;
                    }
                } else if features.contains_key(t) {
                    if active.insert(t.to_string()) {
                        changed = true;
                    }
                } else if enabled_optional.insert(t.to_string()) {
                    // Not a declared feature → treat as an optional-dep implicit feature.
                    changed = true;
                }
            }
        }
    }
    enabled_optional
}

/// Find the resolved package id for a dependency edge, matching the resolve
/// node's `deps[].name` against the edge key (rename) or the crate name.
fn resolve_edge<'a>(node: &'a Value, edge_key: &str, crate_name: &str) -> Option<&'a str> {
    for d in node["deps"].as_array().into_iter().flatten() {
        let dn = d["name"].as_str().unwrap_or("");
        // resolve `deps[].name` uses the rename (underscored) when renamed; match
        // loosely on either the edge key or the crate name, normalizing dashes.
        if names_match(dn, edge_key) || names_match(dn, crate_name) {
            return d["pkg"].as_str();
        }
    }
    None
}

fn names_match(a: &str, b: &str) -> bool {
    a.replace('-', "_") == b.replace('-', "_")
}
