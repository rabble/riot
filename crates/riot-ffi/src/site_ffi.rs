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

use riot_core::import::{decode_bundle_with_root, BundleDecodeOutcome, ItemStatus};
use riot_core::newswire::PostTreatment;
use riot_core::site::moderation::{
    compute_mod_set_digest, ModEpoch, ModerationRecord, Revoke, Tombstone,
};
use riot_core::site::{
    create_signed_moderation_record, evaluate_freshness, item_treatment, read_moderation_record,
    resolve_degradation, resolve_trust_tier, validate_site_manifest, CompositeDegradation,
    DegradationInputs, HeldModerationRecord, MemberClassification, RequireTransport,
    SignedModerationRecord, SiteDisplay, SiteRole, SiteRule, SiteTransport, TrustTier,
    ValidatedManifest, VersionFloorOutcome,
};
use riot_core::willow::site_paths::{
    is_owned_editorial_entry, is_owned_moderation_entry, ARTICLES_COMPONENT, MOD_COMPONENT,
};
use riot_core::willow::{
    decode_entry_canonic, system_snapshot, ClockSnapshot, Entry, OwnedMasthead, Path,
    SignedWillowEntry,
};
use willow25::groupings::Keylike;

use crate::community_registry::Relationship;

use crate::mobile_api::{MobileError, MobileProfile};
use crate::mobile_state::{with_active, LocalProfile};

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

// ---------------------------------------------------------------------------
// Unit 6 — the owner WRITE path: author moderation actions + publish heartbeats.
//
// Ownership IS possession of the site's sealed masthead: these methods open it
// per call (zeroing the wrapping key + key copy after), sign at O:/mod/ under the
// owner cap, and import through the same followed-root path a synced /mod/ record
// takes. Every action AUTO-PUBLISHES a fresh mod-epoch committing to the owner's
// full held revoke/tombstone set, so a follower's freshness can reach Current —
// a forgotten heartbeat leaves followers at ModerationLoading forever.
//
// SCOPE: owner-only authoring. Minting /mod/-scoped DELEGATED moderator caps
// (delegate_section) is deferred — the read path already accepts a
// moderator-cap-signed /mod/ record if one existed; this path just does not let
// the owner hand out moderator caps yet.
// ---------------------------------------------------------------------------

/// A moderation action a site owner authors at O:/mod/. The overlay applies to
/// COMMUNAL member content only (owner editorial is protected read-side).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SiteModerationAction {
    /// Ban an author-key: every item that author signed is Hidden (read-side).
    Revoke {
        /// The author subspace id to revoke, lowercase hex (64 chars).
        author_key: String,
    },
    /// Hide one specific entry by its `(namespace, entry-id)` identity.
    Tombstone {
        /// The namespace the tombstoned entry lives in, hex (64 chars).
        target_namespace: String,
        /// The entry id to hide, hex (64 chars).
        target_entry: String,
    },
}

/// A signed `/mod/` record: its entry id (hex) and the bundle bytes. The bytes are
/// what the app propagates to followers — a composite site's owned-namespace
/// records do NOT ride the per-community sync inventory (that is namespace-scoped
/// to the active community), so, exactly like a newswire share, the owner hands the
/// signed bytes onward for sync.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SiteModerationSignedRecord {
    pub entry_id: String,
    pub signed_bytes: Vec<u8>,
}

/// The result of a moderation write: the signed action AND the fresh heartbeat it
/// auto-published. The heartbeat is coupled to every action (a forgotten one
/// strands followers at ModerationLoading).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SiteModerationOutcome {
    pub action: SiteModerationSignedRecord,
    pub epoch: SiteModerationSignedRecord,
}

#[uniffi::export]
impl MobileProfile {
    /// Author a moderation action as the site owner, then AUTO-PUBLISH a fresh
    /// mod-epoch heartbeat committing to the owner's full held revoke/tombstone
    /// set. Requires the site's sealed masthead + wrapping key (a wrong key, or a
    /// caller who does not hold the masthead, cannot author). Both the wrapping
    /// key and the opened key copy are zeroed after the call.
    pub fn create_site_moderation_action(
        &self,
        sealed_root: Vec<u8>,
        mut wrapping_key: Vec<u8>,
        action: SiteModerationAction,
    ) -> Result<SiteModerationOutcome, MobileError> {
        let result = self.author_moderation(&sealed_root, &wrapping_key, action);
        wrapping_key.iter_mut().for_each(|b| *b = 0);
        result
    }

    /// Re-publish a mod-epoch heartbeat without a new action — to refresh
    /// freshness past the window, or after a sync delivered records the previous
    /// heartbeat did not commit. Same digest-over-all-held-ids discipline.
    pub fn republish_mod_epoch(
        &self,
        sealed_root: Vec<u8>,
        mut wrapping_key: Vec<u8>,
    ) -> Result<SiteModerationSignedRecord, MobileError> {
        let result = self.republish_heartbeat(&sealed_root, &wrapping_key);
        wrapping_key.iter_mut().for_each(|b| *b = 0);
        result
    }
}

