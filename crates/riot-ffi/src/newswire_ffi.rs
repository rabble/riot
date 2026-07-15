//! Newswire FFI surface: typed records and MobileProfile methods that let
//! native apps create newswire spaces, posts, and editorial actions, and
//! project the collective view (front page, open wire, editorial history).
//!
//! All creation delegates to `riot_core::newswire::create_signed_*` using
//! the profile's own author, then imports the signed bytes through the same
//! preview/plan/commit boundary as every other entry — so newswire records
//! are bound by the same byte budgets and admission checks.

use std::collections::BTreeMap;

use riot_core::newswire::{
    create_signed_editorial_action, create_signed_news_post, create_signed_space_descriptor,
    project_space, AlertProfileV1, EditorialActionKind, EditorialActionV1, NewsPostV1,
    OperationalProfileV1, ProjectionClockV1, RequestKind, RequestProfileV1, SignedNewswireRecord,
    SpaceDescriptorV1,
};
use riot_core::profile::path::SUBSPACE_ID_BYTES;
use riot_core::profile::resolver::{key_tag, resolve_display_names, sanitize_display_name};

use crate::mobile_api::{AlertCertainty, AlertSeverity, AlertUrgency, MobileError, MobileProfile};
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
    /// The founding editorial roster, as hex-encoded 32-byte subspace ids. An
    /// EMPTY roster keeps the historical default: the founder is the sole
    /// editor. A non-empty roster is used verbatim — the founder is an editor
    /// only if their own key is in it. Rotation stays out of scope; this is the
    /// one-time founding selection the community makes.
    pub editorial_roster: Vec<String>,
}

/// Which alert dimension a value describes — the same closed vocabulary the
/// core model freezes.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireAlertProfile {
    pub urgency: AlertUrgency,
    pub severity: AlertSeverity,
    pub certainty: AlertCertainty,
    pub valid_from_unix_seconds: Option<u64>,
}

/// Whether a request offers help or needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NewswireRequestKind {
    Need,
    Offer,
}

/// A mutual-aid request or offer carried on a post.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireRequestProfile {
    pub kind: NewswireRequestKind,
    pub needed_by_unix_seconds: Option<u64>,
    pub contact_instructions: String,
}

/// The optional operational overlay on a post. Selecting one makes the model's
/// stricter fields (expiry, location, source claims) mandatory — see the core
/// `validate_post` rules.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NewswireOperationalProfile {
    Alert { profile: NewswireAlertProfile },
    Request { profile: NewswireRequestProfile },
}

/// Input for creating a freeform news post.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswirePostInput {
    pub space_descriptor_entry_id: String,
    pub headline: String,
    pub body: String,
    pub language: String,
    pub event_time_unix_seconds: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
    pub coarse_location: Option<String>,
    pub source_claims: Vec<String>,
    pub operational_profile: Option<NewswireOperationalProfile>,
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

/// A rendered actor — a post's author or an action's editor — carried as the
/// structured `{display_name, tag}` pair plus the pre-rendered string, never a
/// raw hex key posing as a name. `id` is the stable hex subspace id, for
/// pinning and re-resolution; `rendered` is the only form safe to show bare.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireAuthor {
    pub id: String,
    pub display_name: String,
    pub tag: String,
    pub rendered: String,
}

/// A projected post in the collective view. `headline`, `body`,
/// `coarse_location`, `source_claims` and `operational_profile` are `None`/empty
/// on a Hidden or Tombstoned row; identity, ordering and freshness metadata
/// survive so the row stays accountable.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireProjectedPost {
    pub entry_id: String,
    pub author: NewswireAuthor,
    /// The Willow ordering key the open wire is sorted by (newest first),
    /// surfaced so a client can merge projections without re-deriving it.
    pub tai_j2000_micros: u64,
    pub headline: Option<String>,
    pub body: Option<String>,
    pub language: String,
    pub coarse_location: Option<String>,
    pub event_time_unix_seconds: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
    pub source_claims: Vec<String>,
    pub operational_profile: Option<NewswireOperationalProfile>,
    pub ai_assisted: bool,
    pub verification_ids: Vec<String>,
    pub correction_ids: Vec<String>,
    pub treatment: NewswirePostTreatment,
}

