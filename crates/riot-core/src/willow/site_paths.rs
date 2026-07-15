//! Reserved path regions for a composite-site owned masthead namespace `O`.
//!
//! `/manifest`         — the signed site manifest (Unit 2), never delegated.
//! `/articles/<sect>/` — editorial articles; the ONLY region delegated to editors.
//! `/mod/`             — moderation records (Unit 3), never delegated to editors.

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

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: `Path::from_slices` returns `Result<Path, PathError>` in willow25
    // 0.6.0-alpha.3 — every call site `.expect(...)`s it.
    #[test]
    fn articles_path_is_under_articles_but_manifest_is_not() {
        let article = Path::from_slices(&[ARTICLES_COMPONENT, b"news", b"post-1"]).expect("path");
        let manifest = Path::from_slices(&[MANIFEST_COMPONENT]).expect("path");
        assert!(is_under_articles(&article), "article path must be under /articles");
        assert!(!is_under_articles(&manifest), "manifest path must NOT be under /articles");
    }

    #[test]
    fn empty_and_mod_paths_are_not_under_articles() {
        let empty = Path::from_slices(&[]).expect("path");
        let moderation = Path::from_slices(&[MOD_COMPONENT, b"revoke-1"]).expect("path");
        assert!(!is_under_articles(&empty));
        assert!(!is_under_articles(&moderation));
    }
}
