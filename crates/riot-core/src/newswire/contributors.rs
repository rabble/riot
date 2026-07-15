//! The Known-contributors projection: a community's People surface, derived
//! deterministically from the signed records it already holds.
//!
//! This is NOT a membership roster and NOT presence. A contributor is any
//! distinct author of a signed newswire record in the space — the author of a
//! news post or the signer of an editorial action. The row carries only the
//! author coordinate, a content-derived contribution count, and whether the
//! author is the space's recognized organizer.
//!
//! The organizer is marked by ONE rule and no other: the coordinate identity
//! `author_id == namespace_id`. A communal space binds the organizer's subspace
//! id to the namespace id (see `willow::identity::generate_space_organizer_author`),
//! so every client derives the same organizer with no extra record and nothing
//! a member self-claims can promote them. The name a person shows is resolved
//! and rendered at the FFI boundary; the core deals only in the coordinate.

use std::collections::BTreeMap;

use super::projection::NewswireProjection;

/// One known contributor to a community newswire space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContributorRowV1 {
    /// The author's 32-byte subspace id. Rendered to a name + key tag at the
    /// display boundary; never shown raw.
    pub author_id: [u8; 32],
    /// True iff this author is the space's recognized organizer — the single
    /// coordinate where `author_id == namespace_id`.
    pub is_organizer: bool,
    /// How many signed records in this projection this author is behind: news
    /// posts plus editorial actions. It is the evidence that the surface is
    /// derived from content, not from a roster.
    pub contribution_count: u32,
}

/// Derives the contributor set for a projected space. Every author of a
/// projected post (open wire and earlier, including redacted rows whose author
/// stays accountable) and every signer in the editorial history contributes.
/// Rows are ordered deterministically: the organizer first, then ascending by
/// author id, so identical records produce an identical People surface on every
/// client regardless of arrival order.
pub fn contributors(
    projection: &NewswireProjection,
    namespace_id: [u8; 32],
) -> Vec<ContributorRowV1> {
    let mut counts: BTreeMap<[u8; 32], u32> = BTreeMap::new();
    for post in projection.open_wire.iter().chain(projection.earlier.iter()) {
        *counts.entry(post.author_id).or_insert(0) += 1;
    }
    for action in &projection.editorial_history {
        *counts.entry(action.signer_id).or_insert(0) += 1;
    }

    let mut rows: Vec<ContributorRowV1> = counts
        .into_iter()
        .map(|(author_id, contribution_count)| ContributorRowV1 {
            author_id,
            is_organizer: author_id == namespace_id,
            contribution_count,
        })
        .collect();
    rows.sort_by(|left, right| {
        right
            .is_organizer
            .cmp(&left.is_organizer)
            .then_with(|| left.author_id.cmp(&right.author_id))
    });
    rows
}
