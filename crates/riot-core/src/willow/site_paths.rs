//! Reserved path regions for a composite-site owned masthead namespace `O`.
//!
//! `/manifest`         — the signed site manifest (Unit 2), never delegated.
//! `/articles/<sect>/` — editorial articles; the ONLY region delegated to editors.
//! `/mod/`             — moderation records (Unit 3), never delegated to editors.

use willow25::entry::Entry;
use willow25::groupings::{Keylike, Namespaced};
use willow25::paths::Path;

/// First path component of the editorial region.
pub const ARTICLES_COMPONENT: &[u8] = b"articles";
/// First path component of the reserved manifest record.
pub const MANIFEST_COMPONENT: &[u8] = b"manifest";
/// First path component of the moderation region.
pub const MOD_COMPONENT: &[u8] = b"mod";

/// True iff `path`'s first component is exactly `articles` (the delegatable region).
/// A delegated editor cap's granted area path MUST satisfy this; `/manifest` and
/// `/mod/` (and the empty/root path) must not, so they can never be delegated.
pub fn is_under_articles(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|first| first.as_ref() == ARTICLES_COMPONENT)
}

/// True iff `entry` is an owned composite-site editorial entry — its namespace
/// is owned AND its path is under `/articles/`. This is the opaque editorial
/// family admitted in Unit 1: the path is the identity, the payload opaque
/// (integrity via digest/length). Every admission and classification gate that
/// must treat owned editorial entries as a first-class local family (session
/// path-binding, the FFI alert/non-alert classifiers) routes through this one
/// predicate so they cannot drift.
pub fn is_owned_editorial_entry(entry: &Entry) -> bool {
    Namespaced::namespace_id(entry).is_owned() && is_under_articles(Keylike::path(entry))
}

/// True iff `path`'s first component is exactly `mod` (the moderation region).
/// A delegated moderator cap's granted area path MUST satisfy this; `/manifest`
/// and `/articles/` (and the empty/root path) must not, so a moderator cap can
/// never reach the manifest or the site root.
pub fn is_under_mod(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|first| first.as_ref() == MOD_COMPONENT)
}

/// True iff `entry` is an owned composite-site moderation entry — its namespace
/// is owned AND its path is under `/mod/`. The moderation analogue of
/// `is_owned_editorial_entry`: every admission and classification gate that must
/// treat owned moderation records (`revoke`/`tombstone`/`mod_epoch`) as a
/// first-class local family routes through this one predicate so they cannot
/// drift across the (multiple) FFI classifier call sites.
pub fn is_owned_moderation_entry(entry: &Entry) -> bool {
    Namespaced::namespace_id(entry).is_owned() && is_under_mod(Keylike::path(entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: `Path::from_slices` returns `Result<Path, PathError>` in willow25
    // 0.6.0-alpha.3 — every call site `.expect(...)`s it.
    #[test]
    fn articles_path_is_under_articles_but_manifest_is_not() {
        let article = Path::from_slices(&[ARTICLES_COMPONENT, b"news", b"post-1"]).expect("path");
        let manifest = Path::from_slices(&[MANIFEST_COMPONENT]).expect("path");
        assert!(
            is_under_articles(&article),
            "article path must be under /articles"
        );
        assert!(
            !is_under_articles(&manifest),
            "manifest path must NOT be under /articles"
        );
    }

    #[test]
    fn empty_and_mod_paths_are_not_under_articles() {
        let empty = Path::from_slices(&[]).expect("path");
        let moderation = Path::from_slices(&[MOD_COMPONENT, b"revoke-1"]).expect("path");
        assert!(!is_under_articles(&empty));
        assert!(!is_under_articles(&moderation));
    }

    #[test]
    fn mod_path_is_under_mod_but_articles_and_manifest_are_not() {
        let moderation = Path::from_slices(&[MOD_COMPONENT, b"revoke-1"]).expect("path");
        let article = Path::from_slices(&[ARTICLES_COMPONENT, b"news", b"post-1"]).expect("path");
        let manifest = Path::from_slices(&[MANIFEST_COMPONENT]).expect("path");
        assert!(is_under_mod(&moderation), "mod path must be under /mod");
        assert!(
            !is_under_mod(&article),
            "article path must NOT be under /mod"
        );
        assert!(
            !is_under_mod(&manifest),
            "manifest path must NOT be under /mod"
        );
    }

    #[test]
    fn empty_root_path_is_not_under_mod() {
        let empty = Path::from_slices(&[]).expect("path");
        assert!(
            !is_under_mod(&empty),
            "empty/root path must NOT be under /mod"
        );
    }
}
