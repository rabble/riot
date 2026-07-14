//! Newswire FFI surface: typed records and MobileProfile methods that let
//! native apps create newswire spaces, posts, and editorial actions, and
//! project the collective view (front page, open wire, editorial history).
//!
//! All creation delegates to `riot_core::newswire::create_signed_*` using
//! the profile's own author, then imports the signed bytes through the same
//! preview/plan/commit boundary as every other entry — so newswire records
//! are bound by the same byte budgets and admission checks.

use riot_core::newswire::{
    create_signed_editorial_action, create_signed_news_post, create_signed_space_descriptor,
    project_space, EditorialActionKind, EditorialActionV1, NewsPostV1, ProjectionClockV1,
    SignedNewswireRecord, SpaceDescriptorV1,
};

use crate::mobile_api::{MobileError, MobileProfile};
use crate::mobile_state::{hex, with_active};

/// A signed newswire record returned to the native caller: the entry ID
/// (hex) and the raw signed bytes suitable for sync, sharing, or gateway
/// rendering.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireSignedRecord {
    pub entry_id: String,
    pub signed_bytes: Vec<u8>,
}

/// Input for creating a new community newswire space descriptor.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireSpaceInput {
    pub name: String,
    pub summary: String,
    pub languages: Vec<String>,
    pub geographic_tags: Vec<String>,
    pub topic_tags: Vec<String>,
}

/// Input for creating a freeform news post.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswirePostInput {
    pub space_descriptor_entry_id: String,
    pub headline: String,
    pub body: String,
    pub language: String,
    pub coarse_location: Option<String>,
    pub source_claims: Vec<String>,
    pub ai_assisted: bool,
}

/// The kind of editorial action: feature, verify, correct, hide, tombstone, retract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NewswireEditorialActionKind {
    Feature,
    Verify,
    Correct,
    Hide,
    Tombstone,
    Retract,
}

/// Input for creating an editorial action on an existing post.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireEditorialActionInput {
    pub space_descriptor_entry_id: String,
    pub target_entry_id: String,
    pub kind: NewswireEditorialActionKind,
    pub reason: Option<String>,
    pub correction_text: Option<String>,
}

/// How a projected post is being treated by active editorial actions.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NewswirePostTreatment {
    Ordinary,
    Hidden,
    Tombstoned,
}

/// A projected post in the collective view.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireProjectedPost {
    pub entry_id: String,
    pub author_id: String,
    pub body: Option<String>,
    pub source_claims: Vec<String>,
    pub treatment: NewswirePostTreatment,
}

/// The full collective projection of a newswire space.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireProjectionView {
    pub open_wire: Vec<NewswireProjectedPost>,
    pub front_page: Vec<NewswireProjectedPost>,
}

#[uniffi::export]
impl MobileProfile {
    /// Creates and signs a newswire space descriptor using this profile's
    /// organizer author, then imports it into the store. The descriptor
    /// establishes the community space and its editorial roster (this
    /// profile's signer is the founding editor).
    pub fn create_newswire_space(
        &self,
        input: NewswireSpaceInput,
    ) -> Result<NewswireSignedRecord, MobileError> {
        with_active(&self.inner, |profile| {
            let namespace_id = profile.author.identity().namespace_id;
            let signer_id = *profile.author.subspace_id().as_bytes();
            let descriptor = SpaceDescriptorV1 {
                namespace_id,
                name: input.name,
                summary: input.summary,
                languages: input.languages,
                geographic_tags: input.geographic_tags,
                topic_tags: input.topic_tags,
                editorial_roster: vec![signer_id],
                predecessor: None,
                successor: None,
            };
            let signed = create_signed_space_descriptor(&profile.author, descriptor)
                .map_err(map_newswire_error)?;
            import_signed_newswire(profile, &signed)
        })
    }

    /// Creates and signs a freeform news post in the named space. The
    /// space descriptor must already be in the store (created or imported).
    pub fn create_newswire_post(
        &self,
        input: NewswirePostInput,
    ) -> Result<NewswireSignedRecord, MobileError> {
        with_active(&self.inner, |profile| {
            let descriptor_id = parse_entry_id(&input.space_descriptor_entry_id)?;
            let descriptor =
                riot_core::newswire::load_space_descriptor(&profile.store, descriptor_id)
                    .map_err(map_newswire_store_error)?;
            let post = NewsPostV1 {
                space_descriptor_entry_id: descriptor_id,
                headline: input.headline,
                body: input.body,
                language: input.language,
                event_time_unix_seconds: None,
                expires_at_unix_seconds: None,
                coarse_location: input.coarse_location,
                source_claims: input.source_claims,
                operational_profile: None,
                ai_assisted: input.ai_assisted,
            };
            let signed = create_signed_news_post(&profile.author, &descriptor, post)
                .map_err(map_newswire_error)?;
            import_signed_newswire(profile, &signed)
        })
    }

