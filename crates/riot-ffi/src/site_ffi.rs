//! Composite-site owner FFI: create and restore an `OwnedMasthead`.
//!
//! Owner secret material NEVER crosses the FFI boundary unsealed. The caller
//! supplies a 32-byte wrapping key (used only in-process); what crosses back is
//! solely hex id `String`s plus an opaque sealed `Vec<u8>` (the AEAD envelope
//! produced by `OwnedMasthead::seal`). No `willow25` types, no plaintext root
//! or subspace secrets are exposed. The local key copy is zeroed before return.
//!
//! Unit 0 scope: create + restore only. Editor delegation is deferred to Unit 6.
//!
//! NOTE: `CreatedSite` is a new `uniffi::Record`. Native bindings must be
//! regenerated (and the staticlib rebuilt) before any native app can consume
//! these functions — that regen happens in Unit 6, not here.

use std::collections::BTreeSet;

use riot_core::newswire::PostTreatment;
use riot_core::site::{
    evaluate_freshness, item_treatment, read_moderation_record, resolve_degradation,
    resolve_trust_tier, validate_site_manifest, CompositeDegradation, DegradationInputs,
    HeldModerationRecord, MemberClassification, RequireTransport, SiteDisplay, SiteRole, SiteRule,
    SiteTransport, TrustTier, ValidatedManifest, VersionFloorOutcome,
};
use riot_core::willow::site_paths::{ARTICLES_COMPONENT, MOD_COMPONENT};
use riot_core::willow::{OwnedMasthead, Path, SignedWillowEntry};
use willow25::groupings::Keylike;

use crate::mobile_api::{MobileError, MobileProfile};
use crate::mobile_state::with_active;

/// Owner-side result of creating or restoring a composite site.
///
/// All fields are transport-safe: ids are lowercase hex, and `sealed_root` is
/// the opaque encrypted envelope — never plaintext secret material.
#[derive(uniffi::Record)]
pub struct CreatedSite {
    /// The owned namespace id (site root of trust), hex-encoded (64 chars).
    pub namespace_id: String,
    /// The owner's subspace id (receiver of the owner write capability), hex.
    pub owner_subspace_id: String,
    /// The sealed masthead envelope. Opaque to callers; persist as-is and pass
    /// back to `restore_owned_site`.
    pub sealed_root: Vec<u8>,
}

/// Lowercase hex encoding of a byte slice.
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

/// Coerce a caller-supplied key slice into an exact 32-byte array.
fn exact_key(k: &[u8]) -> Result<[u8; 32], MobileError> {
    <[u8; 32]>::try_from(k).map_err(|_| MobileError::InvalidInput)
}

/// Create a fresh composite site owned by the caller.
///
/// Generates a new `OwnedMasthead`, seals it under `wrapping_key`, and returns
/// the site's hex ids plus the opaque sealed root. The in-process key copy is
/// zeroed before returning.
#[uniffi::export]
pub fn create_owned_site(mut wrapping_key: Vec<u8>) -> Result<CreatedSite, MobileError> {
    let key = exact_key(&wrapping_key)?;
    let result = (|| {
        let masthead = OwnedMasthead::generate().map_err(|_| MobileError::InvalidInput)?;
        let sealed_root = masthead.seal(&key).map_err(|_| MobileError::InvalidInput)?;
        Ok(CreatedSite {
            namespace_id: hex(masthead.namespace_id().as_bytes()),
            owner_subspace_id: hex(masthead.owner_subspace_id().as_bytes()),
            sealed_root,
        })
    })();
    wrapping_key.iter_mut().for_each(|b| *b = 0);
    result
}

/// Restore a previously sealed composite site.
///
/// Opens `sealed_root` under `wrapping_key` and returns the site's hex ids,
/// echoing the same sealed root back. Fails if the key is wrong or the envelope
/// is malformed. The in-process key copy is zeroed before returning.
#[uniffi::export]
pub fn restore_owned_site(
    mut wrapping_key: Vec<u8>,
    sealed_root: Vec<u8>,
) -> Result<CreatedSite, MobileError> {
    let key = exact_key(&wrapping_key)?;
    let result = (|| {
        let masthead = OwnedMasthead::open_sealed(&key, &sealed_root)
            .map_err(|_| MobileError::InvalidInput)?;
        Ok(CreatedSite {
            namespace_id: hex(masthead.namespace_id().as_bytes()),
            owner_subspace_id: hex(masthead.owner_subspace_id().as_bytes()),
            sealed_root,
        })
    })();
    wrapping_key.iter_mut().for_each(|b| *b = 0);
    result
}