impl MobileProfile {
    fn author_moderation(
        &self,
        sealed_root: &[u8],
        wrapping_key: &[u8],
        action: SiteModerationAction,
    ) -> Result<SiteModerationOutcome, MobileError> {
        let mut key = exact_key(wrapping_key)?;
        let masthead = OwnedMasthead::open_sealed(&key, sealed_root);
        key.iter_mut().for_each(|b| *b = 0);
        let masthead = masthead.map_err(|_| MobileError::InvalidInput)?;
        let root = *masthead.namespace_id().as_bytes();
        let snapshot = system_snapshot().map_err(|_| MobileError::ClockUnavailable)?;
        let record = moderation_record_for(&action, snapshot.unix_seconds)?;
        let signed_action = create_signed_moderation_record(&masthead, &record, snapshot)
            .map_err(|_| MobileError::InvalidInput)?;
        with_active(&self.inner, |profile| {
            let action = import_owned_mod(profile, root, &signed_action)?;
            let epoch = sign_and_import_epoch(profile, &masthead, root, snapshot)?;
            Ok(SiteModerationOutcome { action, epoch })
        })
    }

    fn republish_heartbeat(
        &self,
        sealed_root: &[u8],
        wrapping_key: &[u8],
    ) -> Result<SiteModerationSignedRecord, MobileError> {
        let mut key = exact_key(wrapping_key)?;
        let masthead = OwnedMasthead::open_sealed(&key, sealed_root);
        key.iter_mut().for_each(|b| *b = 0);
        let masthead = masthead.map_err(|_| MobileError::InvalidInput)?;
        let root = *masthead.namespace_id().as_bytes();
        let snapshot = system_snapshot().map_err(|_| MobileError::ClockUnavailable)?;
        with_active(&self.inner, |profile| {
            sign_and_import_epoch(profile, &masthead, root, snapshot)
        })
    }
}

fn moderation_record_for(
    action: &SiteModerationAction,
    effective_ts: u64,
) -> Result<ModerationRecord, MobileError> {
    match action {
        SiteModerationAction::Revoke { author_key } => Ok(ModerationRecord::Revoke(Revoke {
            author_key: parse_id(author_key)?,
            effective_ts,
        })),
        SiteModerationAction::Tombstone {
            target_namespace,
            target_entry,
        } => Ok(ModerationRecord::Tombstone(Tombstone {
            target_ns: parse_id(target_namespace)?,
            target_entry: parse_id(target_entry)?,
        })),
    }
}

/// Sign a fresh heartbeat committing to ALL held revoke/tombstone ids in the
/// owner's store, and import it. This is what lets a follower's `evaluate_freshness`
/// reach `Current` (their recomputed digest over the same set matches). Returns the
/// heartbeat entry id.
fn sign_and_import_epoch(
    profile: &mut LocalProfile,
    masthead: &OwnedMasthead,
    root: [u8; 32],
    snapshot: ClockSnapshot,
) -> Result<SiteModerationSignedRecord, MobileError> {
    let (mod_ids, next_seq) = scan_held_mods(profile, root)?;
    let epoch = ModerationRecord::ModEpoch(ModEpoch {
        seq: next_seq,
        ts: snapshot.unix_seconds,
        mod_set_digest: compute_mod_set_digest(&mod_ids),
    });
    let signed: SignedModerationRecord =
        create_signed_moderation_record(masthead, &epoch, snapshot)
            .map_err(|_| MobileError::InvalidInput)?;
    import_owned_mod(profile, root, &signed)
}

/// All held Revoke/Tombstone entry ids in the owned root ns + the next mod-epoch
/// seq (latest held + 1, or 1). Mirrors what `evaluate_freshness` recomputes on
/// the read side, so the owner's heartbeat and a follower's recompute agree.
fn scan_held_mods(
    profile: &LocalProfile,
    root: [u8; 32],
) -> Result<(BTreeSet<[u8; 32]>, u64), MobileError> {
    let prefix = Path::from_slices(&[MOD_COMPONENT]).map_err(|_| MobileError::Internal)?;
    let entries = profile
        .store
        .entries_with_prefix_in_namespace(&root, &prefix)
        .map_err(|_| MobileError::Internal)?;
    let mut mod_ids = BTreeSet::new();
    let mut max_seq = 0u64;
    for (id, entry, payload) in &entries {
        let Some(payload) = payload else { continue };
        let Ok(record) = read_moderation_record(entry.path(), payload) else {
            continue;
        };
        match record {
            ModerationRecord::Revoke(_) | ModerationRecord::Tombstone(_) => {
                mod_ids.insert(*id);
            }
            ModerationRecord::ModEpoch(epoch) => max_seq = max_seq.max(epoch.seq),
            ModerationRecord::Endorse(_) => {}
        }
    }
    Ok((mod_ids, max_seq + 1))
}

