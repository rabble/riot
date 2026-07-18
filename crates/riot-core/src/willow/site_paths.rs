//! Reserved path regions for a composite-site owned masthead namespace `O`.
//!
//! `/manifest`          — the signed site manifest (Unit 2), never delegated.
//! `/articles/<sect>/`  — editorial articles; the ONLY region delegated to editors.
//! `/mod/`              — moderation records (Unit 3), never delegated to editors.
//! `/directory/listing` — the community listing record (anchor network); the
//!                        ONLY listable coordinate. `/directory` is delegatable
//!                        to a dedicated listing key, disjoint from every region
//!                        above.

use willow25::entry::Entry;
use willow25::groupings::{Keylike, Namespaced};
use willow25::paths::Path;

/// First path component of the editorial region.
pub const ARTICLES_COMPONENT: &[u8] = b"articles";
/// First path component of the reserved manifest record.
pub const MANIFEST_COMPONENT: &[u8] = b"manifest";
/// First path component of the moderation region.
pub const MOD_COMPONENT: &[u8] = b"mod";
/// First path component of the reserved listing/directory region.
pub const DIRECTORY_COMPONENT: &[u8] = b"directory";
/// Second path component of the community listing record (`/directory/listing`).
pub const LISTING_COMPONENT: &[u8] = b"listing";

/// True iff `path`'s first component is exactly `articles` (the delegatable region).
/// A delegated editor cap's granted area path MUST satisfy this; `/manifest` and
/// `/mod/` (and the empty/root path) must not, so they can never be delegated.
pub fn is_under_articles(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|first| first.as_ref() == ARTICLES_COMPONENT)
}

/// True iff `path`'s first component is exactly `directory` (the delegatable
/// listing region). A dedicated listing delegate cap's granted area path MUST
/// satisfy this; `/manifest`, `/mod/`, `/articles`, and the empty/root path must
/// not, so a listing delegation can never reach them and an editorial cap rooted
/// under `/articles` can never authorize a listing.
pub fn is_under_directory(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|first| first.as_ref() == DIRECTORY_COMPONENT)
}

/// True iff `path` is exactly `/directory/listing` — the one listable coordinate.
/// Authority over arbitrary `/directory` children is NOT interpreted as a new
/// listing record; the admission layer requires this exact two-component path.
pub fn is_directory_listing(path: &Path) -> bool {
    let mut components = path.components();
    matches!(
        (components.next(), components.next(), components.next()),
        (Some(first), Some(second), None)
            if first.as_ref() == DIRECTORY_COMPONENT && second.as_ref() == LISTING_COMPONENT
    )
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
}