// ─── Task 5: resolved (validated) site-manifest record ───────────────────────
//
// NEW `uniffi::Record`s (`ResolvedSiteManifest`, `ResolvedSiteMember`) and a NEW
// `uniffi::Enum` (`ManifestValidationStatus`). Per the UniFFI gate, the native
// binding regen AND staticlib rebuild must land in the SAME commit as these
// types — a new record without the rebuild is a RUNTIME checksum abort in the
// apps, not a compile error. Native regen for the whole `site_ffi` surface
// (including Unit 0's `CreatedSite`) is coordinated centrally (Unit 6 / the
// coordinator), not run from this worktree.

/// The manifest-validation status Unit 2 produces. NOT the composite degradation
/// enum (Unit 4 owns that and folds this in) — only the manifest-derived states.
///
/// `ManifestRollbackAlarm` and `EquivocationAlarm` come from the durable version
/// floor (`riot_core::site::admit_manifest_version`), which needs the profile
/// database; the stateless [`resolve_site_manifest`] emits only `Valid` /
/// `MemberUnverified` / `ManifestInvalid`. The alarm states are attached by the
/// stateful composite resolver (Unit 4) that owns the store handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ManifestValidationStatus {
    /// Owner-signed, all members verified against their namespace key structure.
    Valid,
    /// Owner-signed, but at least one member dropped to unverified (inv 1); the
    /// rest of the site still resolves.
    MemberUnverified,
    /// Signer / root / signature validation failed — no trustworthy manifest.
    ManifestInvalid,
    /// A rollback or require-downgrade was refused by the durable version floor.
    ManifestRollbackAlarm,
    /// Two conflicting owner signatures at the same version — compromise alarm.
    EquivocationAlarm,
}

/// One resolved member. `verified` is invariant 1's classification (declared
/// rule class agrees with the namespace marker bit). `role`/`rule`/`display` are
/// CORE-resolved stable tokens the shells switch on, never re-parsed for policy.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResolvedSiteMember {
    /// Member namespace id, lowercase hex (64 chars).
    pub namespace_id: String,
    /// Stable role token: `masthead` / `comments` / `open-wire` / `unknown-<n>`.
    pub role: String,
    /// Stable rule token: `owned-write` / `communal-open` / `unknown-<n>`.
    pub rule: String,
    /// Stable display token: `front-articles` / `under-articles` / `wire-column`
    /// / `unknown-<n>`.
    pub display: String,
    /// True iff the declared rule class matches the namespace key structure.
    pub verified: bool,
}

/// The owner-signed manifest, validated and projected for the shells. On a
/// validation failure the record carries `status = ManifestInvalid`, empty
/// members, and `invalid_reason`, rather than throwing — degradation is a state.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResolvedSiteManifest {
    /// Site root (owned namespace id), lowercase hex (64 chars).
    pub root: String,
    /// Monotonic manifest version.
    pub version: u64,
    /// Members with per-member classification.
    pub members: Vec<ResolvedSiteMember>,
    /// Allowed transport tokens: `iroh` / `arti` / `unknown-<n>`.
    pub allow_transports: Vec<String>,
    /// Mandatory transport floor: `none` / `arti`.
    pub require_transport: String,
    /// Moderation path components, each lowercase hex (bytes may be non-UTF8).
    pub moderation_path: Vec<String>,
    /// The manifest-validation status.
    pub status: ManifestValidationStatus,
    /// Diagnostic reason when `status == ManifestInvalid`, else `None`.
    pub invalid_reason: Option<String>,
}

fn role_token(role: SiteRole) -> String {
    match role {
        SiteRole::Masthead => "masthead".to_string(),
        SiteRole::Comments => "comments".to_string(),
        SiteRole::OpenWire => "open-wire".to_string(),
        SiteRole::Unknown(code) => format!("unknown-{code}"),
    }
}

fn rule_token(rule: SiteRule) -> String {
    match rule {
        SiteRule::OwnedWrite => "owned-write".to_string(),
        SiteRule::CommunalOpen => "communal-open".to_string(),
        SiteRule::Unknown(code) => format!("unknown-{code}"),
    }
}