/// A projected editorial action in the collective history — every signed act an
/// editor took, active or since retracted.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireProjectedEditorialAction {
    pub entry_id: String,
    pub signer: NewswireAuthor,
    pub tai_j2000_micros: u64,
    pub target_entry_id: String,
    pub kind: NewswireEditorialActionKind,
    pub reason: Option<String>,
    /// A correction's replacement text. Redacted to `None` when the correction's
    /// target is itself hidden or tombstoned.
    pub correction_text: Option<String>,
    /// Whether the action currently has effect. A retracted feature and the
    /// retraction that undid it both remain here; only their `active` differs.
    pub active: bool,
}

/// A known contributor to a community newswire space: a rendered author, whether
/// they are the space's recognized organizer, and how many signed records they
/// are behind. This is the People surface — derived from the community's signed
/// records, NOT a membership roster and NOT presence.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireContributor {
    /// The rendered author — `{display_name, tag}` plus the pre-rendered
    /// string, never a raw key posing as a name.
    pub author: NewswireAuthor,
    /// True iff this author is the recognized organizer — the single coordinate
    /// where the author id equals the space's namespace id. Never a self-claim.
    pub is_organizer: bool,
    /// How many signed records (news posts plus editorial actions) this author
    /// is behind in the current projection.
    pub contribution_count: u32,
}

