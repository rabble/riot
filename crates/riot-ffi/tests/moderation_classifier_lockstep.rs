//! Composite-site Unit 3 — Task 6: the FFI classifier lockstep guard.
//!
//! `mobile_state.rs` splits records alert-vs-non-alert in MULTIPLE places
//! (`list_current_entries`, `inspectable_entries`, `reproject_active`). A new
//! owned record family added to only SOME of them either bricks the board (a
//! rejecting classifier) or silently mis-processes (a tolerant one) — the
//! newswire-0B failure mode. The call-site count already grew 2 -> 3 since the
//! memory note, so this guard does NOT hardcode a count: it asserts that
//! `is_owned_moderation_entry` (Unit 3) appears at EVERY site that classifies
//! `is_owned_editorial_entry` (Unit 1). If a future refactor adds a fourth
//! classifier for editorial, this fails until moderation is added there too.

const SOURCE: &str = include_str!("../src/mobile_state.rs");

/// Count non-comment source lines that CALL a predicate (a `::name(` invocation),
/// so a doc-comment mention (`/// ... is_owned_editorial_entry ...`) is not
/// miscounted as a call site.
fn call_site_count(needle: &str) -> usize {
    SOURCE
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && line.contains(&format!("{needle}("))
        })
        .count()
}

#[test]
fn moderation_is_classified_at_every_editorial_classifier_site() {
    let editorial = call_site_count("is_owned_editorial_entry");
    let moderation = call_site_count("is_owned_moderation_entry");

    assert!(
        editorial >= 3,
        "expected at least the 3 known editorial classifier sites \
         (list_current_entries, inspectable_entries, reproject_active); found {editorial}. \
         If this dropped, the grep or the classifiers changed — investigate."
    );
    assert_eq!(
        moderation, editorial,
        "classifier drift: is_owned_editorial_entry has {editorial} call sites but \
         is_owned_moderation_entry has {moderation}. Every owned-editorial classifier \
         MUST also classify owned-moderation, or a /mod/ record bricks or mis-processes \
         the board (newswire-0B). Add is_owned_moderation_entry beside the missing one."
    );
}