fn display_token(display: SiteDisplay) -> String {
    match display {
        SiteDisplay::FrontArticles => "front-articles".to_string(),
        SiteDisplay::UnderArticles => "under-articles".to_string(),
        SiteDisplay::WireColumn => "wire-column".to_string(),
        SiteDisplay::Unknown(code) => format!("unknown-{code}"),
    }
}

fn transport_token(transport: SiteTransport) -> String {
    match transport {
        SiteTransport::Iroh => "iroh".to_string(),
        SiteTransport::Arti => "arti".to_string(),
        SiteTransport::Unknown(code) => format!("unknown-{code}"),
    }
}

fn require_token(require: RequireTransport) -> String {
    match require {
        RequireTransport::None => "none".to_string(),
        RequireTransport::Arti => "arti".to_string(),
    }
}

fn project(validated: ValidatedManifest) -> ResolvedSiteManifest {
    let manifest = &validated.manifest;
    let status = if validated.all_members_verified() {
        ManifestValidationStatus::Valid
    } else {
        ManifestValidationStatus::MemberUnverified
    };
    let members = validated
        .members
        .iter()
        .map(|classified| ResolvedSiteMember {
            namespace_id: hex(&classified.member.ns),
            role: role_token(classified.member.role),
            rule: rule_token(classified.member.rule),
            display: display_token(classified.member.display),
            verified: classified.classification == MemberClassification::Verified,
        })
        .collect();
    ResolvedSiteManifest {
        root: hex(&manifest.root),
        version: manifest.version,
        members,
        allow_transports: manifest
            .transport_policy
            .allow
            .iter()
            .map(|t| transport_token(*t))
            .collect(),
        require_transport: require_token(manifest.transport_policy.require),
        moderation_path: manifest.moderation_path.iter().map(|c| hex(c)).collect(),
        status,
        invalid_reason: None,
    }
}