/// The full collective projection of a newswire space: everything a client
/// needs to derive the identical front page, open wire, and editorial history
/// as its peers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireProjectionView {
    pub open_wire: Vec<NewswireProjectedPost>,
    pub front_page: Vec<NewswireProjectedPost>,
    /// Posts whose expiry has passed — off the open wire, still readable.
    pub earlier: Vec<NewswireProjectedPost>,
    pub editorial_history: Vec<NewswireProjectedEditorialAction>,
    /// Entry ids of records whose timestamp is implausibly far in the future,
    /// held out of the collective view pending a plausible clock.
    pub future_quarantine: Vec<String>,
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
            // An empty founding roster keeps the historical default — the
            // founder alone. A caller-supplied roster is used verbatim, so the
            // founding collective chooses its own editors.
            let editorial_roster = if input.editorial_roster.is_empty() {
                vec![signer_id]
            } else {
                input
                    .editorial_roster
                    .iter()
                    .map(|key| parse_entry_id(key))
                    .collect::<Result<Vec<_>, _>>()?
            };
            let descriptor = SpaceDescriptorV1 {
                namespace_id,
                name: input.name,
                summary: input.summary,
                languages: input.languages,
                geographic_tags: input.geographic_tags,
                topic_tags: input.topic_tags,
                editorial_roster,
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
                event_time_unix_seconds: input.event_time_unix_seconds,
                expires_at_unix_seconds: input.expires_at_unix_seconds,
                coarse_location: input.coarse_location,
                source_claims: input.source_claims,
                operational_profile: input.operational_profile.map(operational_profile_to_core),
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
            // Resolve every known name ONCE, then render each author against it.
            // A rename repairs every row that person ever touched.
            let names = resolve_display_names(&profile.store).map_err(|_| MobileError::Internal)?;
            let render = |author_id: &[u8; SUBSPACE_ID_BYTES]| render_author(&names, author_id);
            Ok(NewswireProjectionView {
                open_wire: projection
                    .open_wire
                    .iter()
                    .map(|post| projected_post_view(post, &render))
                    .collect(),
                front_page: projection
                    .front_page
                    .iter()
                    .map(|post| projected_post_view(post, &render))
                    .collect(),
                earlier: projection
                    .earlier
                    .iter()
                    .map(|post| projected_post_view(post, &render))
                    .collect(),
                editorial_history: projection
                    .editorial_history
                    .iter()
                    .map(|action| projected_action_view(action, &render))
                    .collect(),
                future_quarantine: projection
                    .future_quarantine
                    .iter()
                    .map(|entry_id| hex(entry_id))
                    .collect(),
            })
        })
    }

    /// Projects the Known-contributors (People) surface for a newswire space:
    /// every distinct author of a signed record it holds, each rendered by the
    /// same sanctioned name path as a post author, with the recognized organizer
    /// marked by the namespace coordinate. Derived from the community's records,
    /// so every client sees the identical surface — not a roster, not presence.
    pub fn project_newswire_contributors(
        &self,
        space_descriptor_entry_id: String,
    ) -> Result<Vec<NewswireContributor>, MobileError> {
        with_active(&self.inner, |profile| {
            let descriptor_id = parse_entry_id(&space_descriptor_entry_id)?;
            let clock = ProjectionClockV1::system().map_err(|_| MobileError::ClockUnavailable)?;
            let rows =
                riot_core::newswire::contributors_for_space(&profile.store, descriptor_id, clock)
                    .map_err(map_newswire_store_error)?;
            let names = resolve_display_names(&profile.store).map_err(|_| MobileError::Internal)?;
            Ok(rows
                .iter()
                .map(|row| NewswireContributor {
                    author: render_author(&names, &row.author_id),
                    is_organizer: row.is_organizer,
                    contribution_count: row.contribution_count,
                })
                .collect())
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

/// Renders one actor against the names resolved for this projection, reusing
/// the profile crate's sanctioned rendering path so a newswire author is drawn
/// exactly as they are everywhere else. The name is SANITIZED and the tag is
/// key-derived; the two are also handed over separately so a native renderer
/// can lay them out, never a raw key posing as a name.
fn render_author(
    names: &BTreeMap<[u8; SUBSPACE_ID_BYTES], String>,
    author_id: &[u8; SUBSPACE_ID_BYTES],
) -> NewswireAuthor {
    let display_name = sanitize_display_name(names.get(author_id).map(String::as_str));
    let tag = key_tag(author_id);
    NewswireAuthor {
        id: hex(author_id),
        rendered: format!("{display_name} · {tag}"),
        display_name,
        tag,
    }
}

fn projected_post_view(
    post: &riot_core::newswire::ProjectedPost,
    render: &impl Fn(&[u8; SUBSPACE_ID_BYTES]) -> NewswireAuthor,
) -> NewswireProjectedPost {
    let treatment = match &post.treatment {
        riot_core::newswire::PostTreatment::Ordinary => NewswirePostTreatment::Ordinary,
        riot_core::newswire::PostTreatment::Hidden { .. } => NewswirePostTreatment::Hidden,
        riot_core::newswire::PostTreatment::Tombstoned { .. } => NewswirePostTreatment::Tombstoned,
    };
    NewswireProjectedPost {
        entry_id: hex(&post.entry_id),
        author: render(&post.author_id),
        tai_j2000_micros: post.tai_j2000_micros,
        headline: post.headline.clone(),
        body: post.body.clone(),
        language: post.language.clone(),
        coarse_location: post.coarse_location.clone(),
        event_time_unix_seconds: post.event_time_unix_seconds,
        expires_at_unix_seconds: post.expires_at_unix_seconds,
        source_claims: post.source_claims.clone(),
        operational_profile: post
            .operational_profile
            .as_ref()
            .map(operational_profile_from_core),
        ai_assisted: post.ai_assisted,
        verification_ids: post.verification_ids.iter().map(|id| hex(id)).collect(),
        correction_ids: post.correction_ids.iter().map(|id| hex(id)).collect(),
        treatment,
    }
}

fn projected_action_view(
    action: &riot_core::newswire::ProjectedEditorialAction,
    render: &impl Fn(&[u8; SUBSPACE_ID_BYTES]) -> NewswireAuthor,
) -> NewswireProjectedEditorialAction {
    NewswireProjectedEditorialAction {
        entry_id: hex(&action.entry_id),
        signer: render(&action.signer_id),
        tai_j2000_micros: action.tai_j2000_micros,
        target_entry_id: hex(&action.target_entry_id),
        kind: editorial_kind_from_core(action.kind),
        reason: action.reason.clone(),
        correction_text: action.correction_text.clone(),
        active: action.active,
    }
}

fn editorial_kind_from_core(kind: EditorialActionKind) -> NewswireEditorialActionKind {
    match kind {
        EditorialActionKind::Feature => NewswireEditorialActionKind::Feature,
        EditorialActionKind::Verify => NewswireEditorialActionKind::Verify,
        EditorialActionKind::Correct => NewswireEditorialActionKind::Correct,
        EditorialActionKind::Hide => NewswireEditorialActionKind::Hide,
        EditorialActionKind::Tombstone => NewswireEditorialActionKind::Tombstone,
        EditorialActionKind::Retract => NewswireEditorialActionKind::Retract,
    }
}

fn operational_profile_to_core(profile: NewswireOperationalProfile) -> OperationalProfileV1 {
    match profile {
        NewswireOperationalProfile::Alert { profile } => {
            OperationalProfileV1::Alert(AlertProfileV1 {
                urgency: urgency_to_core(profile.urgency),
                severity: severity_to_core(profile.severity),
                certainty: certainty_to_core(profile.certainty),
                valid_from_unix_seconds: profile.valid_from_unix_seconds,
            })
        }
        NewswireOperationalProfile::Request { profile } => {
            OperationalProfileV1::Request(RequestProfileV1 {
                kind: match profile.kind {
                    NewswireRequestKind::Need => RequestKind::Need,
                    NewswireRequestKind::Offer => RequestKind::Offer,
                },
                needed_by_unix_seconds: profile.needed_by_unix_seconds,
                contact_instructions: profile.contact_instructions,
            })
        }
    }
}

fn operational_profile_from_core(profile: &OperationalProfileV1) -> NewswireOperationalProfile {
    match profile {
        OperationalProfileV1::Alert(alert) => NewswireOperationalProfile::Alert {
            profile: NewswireAlertProfile {
                urgency: urgency_from_core(alert.urgency),
                severity: severity_from_core(alert.severity),
                certainty: certainty_from_core(alert.certainty),
                valid_from_unix_seconds: alert.valid_from_unix_seconds,
            },
        },
        OperationalProfileV1::Request(request) => NewswireOperationalProfile::Request {
            profile: NewswireRequestProfile {
                kind: match request.kind {
                    RequestKind::Need => NewswireRequestKind::Need,
                    RequestKind::Offer => NewswireRequestKind::Offer,
                },
                needed_by_unix_seconds: request.needed_by_unix_seconds,
                contact_instructions: request.contact_instructions.clone(),
            },
        },
    }
}

fn urgency_to_core(value: AlertUrgency) -> riot_core::model::Urgency {
    use riot_core::model::Urgency;
    match value {
        AlertUrgency::Immediate => Urgency::Immediate,
        AlertUrgency::Expected => Urgency::Expected,
        AlertUrgency::Future => Urgency::Future,
        AlertUrgency::Past => Urgency::Past,
        AlertUrgency::Unknown => Urgency::Unknown,
    }
}

fn urgency_from_core(value: riot_core::model::Urgency) -> AlertUrgency {
    use riot_core::model::Urgency;
    match value {
        Urgency::Immediate => AlertUrgency::Immediate,
        Urgency::Expected => AlertUrgency::Expected,
        Urgency::Future => AlertUrgency::Future,
        Urgency::Past => AlertUrgency::Past,
        Urgency::Unknown => AlertUrgency::Unknown,
    }
}

fn severity_to_core(value: AlertSeverity) -> riot_core::model::Severity {
    use riot_core::model::Severity;
    match value {
        AlertSeverity::Extreme => Severity::Extreme,
        AlertSeverity::Severe => Severity::Severe,
        AlertSeverity::Moderate => Severity::Moderate,
        AlertSeverity::Minor => Severity::Minor,
        AlertSeverity::Unknown => Severity::Unknown,
    }
}

fn severity_from_core(value: riot_core::model::Severity) -> AlertSeverity {
    use riot_core::model::Severity;
    match value {
        Severity::Extreme => AlertSeverity::Extreme,
        Severity::Severe => AlertSeverity::Severe,
        Severity::Moderate => AlertSeverity::Moderate,
        Severity::Minor => AlertSeverity::Minor,
        Severity::Unknown => AlertSeverity::Unknown,
    }
}

fn certainty_to_core(value: AlertCertainty) -> riot_core::model::Certainty {
    use riot_core::model::Certainty;
    match value {
        AlertCertainty::Observed => Certainty::Observed,
        AlertCertainty::Likely => Certainty::Likely,
        AlertCertainty::Possible => Certainty::Possible,
        AlertCertainty::Unlikely => Certainty::Unlikely,
        AlertCertainty::Unknown => Certainty::Unknown,
    }
}

fn certainty_from_core(value: riot_core::model::Certainty) -> AlertCertainty {
    use riot_core::model::Certainty;
    match value {
        Certainty::Observed => AlertCertainty::Observed,
        Certainty::Likely => AlertCertainty::Likely,
        Certainty::Possible => AlertCertainty::Possible,
        Certainty::Unlikely => AlertCertainty::Unlikely,
        Certainty::Unknown => AlertCertainty::Unknown,
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