    /// Creates and signs an editorial action (feature, verify, correct, hide,
    /// tombstone, retract) targeting an existing post. Only recognized editors
    /// (in the descriptor's editorial roster) may author actions.
    pub fn create_newswire_editorial_action(
        &self,
        input: NewswireEditorialActionInput,
    ) -> Result<NewswireSignedRecord, MobileError> {
        with_active(&self.inner, |profile| {
            let descriptor_id = parse_entry_id(&input.space_descriptor_entry_id)?;
            let target_id = parse_entry_id(&input.target_entry_id)?;
            let descriptor =
                riot_core::newswire::load_space_descriptor(&profile.store, descriptor_id)
                    .map_err(map_newswire_store_error)?;
            let kind = match input.kind {
                NewswireEditorialActionKind::Feature => EditorialActionKind::Feature,
                NewswireEditorialActionKind::Verify => EditorialActionKind::Verify,
                NewswireEditorialActionKind::Correct => EditorialActionKind::Correct,
                NewswireEditorialActionKind::Hide => EditorialActionKind::Hide,
                NewswireEditorialActionKind::Tombstone => EditorialActionKind::Tombstone,
                NewswireEditorialActionKind::Retract => EditorialActionKind::Retract,
            };
            let action = EditorialActionV1 {
                space_descriptor_entry_id: descriptor_id,
                target_entry_id: target_id,
                kind,
                reason: input.reason,
                correction_text: input.correction_text,
            };
            let signed = create_signed_editorial_action(&profile.author, &descriptor, action)
                .map_err(map_newswire_error)?;
            import_signed_newswire(profile, &signed)
        })
    }

    /// Projects the collective view (front page + open wire) for a newswire
    /// space, derived from all verified records currently in the store.
    pub fn project_newswire_space(
        &self,
        space_descriptor_entry_id: String,
    ) -> Result<NewswireProjectionView, MobileError> {
        with_active(&self.inner, |profile| {
            let descriptor_id = parse_entry_id(&space_descriptor_entry_id)?;
            let clock = ProjectionClockV1::system().map_err(|_| MobileError::ClockUnavailable)?;
            let projection = project_space(&profile.store, descriptor_id, clock)
                .map_err(map_newswire_store_error)?;
            Ok(NewswireProjectionView {
                open_wire: projection
                    .open_wire
                    .iter()
                    .map(projected_post_view)
                    .collect(),
                front_page: projection
                    .front_page
                    .iter()
                    .map(projected_post_view)
                    .collect(),
            })
        })
    }
}

/// Imports a signed newswire record into the store via the standard
/// preview/plan/commit path, then returns the entry ID + signed bytes.
fn import_signed_newswire(
    profile: &mut crate::mobile_state::LocalProfile,
    signed: &SignedNewswireRecord,
) -> Result<NewswireSignedRecord, MobileError> {
    let bundle_bytes = riot_core::import::encode_bundle(std::slice::from_ref(&signed.signed))
        .map_err(|_| MobileError::Internal)?;
    profile.preview = None;
    profile.plan = None;
    let preview =
        crate::mobile_state::inspect_core(&profile.store, &bundle_bytes, "local-newswire-sign")?;
    let plan = preview.plan_all().map_err(map_core_error_inner)?;
    use riot_core::session::CommitOutcome;
    match plan.commit().map_err(map_core_error_inner)? {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
    Ok(NewswireSignedRecord {
        entry_id: hex(&signed.entry_id),
        signed_bytes: bundle_bytes,
    })
}

fn projected_post_view(post: &riot_core::newswire::ProjectedPost) -> NewswireProjectedPost {
    let treatment = match &post.treatment {
        riot_core::newswire::PostTreatment::Ordinary => NewswirePostTreatment::Ordinary,
        riot_core::newswire::PostTreatment::Hidden { .. } => NewswirePostTreatment::Hidden,
        riot_core::newswire::PostTreatment::Tombstoned { .. } => NewswirePostTreatment::Tombstoned,
    };
    NewswireProjectedPost {
        entry_id: hex(&post.entry_id),
        author_id: hex(&post.author_id),
        body: post.body.clone(),
        source_claims: post.source_claims.clone(),
        treatment,
    }
}

fn parse_entry_id(hex_str: &str) -> Result<[u8; 32], MobileError> {
    let bytes = hex_decode(hex_str).ok_or(MobileError::InvalidInput)?;
    if bytes.len() == 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    } else {
        Err(MobileError::InvalidInput)
    }
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

fn map_newswire_error(error: riot_core::newswire::NewswireError) -> MobileError {
    let _ = error;
    MobileError::InvalidInput
}

fn map_newswire_store_error(error: riot_core::newswire::NewswireStoreError) -> MobileError {
    let _ = error;
    MobileError::Internal
}

fn map_core_error_inner(error: riot_core::session::SessionError) -> MobileError {
    let _ = error;
    MobileError::Internal
}