/// Validate an owner-signed site manifest (Unit 2 signer + member checks,
/// INDEPENDENT of admission) and project it for the shells.
///
/// `signature` must be 64 bytes and `followed_site_root` 32 bytes, else
/// `InvalidInput`. A validation *failure* is not an error: the returned record
/// carries `status = ManifestInvalid` so the app can render the degraded state.
#[uniffi::export]
pub fn resolve_site_manifest(
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature: Vec<u8>,
    payload_bytes: Vec<u8>,
    followed_site_root: Vec<u8>,
) -> Result<ResolvedSiteManifest, MobileError> {
    let signature =
        <[u8; 64]>::try_from(signature.as_slice()).map_err(|_| MobileError::InvalidInput)?;
    let root = <[u8; 32]>::try_from(followed_site_root.as_slice())
        .map_err(|_| MobileError::InvalidInput)?;
    let signed = SignedWillowEntry {
        entry_bytes,
        capability_bytes,
        signature,
        payload_bytes,
    };
    match validate_site_manifest(&signed, &root) {
        Ok(validated) => Ok(project(validated)),
        Err(error) => Ok(ResolvedSiteManifest {
            root: hex(&root),
            version: 0,
            members: Vec::new(),
            allow_transports: Vec::new(),
            require_transport: String::new(),
            moderation_path: Vec::new(),
            status: ManifestValidationStatus::ManifestInvalid,
            invalid_reason: Some(error.to_string()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Unit 4 — resolved composite-site view model (Task 7).
//
// NEW `uniffi::Enum`s (`SiteTrustTier`, `SiteDegradation`, `SiteItemTreatment`)
// and NEW `uniffi::Record`s (`ResolvedSiteItem`, `ResolvedCompositeSite`). Per
// the UniFFI gate, the native bindings + staticlib must be regenerated together
// (`scripts/conference/build-native-core.sh`) or the apps runtime-checksum-abort.
//
// These mirror the core `site::resolve` decisions so the shells RENDER them with
// no business logic: core owns trust tier, treatment, and degradation; the shell
// styles exactly what core resolved.
// ---------------------------------------------------------------------------

/// Per-item trust tier the shell styles (mirror of `site::resolve::TrustTier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SiteTrustTier {
    /// Editorial — `O:/articles`, cap-chain verified.
    Editorial,
    /// Open-wire — `W`, open publishing.
    OpenWire,
    /// Comment — `C`, open, unlinkable author.
    Comment,
}

impl From<TrustTier> for SiteTrustTier {
    fn from(tier: TrustTier) -> Self {
        match tier {
            TrustTier::Editorial => SiteTrustTier::Editorial,
            TrustTier::OpenWire => SiteTrustTier::OpenWire,
            TrustTier::Comment => SiteTrustTier::Comment,
        }
    }
}

/// The composite site's honest degradation state (mirror of
/// `site::resolve::CompositeDegradation`). A shell shows the matching copy +
/// next-step; `None` is fully resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SiteDegradation {
    None,
    MemberUnverified,
    EditorialOnly,
    ModerationLoading,
    TransportBlocked,
    ManifestRollbackAlarm,
    EquivocationAlarm,
    ManifestInvalid,
}

impl From<CompositeDegradation> for SiteDegradation {
    fn from(d: CompositeDegradation) -> Self {
        match d {
            CompositeDegradation::None => SiteDegradation::None,
            CompositeDegradation::MemberUnverified => SiteDegradation::MemberUnverified,
            CompositeDegradation::EditorialOnly => SiteDegradation::EditorialOnly,
            CompositeDegradation::ModerationLoading => SiteDegradation::ModerationLoading,
            CompositeDegradation::TransportBlocked => SiteDegradation::TransportBlocked,
            CompositeDegradation::ManifestRollbackAlarm => SiteDegradation::ManifestRollbackAlarm,
            CompositeDegradation::EquivocationAlarm => SiteDegradation::EquivocationAlarm,
            CompositeDegradation::ManifestInvalid => SiteDegradation::ManifestInvalid,
        }
    }
}

/// A moderated item's treatment (mirror of newswire `PostTreatment`, collapsed to
/// the shell-facing shape). Moderated rows stay as accountable placeholders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SiteItemTreatment {
    Ordinary,
    Hidden,
    Tombstoned,
}

impl From<PostTreatment> for SiteItemTreatment {
    fn from(t: PostTreatment) -> Self {
        match t {
            PostTreatment::Ordinary => SiteItemTreatment::Ordinary,
            PostTreatment::Hidden { .. } => SiteItemTreatment::Hidden,
            PostTreatment::Tombstoned { .. } => SiteItemTreatment::Tombstoned,
        }
    }
}

/// One resolved composite-site item, projected for the shells.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResolvedSiteItem {
    /// Entry value-identity, lowercase hex (64 chars).
    pub entry_id: String,
    /// Author subspace id, lowercase hex (64 chars).
    pub author_subspace: String,
    /// Core-resolved trust tier — the shell styles this, never infers it.
    pub trust_tier: SiteTrustTier,
    /// Core-resolved moderation treatment.
    pub treatment: SiteItemTreatment,
}

/// The resolved composite site the native shells render with no business logic.
/// A degradation is a STATE carried here, never a thrown error.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResolvedCompositeSite {
    /// Site root (owned namespace id), lowercase hex (64 chars).
    pub root: String,
    /// The single primary degradation state (most severe applicable).
    pub degradation: SiteDegradation,
    /// Fail-closed transport reason token, or `available`.
    pub transport_status: String,
    /// Resolved items across the composite (editorial + comments + wire).
    pub items: Vec<ResolvedSiteItem>,
    /// True iff the local editor's cap has expired (compose-time warning).
    pub writer_cap_expired: bool,
}

// ---------------------------------------------------------------------------
// Unit 6 — the store-wired composite-site resolver (read path).
//
// Assembles a real `ResolvedCompositeSite` from a profile's synced store: it
// validates the owner-signed manifest, loads the held `O:/mod/` records, and
// applies the core `site::resolve` decisions (freshness → degradation, per-item
// trust tier + moderation treatment). It owns NO decision logic — every verdict
// comes from `riot_core::site` — it is pure store I/O + core calls + FFI mapping.
// ---------------------------------------------------------------------------

