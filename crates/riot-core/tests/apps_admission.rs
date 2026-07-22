use riot_core::apps::admission::{
    preflight, AdmissionOutcome, MAX_AGGREGATE_PAIR_BYTES, MAX_INSTALLED_APPS,
};

// Held pairs are (already_installed_app_ids_len, aggregate_bytes). preflight
// takes the current held count + held aggregate pair bytes and the prospective
// pair's (app_id_already_held, pair_bytes) and returns an outcome. No I/O.

#[test]
fn accepts_when_under_both_limits() {
    assert_eq!(preflight(31, 1000, false, 1000), AdmissionOutcome::Admit);
}

#[test]
fn count_boundary_31_to_32_admits_but_32_to_33_refuses_on_count() {
    assert_eq!(preflight(31, 0, false, 10), AdmissionOutcome::Admit);
    assert_eq!(preflight(32, 0, false, 10), AdmissionOutcome::RefuseCount);
}

#[test]
fn aggregate_byte_boundary_is_exact() {
    // Exactly 3 MiB total admits; one over refuses on bytes.
    assert_eq!(
        preflight(1, MAX_AGGREGATE_PAIR_BYTES - 10, false, 10),
        AdmissionOutcome::Admit
    );
    assert_eq!(
        preflight(1, MAX_AGGREGATE_PAIR_BYTES - 10, false, 11),
        AdmissionOutcome::RefuseBytes
    );
}

#[test]
fn reinstalling_a_held_id_is_idempotent_and_adds_no_bytes() {
    // An already-held ID neither increments count nor adds pair bytes, even at
    // the ceilings — idempotent restoration, not new admission.
    assert_eq!(
        preflight(32, MAX_AGGREGATE_PAIR_BYTES, true, 999_999),
        AdmissionOutcome::Admit
    );
}

#[test]
fn count_is_checked_before_bytes() {
    // Over on both: count wins so callers can map distinct copy deterministically.
    assert_eq!(
        preflight(32, MAX_AGGREGATE_PAIR_BYTES + 1, false, 1),
        AdmissionOutcome::RefuseCount
    );
}

#[test]
fn impossible_hostile_count_fails_closed_without_overflowing() {
    assert_eq!(
        preflight(usize::MAX, 0, false, 1),
        AdmissionOutcome::RefuseCount
    );
}

#[test]
fn limits_match_spec() {
    assert_eq!(MAX_INSTALLED_APPS, 32);
    assert_eq!(MAX_AGGREGATE_PAIR_BYTES, 3 * 1024 * 1024);
}