/// Import an owner-signed /mod/ record through the followed-root admission path
/// (the owner follows their own site) and return its entry id + bundle bytes.
/// Owned /mod/ fails closed under a rootless import, so the root is required.
///
/// It is NOT added to the per-community sync inventory: that inventory is
/// namespace-scoped to the ACTIVE community and would prune out an owned-namespace
/// /mod/ record. Composite-site /mod/ propagation instead rides the returned bytes
/// (the same way a newswire share hands signed bytes onward), which is what a
/// follower imports to receive the moderation set.
fn import_owned_mod(
    profile: &mut LocalProfile,
    root: [u8; 32],
    record: &SignedModerationRecord,
) -> Result<SiteModerationSignedRecord, MobileError> {
    let bundle = riot_core::import::encode_bundle(std::slice::from_ref(&record.signed))
        .map_err(|_| MobileError::Internal)?;
    profile.preview = None;
    profile.plan = None;
    let preview = crate::mobile_state::inspect_core_with_root(
        &profile.store,
        &bundle,
        "local-mod-sign",
        Some(root),
    )?;
    let plan = preview.plan_all().map_err(|_| MobileError::Internal)?;
    use riot_core::session::CommitOutcome;
    match plan.commit().map_err(|_| MobileError::Internal)? {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
    Ok(SiteModerationSignedRecord {
        entry_id: hex(&record.entry_id),
        signed_bytes: bundle,
    })
}

fn parse_id(hex_str: &str) -> Result<[u8; 32], MobileError> {
    let bytes = hex_decode(hex_str).ok_or(MobileError::InvalidInput)?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| MobileError::InvalidInput)
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Delivery: import a shared followed-site bundle (the propagation channel).
//
// Owned-namespace content (/mod/, /articles/, /manifest) has no automatic
// follower sync — the community sync inventory is namespace-scoped to the ACTIVE
// community, so it never carries an owned site's records (see import_owned_mod).
// The owner therefore hands the signed bytes onward out-of-band; THIS is the
// follower's importer for them. It is a NEW admission entry point (external bytes
// → store), gated by TWO checks before the proven inspect_core_with_root(Some(root))
// admission runs:
//   1. FOLLOWING gate: the caller must already hold a `Following` registry record
//      for `root` — a bundle can never smuggle an UNFOLLOWED owned namespace in.
//   2. FAMILY gate: every entry must be owned /mod, /articles, or /manifest;
//      anything else (a communal alert, a newswire post) rejects the whole bundle.
// A non-admissible item (e.g. an owned entry NOT rooted at `root`) is Invalid at
// decode and rejects the whole bundle (fail-closed, all-or-nothing). The imported
// records go to the store only — sync_inventory is never touched (owned ns must
// not leak into the active community's peer offer set).
// ---------------------------------------------------------------------------

/// The result of importing a followed-site bundle: how many records committed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportSummary {
    /// Number of owned-site records (mod / articles / manifest) admitted + committed.
    pub imported: u32,
}

#[uniffi::export]
impl MobileProfile {
    /// Import a shared owner-signed bundle for a followed composite site. The
    /// caller must already FOLLOW `followed_site_root` (a Following registry
    /// record); the bundle may carry ONLY that site's owned /mod, /articles, and
    /// /manifest records — anything else, or any entry not rooted at the followed
    /// site, rejects the whole bundle. Admitted records land in the store (read by
    /// resolve_composite_site); nothing enters the community sync inventory.
    pub fn import_followed_site_bundle(
        &self,
        bytes: Vec<u8>,
        followed_site_root: Vec<u8>,
    ) -> Result<ImportSummary, MobileError> {
        let root = <[u8; 32]>::try_from(followed_site_root.as_slice())
            .map_err(|_| MobileError::InvalidInput)?;
        with_active(&self.inner, |profile| {
            // 1. FOLLOWING gate — the key security check. A bundle for an
            //    unfollowed owned namespace is refused before any admission runs.
            let following = profile
                .registry
                .find(&root)
                .is_some_and(|record| record.relationship == Relationship::Following);
            if !following {
                return Err(MobileError::ImportRejected);
            }

            // 2. Decode under the followed root + FAMILY gate (all-or-nothing).
            let decoded = match decode_bundle_with_root(&bytes, Some(root)) {
                BundleDecodeOutcome::Decoded(decoded) => decoded,
                BundleDecodeOutcome::Rejected(_) => return Err(MobileError::ImportRejected),
            };
            let mut count = 0u32;
            for item in &decoded.items {
                // A non-admissible item (owned entry not rooted at `root`, forged
                // cap, etc.) makes the whole followed-site bundle untrustworthy.
                let ItemStatus::Valid(_) = &item.status else {
                    return Err(MobileError::ImportRejected);
                };
                let entry = decode_entry_canonic(item.frame.entry_bytes())
                    .map_err(|_| MobileError::ImportRejected)?;
                if !is_followed_site_family(&entry) {
                    return Err(MobileError::ImportRejected);
                }
                count += 1;
            }
            if count == 0 {
                return Err(MobileError::ImportRejected);
            }

            // 3. Admit + commit through the proven followed-root path. Owned /mod,
            //    /articles, /manifest are all admitted under a cap rooted at `root`.
            profile.preview = None;
            profile.plan = None;
            let preview = crate::mobile_state::inspect_core_with_root(
                &profile.store,
                &bytes,
                "site-follow-import",
                Some(root),
            )?;
            let plan = preview.plan_all().map_err(|_| MobileError::Internal)?;
            use riot_core::session::CommitOutcome;
            match plan.commit().map_err(|_| MobileError::Internal)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }

            // 4. sync_inventory is intentionally NOT touched: owned-namespace records
            //    must stay out of the active community's peer offer set (the
            //    install_sync_inventory isolation invariant).
            // 5. Stamp the follow's last-sync time (best-effort).
            if let Ok(snapshot) = system_snapshot() {
                if let Some(record) = profile.registry.find_mut(&root) {
                    record.last_sync_unix_seconds = Some(snapshot.unix_seconds);
                    let _ = crate::mobile_state::persist_registry(profile);
                }
            }
            Ok(ImportSummary { imported: count })
        })
    }
}