#[uniffi::export]
impl MobileProfile {
    /// Resolve the composite site rooted at `root` from this profile's synced
    /// store, for rendering by the shells with no business logic.
    ///
    /// The manifest is passed IN as its owner-signed wire (the four fields mirror
    /// [`resolve_site_manifest`]); there is no stored "active site" — a composite
    /// site's members, moderation records, and items all live in the shared
    /// evidence store and are read by namespace here. `now_unix_seconds` is the
    /// clock the moderation freshness window is evaluated against.
    ///
    /// A validation or resolution problem is a STATE carried in `degradation`,
    /// never a thrown error: an invalid manifest returns a `ManifestInvalid` view
    /// with no items, so the shell renders the honest degraded surface.
    pub fn resolve_composite_site(
        &self,
        entry_bytes: Vec<u8>,
        capability_bytes: Vec<u8>,
        signature: Vec<u8>,
        payload_bytes: Vec<u8>,
        root: Vec<u8>,
        now_unix_seconds: u64,
    ) -> Result<ResolvedCompositeSite, MobileError> {
        let root = <[u8; 32]>::try_from(root.as_slice()).map_err(|_| MobileError::InvalidInput)?;
        let signature =
            <[u8; 64]>::try_from(signature.as_slice()).map_err(|_| MobileError::InvalidInput)?;
        let signed = SignedWillowEntry {
            entry_bytes,
            capability_bytes,
            signature,
            payload_bytes,
        };
        with_active(&self.inner, |profile| {
            resolve_composite_site_from_store(&profile.store, &signed, root, now_unix_seconds)
        })
    }
}

/// The fail-closed invalid-manifest view: no trustworthy site to resolve, so the
/// shell shows the degraded state and no items.
fn manifest_invalid_view(root: [u8; 32]) -> ResolvedCompositeSite {
    ResolvedCompositeSite {
        root: hex(&root),
        degradation: SiteDegradation::ManifestInvalid,
        transport_status: "manifest_invalid".to_string(),
        items: Vec::new(),
        writer_cap_expired: false,
    }
}

fn resolve_composite_site_from_store(
    store: &riot_core::session::EvidenceStore,
    signed: &SignedWillowEntry,
    root: [u8; 32],
    now_unix_seconds: u64,
) -> Result<ResolvedCompositeSite, MobileError> {
    // 1. Validate the owner-signed manifest. A failure is the ManifestInvalid
    //    STATE (fail-closed), never an error to the caller.
    let Ok(validated) = validate_site_manifest(signed, &root) else {
        return Ok(manifest_invalid_view(root));
    };
    let manifest = &validated.manifest;

    // 2. Load the held O:/mod/ records from the owned root namespace. The path
    //    guard (`read_moderation_record`) refuses any non-/mod/ payload; a record
    //    that will not decode, or whose payload was not retained, is dropped
    //    (fail-closed — a malformed record is simply not held, never trusted).
    let mod_prefix = Path::from_slices(&[MOD_COMPONENT]).map_err(|_| MobileError::Internal)?;
    let mod_entries = store
        .entries_with_prefix_in_namespace(&root, &mod_prefix)
        .map_err(|_| MobileError::Internal)?;
    let mut held = Vec::new();
    for (record_id, entry, payload) in &mod_entries {
        let Some(payload) = payload else { continue };
        if let Ok(record) = read_moderation_record(entry.path(), payload) {
            held.push(HeldModerationRecord {
                record,
                record_id: *record_id,
            });
        }
    }

    // 3. Protected entries = every entry in the owned root namespace (the
    //    manifest + owner records). Over-approximating the exemption is fail-safe:
    //    a moderator must never be able to tombstone an owner record or /manifest.
    let all_prefix = Path::from_slices(&[]).map_err(|_| MobileError::Internal)?;
    let protected: BTreeSet<[u8; 32]> = store
        .entries_with_prefix_in_namespace(&root, &all_prefix)
        .map_err(|_| MobileError::Internal)?
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();

    // 4. Freshness verdict — Loading holds the whole surface (never a false
    //    "current"); Current carries the exemption-filtered revoke/tombstone sets.
    let freshness = evaluate_freshness(&held, root, &protected, now_unix_seconds);

    // 5. Items: for each manifest member namespace, list its content and tag each
    //    entry with the OWNER-signed trust tier and the moderation treatment.
    //    Editorial (O) is scoped to /articles/ (excludes /manifest and /mod);
    //    the open communal members (W, C) have no reserved content path, so every
    //    entry in the member namespace is an item (honest over-approximation).
    let mut items = Vec::new();
    let mut comment_member_pending = false;
    let mut wire_member_pending = false;
    for member in &manifest.members {
        let Some(tier) = resolve_trust_tier(manifest, &member.ns) else {
            // Unknown/untrusted role — not styled here; surfaced via all_members_verified.
            continue;
        };
        let prefix = match tier {
            TrustTier::Editorial => {
                Path::from_slices(&[ARTICLES_COMPONENT]).map_err(|_| MobileError::Internal)?
            }
            TrustTier::OpenWire | TrustTier::Comment => {
                Path::from_slices(&[]).map_err(|_| MobileError::Internal)?
            }
        };
        let member_entries = store
            .entries_with_prefix_in_namespace(&member.ns, &prefix)
            .map_err(|_| MobileError::Internal)?;
        match tier {
            TrustTier::Comment => comment_member_pending = member_entries.is_empty(),
            TrustTier::OpenWire => wire_member_pending = member_entries.is_empty(),
            TrustTier::Editorial => {}
        }
        for (entry_id, entry, _) in &member_entries {
            let author = *entry.subspace_id().as_bytes();
            let treatment = item_treatment(&author, entry_id, &freshness);
            items.push(ResolvedSiteItem {
                entry_id: hex(entry_id),
                author_subspace: hex(&author),
                trust_tier: tier.into(),
                treatment: treatment.into(),
            });
        }
    }

    // 6. Fold the sub-verdicts into one primary degradation. The three inputs
    //    below are STUBBED benign for this read-path slice — each is strictly
    //    MORE severe than ModerationLoading in the resolver's precedence, so
    //    stubbing them cannot mask the moderation state:
    //    - floor = Accepted: the durable version-floor check needs the profile
    //      database (rollback/equivocation alarms) — a tracked follow-up.
    //    - transport_blocked = false: transport reachability is not yet wired.
    //    - writer_cap_expired = false: writer/compose state is the write path.
    //    `comments_and_wire_synced` is a heuristic: a member namespace that
    //    exists but holds nothing is treated as still-syncing (EditorialOnly,
    //    milder than ModerationLoading).
    let degradation = resolve_degradation(&DegradationInputs {
        manifest_valid: true,
        all_members_verified: validated.all_members_verified(),
        floor: VersionFloorOutcome::Accepted,
        moderation: &freshness,
        transport_blocked: false,
        comments_and_wire_synced: !comment_member_pending && !wire_member_pending,
    });

    Ok(ResolvedCompositeSite {
        root: hex(&root),
        degradation: degradation.into(),
        transport_status: "available".to_string(),
        items,
        writer_cap_expired: false,
    })
}

