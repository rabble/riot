//! Contract: the FFI `CommunityRelationship` enum carries the additive
//! `Following` and `Personal` variants (Spaces-first Rung 1). Seam-free, so it
//! lives as an integration test.

use riot_ffi::CommunityRelationship;

#[test]
fn community_relationship_has_following_and_personal() {
    let _ = CommunityRelationship::Following;
    let _ = CommunityRelationship::Personal;
}