/// Whether `entry` is an owned composite-site record a followed-site bundle may
/// carry: `/mod` (moderation) or `/articles` (editorial) only. Least-privilege —
/// these are exactly the families a STORE READER consumes (resolve_composite_site
/// reads /mod freshness + /articles items). The manifest is validated from a
/// caller argument, never read from the store, so admitting owned /manifest here
/// would be dead admission that only widens the surface — excluded. Anything else
/// (a communal alert/newswire entry) is never admissible here.
fn is_followed_site_family(entry: &Entry) -> bool {
    is_owned_moderation_entry(entry) || is_owned_editorial_entry(entry)
}

// These tests live IN-CRATE (not crates/riot-ffi/tests/) on purpose: owner /mod/
// and owner /manifest entries can only enter a profile store via the crate-internal
// followed-root admission path (`inspect_core_with_root`) — there is no public
// single-shot site import, only the full sync session. So an integration test in
// tests/ could not populate the store the way newswire_contract.rs does. These
// exercise the real store, the real admission path, and real core — same fidelity,
// just reachable only from inside the crate. Do NOT "fix" this by moving to tests/.
#[cfg(test)]
mod resolve_composite_tests {
    use super::*;
    use crate::mobile_api::open_local_profile;
    use crate::newswire_ffi::{NewswirePostInput, NewswireSpaceInput};
    use riot_core::import::encode_bundle;
    use riot_core::session::CommitOutcome;
    use riot_core::site::manifest::{
        encode_site_manifest, SiteLayout, SiteManifestV1, SiteMemberV1, TransportPolicyV1,
    };
    use riot_core::site::moderation::{
        compute_mod_set_digest, encode_moderation_record, ModEpoch, ModerationRecord, Revoke,
        Tombstone,
    };
    use riot_core::willow::{
        encode_capability, encode_entry, entry_id, Entry, SignedWillowEntry, MANIFEST_COMPONENT,
    };
    use std::collections::BTreeSet as StdBTreeSet;
    use std::sync::Arc;

    /// A fixed wall-clock inside the freshness window of a fresh heartbeat.
    const NOW: u64 = 2_000_000_000;

    fn unhex(s: &str) -> [u8; 32] {
        let bytes: Vec<u8> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect();
        <[u8; 32]>::try_from(bytes.as_slice()).unwrap()
    }

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

    fn wire_member(ns: [u8; 32]) -> SiteMemberV1 {
        SiteMemberV1 {
            ns,
            role: SiteRole::OpenWire,
            rule: SiteRule::CommunalOpen,
            display: SiteDisplay::WireColumn,
        }
    }