#[cfg(test)]
mod resolve_composite_tests {
    use super::*;
    use crate::mobile_api::open_local_profile;
    use riot_core::site::manifest::{
        encode_site_manifest, SiteLayout, SiteManifestV1, SiteMemberV1, TransportPolicyV1,
    };
    use riot_core::willow::{
        encode_capability, encode_entry, entry_id, Entry, SignedWillowEntry, MANIFEST_COMPONENT,
    };
    use std::sync::Arc;

    /// A fixed wall-clock inside the freshness window of the heartbeats below.
    const NOW: u64 = 2_000_000_000;

    /// Sign an owner entry at `path` and return its wire plus its willow entry-id.
    fn owner_sign(
        masthead: &OwnedMasthead,
        path: &[&[u8]],
        ts: u64,
        payload: &[u8],
    ) -> (SignedWillowEntry, [u8; 32]) {
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(Path::from_slices(path).expect("path"))
            .timestamp(ts)
            .payload(payload)
            .build();
        let authorised = masthead.authorise_owner_entry(entry).expect("authorise");
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        let entry_bytes = encode_entry(authorised.entry());
        let id = entry_id(&entry_bytes);
        (
            SignedWillowEntry {
                entry_bytes,
                capability_bytes: encode_capability(token.capability()),
                signature: signature.to_bytes(),
                payload_bytes: payload.to_vec(),
            },
            id,
        )
    }

    fn masthead_member(root: [u8; 32]) -> SiteMemberV1 {
        SiteMemberV1 {
            ns: root,
            role: SiteRole::Masthead,
            rule: SiteRule::OwnedWrite,
            display: SiteDisplay::FrontArticles,
        }
    }

    /// An owner-signed manifest at `O:/manifest` over the given members.
    fn manifest_wire(masthead: &OwnedMasthead, members: Vec<SiteMemberV1>) -> SignedWillowEntry {
        let manifest = SiteManifestV1 {
            root: *masthead.namespace_id().as_bytes(),
            members,
            moderation_path: vec![b"mod".to_vec()],
            transport_policy: TransportPolicyV1 {
                allow: vec![],
                require: RequireTransport::None,
            },
            version: 1,
            layout: SiteLayout::SiteDefault,
        };
        let payload = encode_site_manifest(&manifest).expect("encode manifest");
        owner_sign(masthead, &[MANIFEST_COMPONENT], 1_000, &payload).0
    }