    /// Import an owner-signed entry into the profile store via the same
    /// followed-root admission path the sync commit uses.
    fn import_owned(profile: &Arc<MobileProfile>, root: [u8; 32], signed: &SignedWillowEntry) {
        with_active(&profile.inner, |p| {
            let bundle =
                encode_bundle(std::slice::from_ref(signed)).map_err(|_| MobileError::Internal)?;
            let preview = crate::mobile_state::inspect_core_with_root(
                &p.store,
                &bundle,
                "test-composite",
                Some(root),
            )?;
            let plan = preview.plan_all().map_err(|_| MobileError::Internal)?;
            match plan.commit().map_err(|_| MobileError::Internal)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }
            Ok(())
        })
        .expect("import owned entry");
    }

    /// Import already-framed owner-signed bundle bytes (e.g. the bytes the write
    /// path returns) through the followed-root path — a follower receiving a shared
    /// /mod/ record.
    fn import_bundle(profile: &Arc<MobileProfile>, root: [u8; 32], bytes: &[u8]) {
        with_active(&profile.inner, |p| {
            let preview = crate::mobile_state::inspect_core_with_root(
                &p.store,
                bytes,
                "test-follower",
                Some(root),
            )?;
            let plan = preview.plan_all().map_err(|_| MobileError::Internal)?;
            match plan.commit().map_err(|_| MobileError::Internal)? {
                CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
            }
            Ok(())
        })
        .expect("import bundle");
    }

    /// A signed owner `mod_epoch` heartbeat committing to `mod_ids`.
    fn heartbeat(
        masthead: &OwnedMasthead,
        seq: u64,
        ts: u64,
        mod_ids: &[[u8; 32]],
    ) -> SignedWillowEntry {
        let set: StdBTreeSet<[u8; 32]> = mod_ids.iter().copied().collect();
        let payload = encode_moderation_record(&ModerationRecord::ModEpoch(ModEpoch {
            seq,
            ts,
            mod_set_digest: compute_mod_set_digest(&set),
        }))
        .expect("encode epoch");
        owner_sign(masthead, &[b"mod", b"epoch"], ts, &payload).0
    }

    /// A signed owner `revoke{author_key}` moderation record.
    fn revoke(
        masthead: &OwnedMasthead,
        slug: &[u8],
        author_key: [u8; 32],
    ) -> (SignedWillowEntry, [u8; 32]) {
        let payload = encode_moderation_record(&ModerationRecord::Revoke(Revoke {
            author_key,
            effective_ts: NOW,
        }))
        .expect("encode revoke");
        owner_sign(masthead, &[b"mod", slug], NOW, &payload)
    }

    /// A signed owner `tombstone{target_ns, target_entry}` moderation record.
    fn tombstone(
        masthead: &OwnedMasthead,
        slug: &[u8],
        target_ns: [u8; 32],
        target_entry: [u8; 32],
    ) -> (SignedWillowEntry, [u8; 32]) {
        let payload = encode_moderation_record(&ModerationRecord::Tombstone(Tombstone {
            target_ns,
            target_entry,
        }))
        .expect("encode tombstone");
        owner_sign(masthead, &[b"mod", slug], NOW, &payload)
    }

    /// A held communal open-wire item: a real newswire post in its own communal
    /// namespace, which the manifest binds as the `W` member. Returns the profile,
    /// the wire namespace, the post entry id, and the post author subspace.
    fn wire_post(profile: &Arc<MobileProfile>) -> ([u8; 32], [u8; 32], [u8; 32]) {
        let space = profile
            .create_newswire_space(NewswireSpaceInput {
                name: "Wire".into(),
                summary: "Open wire.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
            })
            .expect("create wire space");
        let post = profile
            .create_newswire_post(NewswirePostInput {
                space_descriptor_entry_id: space.entry_id.clone(),
                headline: "On the wire".into(),
                body: "A community report.".into(),
                language: "en".into(),
                event_time_unix_seconds: None,
                expires_at_unix_seconds: None,
                coarse_location: None,
                source_claims: vec![],
                operational_profile: None,
                ai_assisted: false,
            })
            .expect("create wire post");
        let post_id = unhex(&post.entry_id);
        // The wire namespace and the post's author come straight from the store,
        // so the manifest binds the real namespace and moderation targets the real
        // author/entry.
        with_active(&profile.inner, |p| {
            let ns_hex = p.space.as_ref().unwrap().namespace_id.clone();
            let ns = unhex(&ns_hex);
            let prefix = Path::from_slices(&[b"newswire", b"v1"]).unwrap();
            let entries = p
                .store
                .entries_with_prefix_in_namespace(&ns, &prefix)
                .map_err(|_| MobileError::Internal)?;
            let author = entries
                .iter()
                .find(|(id, _, _)| *id == post_id)
                .map(|(_, e, _)| *e.subspace_id().as_bytes())
                .expect("post held in wire namespace");
            Ok((ns, post_id, author))
        })
        .expect("read wire namespace")
    }

    /// The count of genuinely-held (decodable, payload-retained) `/mod/` records in
    /// the owned root namespace — the resolver's loaded moderation set.
    fn held_mod_count(profile: &Arc<MobileProfile>, root: [u8; 32]) -> usize {
        with_active(&profile.inner, |p| {
            let prefix = Path::from_slices(&[MOD_COMPONENT]).unwrap();
            let entries = p
                .store
                .entries_with_prefix_in_namespace(&root, &prefix)
                .map_err(|_| MobileError::Internal)?;
            Ok(entries
                .iter()
                .filter(|(_, e, payload)| {
                    payload
                        .as_ref()
                        .is_some_and(|pl| read_moderation_record(e.path(), pl).is_ok())
                })
                .count())
        })
        .expect("count held mod records")
    }

    /// Find the resolved item for `entry_id` (hex), if present.
    fn item_for(resolved: &ResolvedCompositeSite, entry_id: [u8; 32]) -> Option<&ResolvedSiteItem> {
        let hexed = hex(&entry_id);
        resolved.items.iter().find(|i| i.entry_id == hexed)
    }

    fn call_resolve(
        profile: &Arc<MobileProfile>,
        manifest: &SignedWillowEntry,
        root: [u8; 32],
    ) -> ResolvedCompositeSite {
        call_resolve_at(profile, manifest, root, NOW)
    }

    /// Resolve at an explicit `now`. The write path stamps a heartbeat with the
    /// REAL system clock, so a round-trip must resolve near that clock (else the
    /// heartbeat reads Stale rather than Current) — write tests pass the real now.
    fn call_resolve_at(
        profile: &Arc<MobileProfile>,
        manifest: &SignedWillowEntry,
        root: [u8; 32],
        now: u64,
    ) -> ResolvedCompositeSite {
        profile
            .resolve_composite_site(
                manifest.entry_bytes.clone(),
                manifest.capability_bytes.clone(),
                manifest.signature.to_vec(),
                manifest.payload_bytes.clone(),
                root.to_vec(),
                now,
            )
            .expect("resolve")
    }

    /// The real system clock in unix seconds — the same clock the write path
    /// stamps heartbeats with, so a resolve at this `now` sees a fresh heartbeat.
    fn now_unix() -> u64 {
        riot_core::willow::system_snapshot().unwrap().unix_seconds
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

    /// A fresh heartbeat committing to an empty mod set is a Current verdict with
    /// nothing moderated: no degradation. (Also proves owner /mod/ records now
    /// reach the store — the session-import fix.)
    #[test]
    fn a_fresh_empty_heartbeat_is_current_with_no_degradation() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let manifest = manifest_wire(&masthead, vec![masthead_member(root)]);
        let profile = open_local_profile().unwrap();
        import_owned(&profile, root, &heartbeat(&masthead, 1, NOW, &[]));

        assert_eq!(
            held_mod_count(&profile, root),
            1,
            "the heartbeat must be held"
        );
        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::None);
    }

    /// A held tombstone of a communal wire item, attested by a fresh heartbeat,
    /// renders that item Tombstoned — the moderation overlay reaches communal
    /// content.
    #[test]
    fn a_tombstoned_wire_item_is_tombstoned_when_current() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let profile = open_local_profile().unwrap();
        let (ns_w, post_id, _author) = wire_post(&profile);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        let (tomb, tomb_id) = tombstone(&masthead, b"t1", ns_w, post_id);
        import_owned(&profile, root, &tomb);
        import_owned(&profile, root, &heartbeat(&masthead, 1, NOW, &[tomb_id]));

        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::None);
        let item = item_for(&resolved, post_id).expect("wire post is an item");
        assert_eq!(item.treatment, SiteItemTreatment::Tombstoned);
        assert_eq!(item.trust_tier, SiteTrustTier::OpenWire);
    }

    /// A held revoke of a communal author, attested by a fresh heartbeat, hides
    /// that author's wire item.
    #[test]
    fn a_revoked_authors_wire_item_is_hidden_when_current() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let profile = open_local_profile().unwrap();
        let (ns_w, post_id, author) = wire_post(&profile);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        let (rev, rev_id) = revoke(&masthead, b"r1", author);
        import_owned(&profile, root, &rev);
        import_owned(&profile, root, &heartbeat(&masthead, 1, NOW, &[rev_id]));

        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::None);
        let item = item_for(&resolved, post_id).expect("wire post is an item");
        assert_eq!(item.treatment, SiteItemTreatment::Hidden);
    }

    /// KEYSTONE: under a Loading verdict the whole surface is HELD — even a
    /// present, would-be-revoked author's item stays Ordinary (rendering it Hidden
    /// would leak a partial verdict), and the degradation is ModerationLoading. The
    /// hold happens BECAUSE the mod is present-but-unattested (a stale heartbeat),
    /// not because it is absent — asserted by held_mod_count.
    #[test]
    fn loading_holds_the_surface_even_with_a_present_but_unattested_revoke() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let profile = open_local_profile().unwrap();
        let (ns_w, post_id, author) = wire_post(&profile);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        let (rev, rev_id) = revoke(&masthead, b"r1", author);
        import_owned(&profile, root, &rev);
        // A STALE heartbeat (far outside the freshness window) that DOES commit the
        // revoke: the mod is present and attested, but freshness is Loading, so the
        // surface holds rather than applying the revoke.
        let stale_ts = NOW - 10 * riot_core::site::MODERATION_FRESHNESS_WINDOW_SECS;
        import_owned(
            &profile,
            root,
            &heartbeat(&masthead, 1, stale_ts, &[rev_id]),
        );

        // The revoke IS held — the hold below is not because moderation is missing.
        assert_eq!(
            held_mod_count(&profile, root),
            2,
            "both the revoke and the (stale) heartbeat are held"
        );
        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::ModerationLoading);
        let item = item_for(&resolved, post_id).expect("wire post is an item");
        assert_eq!(
            item.treatment,
            SiteItemTreatment::Ordinary,
            "under Loading the item is held Ordinary, never Hidden — no partial verdict leak"
        );
    }

    /// A held revoke that the latest heartbeat does NOT commit to (digest mismatch)
    /// is tail-suppression: the client is missing records the owner attested, so
    /// the surface holds at ModerationLoading.
    #[test]
    fn a_revoke_absent_from_the_heartbeat_digest_holds_at_moderation_loading() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let profile = open_local_profile().unwrap();
        let (ns_w, _post_id, author) = wire_post(&profile);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        let (rev, _rev_id) = revoke(&masthead, b"r1", author);
        import_owned(&profile, root, &rev);
        // Fresh heartbeat, but its digest commits to the EMPTY set — it does not
        // attest the held revoke.
        import_owned(&profile, root, &heartbeat(&masthead, 1, NOW, &[]));

        assert_eq!(held_mod_count(&profile, root), 2);
        let resolved = call_resolve(&profile, &manifest, root);
        assert_eq!(resolved.degradation, SiteDegradation::ModerationLoading);
    }

    /// D3 GUARD (keep permanently): the moderation overlay applies to communal
    /// content only — an owner O:/articles item is PROTECTED, so a tombstone
    /// targeting it (attested by a fresh heartbeat) has no effect and the item
    /// stays Ordinary. A delegated moderator must never tombstone the owner's
    /// editorial.
    #[test]
    fn an_owner_article_is_protected_from_tombstone() {
        let masthead = OwnedMasthead::generate().unwrap();
        let root = *masthead.namespace_id().as_bytes();
        let manifest = manifest_wire(&masthead, vec![masthead_member(root)]);
        let profile = open_local_profile().unwrap();

        // An owner-signed article at O:/articles/, held in the root namespace.
        let (article, article_id) = owner_sign(
            &masthead,
            &[ARTICLES_COMPONENT, b"news", b"a1"],
            NOW,
            b"editorial body",
        );
        import_owned(&profile, root, &article);
        let (tomb, tomb_id) = tombstone(&masthead, b"t1", root, article_id);
        import_owned(&profile, root, &tomb);
        import_owned(&profile, root, &heartbeat(&masthead, 1, NOW, &[tomb_id]));

        let resolved = call_resolve(&profile, &manifest, root);
        let item = item_for(&resolved, article_id).expect("owner article is an item");
        assert_eq!(item.trust_tier, SiteTrustTier::Editorial);
        assert_eq!(
            item.treatment,
            SiteItemTreatment::Ordinary,
            "an owner article must be protected from a moderator tombstone (D3)"
        );
    }

    // ---- Write path: owner authors moderation + auto-published heartbeat ----

    const WRAP_KEY: [u8; 32] = [0x5a; 32];

    /// A fresh masthead sealed under WRAP_KEY. Returns (masthead for signing the
    /// manifest, sealed blob for the write FFI, root ns).
    fn sealed_masthead() -> (OwnedMasthead, Vec<u8>, [u8; 32]) {
        let masthead = OwnedMasthead::generate().unwrap();
        let sealed = masthead.seal(&WRAP_KEY).unwrap();
        let root = *masthead.namespace_id().as_bytes();
        (masthead, sealed, root)
    }

    /// (b) A wrong wrapping key cannot open the masthead — ownership IS possession
    /// of the masthead secret, so a non-owner (wrong key, or no sealed blob) is
    /// refused before anything is signed.
    #[test]
    fn a_wrong_wrapping_key_cannot_author_moderation() {
        let (_masthead, sealed, _root) = sealed_masthead();
        let profile = open_local_profile().unwrap();
        let result = profile.create_site_moderation_action(
            sealed,
            vec![0x11u8; 32],
            SiteModerationAction::Revoke {
                author_key: hex(&[0x42; 32]),
            },
        );
        assert!(result.is_err(), "a wrong wrapping key must be refused");
    }

    /// (c) ROUND-TRIP: the owner tombstones a wire item; the auto-published
    /// heartbeat makes freshness Current, and the read-side resolve renders that
    /// item Tombstoned. Write path → read path Current, end to end.
    #[test]
    fn owner_tombstone_round_trips_to_a_tombstoned_item() {
        let (masthead, sealed, root) = sealed_masthead();
        let profile = open_local_profile().unwrap();
        let (ns_w, post_id, _author) = wire_post(&profile);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        let outcome = profile
            .create_site_moderation_action(
                sealed,
                WRAP_KEY.to_vec(),
                SiteModerationAction::Tombstone {
                    target_namespace: hex(&ns_w),
                    target_entry: hex(&post_id),
                },
            )
            .unwrap();
        assert!(!outcome.action.entry_id.is_empty());
        assert!(!outcome.action.signed_bytes.is_empty());
        assert!(!outcome.epoch.entry_id.is_empty());
        assert!(!outcome.epoch.signed_bytes.is_empty());

        let resolved = call_resolve_at(&profile, &manifest, root, now_unix());
        assert_eq!(
            resolved.degradation,
            SiteDegradation::None,
            "the auto-published heartbeat commits to the owner's held set → Current"
        );
        let item = item_for(&resolved, post_id).expect("wire post is an item");
        assert_eq!(item.treatment, SiteItemTreatment::Tombstoned);
    }

    /// (c) ROUND-TRIP: the owner revokes a wire author; the item is Hidden under a
    /// Current verdict.
    #[test]
    fn owner_revoke_round_trips_to_a_hidden_item() {
        let (masthead, sealed, root) = sealed_masthead();
        let profile = open_local_profile().unwrap();
        let (ns_w, post_id, author) = wire_post(&profile);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        profile
            .create_site_moderation_action(
                sealed,
                WRAP_KEY.to_vec(),
                SiteModerationAction::Revoke {
                    author_key: hex(&author),
                },
            )
            .unwrap();

        let resolved = call_resolve_at(&profile, &manifest, root, now_unix());
        assert_eq!(resolved.degradation, SiteDegradation::None);
        let item = item_for(&resolved, post_id).expect("wire post is an item");
        assert_eq!(item.treatment, SiteItemTreatment::Hidden);
    }

    /// (d) FAIL-CLOSED against a REAL write-path epoch: a follower that received the
    /// auto-published heartbeat but is MISSING the tombstone it commits to must NOT
    /// reach a false Current — the recomputed digest mismatches, so the surface
    /// holds at ModerationLoading. Proves the write path produces epochs the read
    /// path correctly rejects when a follower's sync is incomplete (tail-suppression
    /// resistance against genuine epochs, not hand-built ones).
    #[test]
    fn a_follower_missing_the_committed_action_holds_at_moderation_loading() {
        let (masthead, sealed, root) = sealed_masthead();
        let owner = open_local_profile().unwrap();
        let (ns_w, post_id, _author) = wire_post(&owner);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root), wire_member(ns_w)]);

        let outcome = owner
            .create_site_moderation_action(
                sealed,
                WRAP_KEY.to_vec(),
                SiteModerationAction::Tombstone {
                    target_namespace: hex(&ns_w),
                    target_entry: hex(&post_id),
                },
            )
            .unwrap();

        // A fresh FOLLOWER receives ONLY the heartbeat's bytes (the returned bundle
        // the app would propagate), not the tombstone action it commits to.
        let follower = open_local_profile().unwrap();
        import_bundle(&follower, root, &outcome.epoch.signed_bytes);

        // Resolve at the REAL clock so the ONLY reason for a hold is the digest
        // mismatch (the missing tombstone) — not a stale heartbeat.
        let resolved = call_resolve_at(&follower, &manifest, root, now_unix());
        assert_eq!(
            resolved.degradation,
            SiteDegradation::ModerationLoading,
            "a heartbeat committing to an unheld record must hold the surface, never false-Current"
        );
    }

    /// republish_mod_epoch publishes a heartbeat on demand (empty set here).
    #[test]
    fn republish_mod_epoch_publishes_a_heartbeat() {
        let (_masthead, sealed, _root) = sealed_masthead();
        let profile = open_local_profile().unwrap();
        let epoch = profile
            .republish_mod_epoch(sealed, WRAP_KEY.to_vec())
            .unwrap();
        assert!(!epoch.entry_id.is_empty());
        assert!(!epoch.signed_bytes.is_empty());
    }

    // ---- Delivery: import_followed_site_bundle (the propagation channel) ----

    /// A shared /mod bundle (tombstone + an attesting fresh heartbeat) as owner
    /// bytes a follower would receive out of band.
    fn shared_mod_bundle(masthead: &OwnedMasthead) -> Vec<u8> {
        let (tomb, tomb_id) = tombstone(masthead, b"t1", [0x77; 32], [0x88; 32]);
        let epoch = heartbeat(masthead, 1, now_unix(), &[tomb_id]);
        encode_bundle(&[tomb, epoch]).expect("bundle")
    }

    /// (1) ROUND-TRIP DELIVERY: a follower holding a Following record imports the
    /// owner's /mod bundle and advances OFF ModerationLoading — the delivery the
    /// merged read/write paths were missing.
    #[test]
    fn a_following_follower_imports_a_mod_bundle_and_advances_off_loading() {
        let (masthead, _sealed, root) = sealed_masthead();
        let bundle = shared_mod_bundle(&masthead);
        let manifest = manifest_wire(&masthead, vec![masthead_member(root)]);
        let follower = open_local_profile().unwrap();
        follower.follow_site_for_test(root.to_vec()).unwrap();

        // Before delivery: no /mod records → held at ModerationLoading.
        assert_eq!(
            call_resolve_at(&follower, &manifest, root, now_unix()).degradation,
            SiteDegradation::ModerationLoading
        );

        let summary = follower
            .import_followed_site_bundle(bundle, root.to_vec())
            .unwrap();
        assert_eq!(summary.imported, 2);

        // After delivery: freshness reaches Current → off ModerationLoading.
        assert_ne!(
            call_resolve_at(&follower, &manifest, root, now_unix()).degradation,
            SiteDegradation::ModerationLoading,
            "delivering the owner's /mod bundle must advance the follower off ModerationLoading"
        );
    }

    /// (2) FOLLOWING GATE: a bundle for a root the caller does NOT follow is
    /// refused — a bundle can never smuggle an unfollowed owned namespace in.
    #[test]
    fn a_bundle_for_an_unfollowed_root_is_rejected() {
        let (masthead, _sealed, root) = sealed_masthead();
        let bundle = shared_mod_bundle(&masthead);
        let follower = open_local_profile().unwrap();
        // Deliberately does NOT follow `root`.
        assert!(follower
            .import_followed_site_bundle(bundle, root.to_vec())
            .is_err());
    }

    /// (3) FAMILY GATE: a bundle carrying a COMMUNAL entry (a real newswire post,
    /// valid under communal admission) is rejected — only owned /mod, /articles,
    /// /manifest may ride this channel.
    #[test]
    fn a_bundle_carrying_a_communal_entry_is_rejected() {
        // A genuine communal newswire post bundle (the FFI returns the signed bytes).
        let poster = open_local_profile().unwrap();
        let space = poster
            .create_newswire_space(NewswireSpaceInput {
                name: "Wire".into(),
                summary: "Open wire.".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
            })
            .unwrap();
        let post = poster
            .create_newswire_post(NewswirePostInput {
                space_descriptor_entry_id: space.entry_id.clone(),
                headline: "Communal".into(),
                body: "Not owned.".into(),
                language: "en".into(),
                event_time_unix_seconds: None,
                expires_at_unix_seconds: None,
                coarse_location: None,
                source_claims: vec![],
                operational_profile: None,
                ai_assisted: false,
            })
            .unwrap();

        let (_masthead, _sealed, root) = sealed_masthead();
        let follower = open_local_profile().unwrap();
        follower.follow_site_for_test(root.to_vec()).unwrap();
        assert!(
            follower
                .import_followed_site_bundle(post.signed_bytes, root.to_vec())
                .is_err(),
            "a communal entry must not ride the owned-site import channel"
        );
    }

    /// (4) An owned entry NOT rooted at the followed site is refused end-to-end: it
    /// is non-admissible under `Some(followed_root)`, so the whole bundle rejects.
    #[test]
    fn an_owned_entry_not_rooted_at_the_followed_site_is_rejected() {
        let (masthead_a, _sa, _root_a) = sealed_masthead();
        let (revoke_a, _id) = revoke(&masthead_a, b"r1", [1; 32]);
        let bundle_a = encode_bundle(&[revoke_a]).expect("bundle");

        let (_masthead_b, _sb, root_b) = sealed_masthead();
        let follower = open_local_profile().unwrap();
        follower.follow_site_for_test(root_b.to_vec()).unwrap();
        // A's /mod entry is not rooted at B → admission rejects it under Some(B).
        assert!(follower
            .import_followed_site_bundle(bundle_a, root_b.to_vec())
            .is_err());
    }

    /// (5) ISOLATION INVARIANT: importing an owned /mod bundle leaves the community
    /// sync inventory UNCHANGED — owned-ns records must never enter the active
    /// community's peer offer set.
    #[test]
    fn importing_a_mod_bundle_leaves_the_sync_inventory_unchanged() {
        let (masthead, _sealed, root) = sealed_masthead();
        let bundle = shared_mod_bundle(&masthead);
        let follower = open_local_profile().unwrap();
        follower.follow_site_for_test(root.to_vec()).unwrap();

        let before = with_active(&follower.inner, |p| Ok(p.sync_inventory.len())).unwrap();
        follower
            .import_followed_site_bundle(bundle, root.to_vec())
            .unwrap();
        let after = with_active(&follower.inner, |p| Ok(p.sync_inventory.len())).unwrap();
        assert_eq!(
            before, after,
            "owned /mod records must NOT enter the community sync inventory"
        );
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