    fn call_resolve(
        profile: &Arc<MobileProfile>,
        manifest: &SignedWillowEntry,
        root: [u8; 32],
    ) -> ResolvedCompositeSite {
        profile
            .resolve_composite_site(
                manifest.entry_bytes.clone(),
                manifest.capability_bytes.clone(),
                manifest.signature.to_vec(),
                manifest.payload_bytes.clone(),
                root.to_vec(),
                NOW,
            )
            .expect("resolve")
    }

    /// With no `mod_epoch` heartbeat held, freshness is a positive signal that is
    /// absent, so the whole surface is held at `ModerationLoading` — never falsely
    /// rendered current-and-unmoderated. This is the fail-closed default and works
    /// against an empty store (no `/mod/` records needed).
    #[test]
    fn no_moderation_records_holds_the_surface_at_moderation_loading() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let manifest = manifest_wire(&masthead, vec![masthead_member(root)]);
        let profile = open_local_profile().unwrap();

        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::ModerationLoading);
    }

    /// A tampered manifest signature resolves to the ManifestInvalid STATE (never
    /// a thrown error), with no items.
    #[test]
    fn a_tampered_manifest_resolves_to_the_invalid_state_not_an_error() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let mut manifest = manifest_wire(&masthead, vec![masthead_member(root)]);
        manifest.signature[0] ^= 0x01;
        let profile = open_local_profile().unwrap();

        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::ManifestInvalid);
        assert!(resolved.items.is_empty());
        assert_eq!(resolved.transport_status, "manifest_invalid");
    }

    #[test]
    fn trust_tier_maps_and_open_wire_is_never_editorial() {
        assert_eq!(
            SiteTrustTier::from(TrustTier::Editorial),
            SiteTrustTier::Editorial
        );
        let wire = SiteTrustTier::from(TrustTier::OpenWire);
        assert_eq!(wire, SiteTrustTier::OpenWire);
        assert_ne!(wire, SiteTrustTier::Editorial);
    }

    #[test]
    fn degradation_maps_every_variant() {
        for (core, ffi) in [
            (CompositeDegradation::None, SiteDegradation::None),
            (
                CompositeDegradation::MemberUnverified,
                SiteDegradation::MemberUnverified,
            ),
            (
                CompositeDegradation::EditorialOnly,
                SiteDegradation::EditorialOnly,
            ),
            (
                CompositeDegradation::ModerationLoading,
                SiteDegradation::ModerationLoading,
            ),
            (
                CompositeDegradation::TransportBlocked,
                SiteDegradation::TransportBlocked,
            ),
            (
                CompositeDegradation::ManifestRollbackAlarm,
                SiteDegradation::ManifestRollbackAlarm,
            ),
            (
                CompositeDegradation::EquivocationAlarm,
                SiteDegradation::EquivocationAlarm,
            ),
            (
                CompositeDegradation::ManifestInvalid,
                SiteDegradation::ManifestInvalid,
            ),
        ] {
            assert_eq!(SiteDegradation::from(core), ffi);
        }
    }

    #[test]
    fn treatment_maps_hidden_and_tombstoned() {
        assert_eq!(
            SiteItemTreatment::from(PostTreatment::Hidden { actions: vec![] }),
            SiteItemTreatment::Hidden
        );
        assert_eq!(
            SiteItemTreatment::from(PostTreatment::Tombstoned { actions: vec![] }),
            SiteItemTreatment::Tombstoned
        );
        assert_eq!(
            SiteItemTreatment::from(PostTreatment::Ordinary),
            SiteItemTreatment::Ordinary
        );
    }

    #[test]
    fn create_site_returns_owned_namespace_and_sealed_root() {
        let key = vec![0x22; 32];
        let created = create_owned_site(key.clone()).expect("create should succeed");
        assert_eq!(created.namespace_id.len(), 64);
        assert!(!created.sealed_root.is_empty());

        let restored = restore_owned_site(key, created.sealed_root.clone())
            .expect("restore with the same key should succeed");
        assert_eq!(restored.namespace_id, created.namespace_id);
        assert_eq!(restored.owner_subspace_id, created.owner_subspace_id);
    }

    #[test]
    fn restore_with_wrong_key_fails() {
        let created = create_owned_site(vec![0x01; 32]).expect("create should succeed");
        assert!(restore_owned_site(vec![0x02; 32], created.sealed_root).is_err());
    }
}
