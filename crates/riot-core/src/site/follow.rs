//! The single canonical followed-site admission gate — the SECURITY boundary
//! that decides which bytes from a followed composite site may enter the store.
//!
//! Shared by every delivery path so the gate cannot drift between them: the
//! manual Option B import (`import_followed_site_bundle`), the WU2 followed-site
//! sync session (`prepare_followed_site_import`), and the WU3 transport follower
//! (the `pump` `on_bundle` hook). One admits exactly what the others admit.

use crate::import::{decode_bundle_with_root, BundleDecodeOutcome, ItemStatus};
use crate::session::{CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, SessionError};
use crate::willow::decode_entry_canonic;
use crate::willow::site_paths::{is_owned_editorial_entry, is_owned_moderation_entry};
use crate::willow::Entry;

/// Why a followed-site frame was refused. `Rejected` is the fail-closed refusal
/// (decode, family gate, cap binding, or eligible-count mismatch); `Store` is an
/// underlying store/commit fault (e.g. `StoreFull`) a caller may map differently.
#[derive(Debug)]
pub enum FollowedSiteAdmitError {
    Rejected,
    Store(SessionError),
}

impl From<SessionError> for FollowedSiteAdmitError {
    fn from(error: SessionError) -> Self {
        Self::Store(error)
    }
}

/// Whether `entry` is an owned composite-site record a followed-site delivery may
/// carry: `/mod` (moderation) or `/articles` (editorial) ONLY. Least-privilege —
/// exactly the families a store reader consumes; `/manifest` is validated from a
/// caller argument, never read from the store, so admitting it here would only
/// widen the surface. Anything else (a communal alert/newswire entry) is never
/// admissible on this channel.
pub fn is_followed_site_family(entry: &Entry) -> bool {
    is_owned_moderation_entry(entry) || is_owned_editorial_entry(entry)
}

/// Admit a followed-site frame under `root` and commit it, all-or-nothing.
///
/// The one canonical gate: (1) decode under `Some(root)` — an entry not rooted at
/// `root` is Invalid and rejects the whole bundle; (2) FAMILY-gate every item to
/// owned /mod + /articles; (3) inspect under `followed_root = root` and require
/// the eligible count to equal the family-gated count (a divergence rejects the
/// whole bundle); (4) plan + commit. Returns the number of records committed.
///
/// Does NOT touch any sync inventory: that isolation lives in the FFI caller and
/// this store-level gate structurally cannot reach it.
pub fn admit_followed_site_frame(
    store: &EvidenceStore,
    root: [u8; 32],
    bytes: &[u8],
    route: &str,
) -> Result<u32, FollowedSiteAdmitError> {
    // 1. Decode under the followed root: an entry not rooted at `root` (or one
    //    whose schema is not an admissible owned family) is Invalid, and a single
    //    Invalid item rejects the WHOLE bundle (fail-closed, all-or-nothing).
    let decoded = match decode_bundle_with_root(bytes, Some(root)) {
        BundleDecodeOutcome::Decoded(decoded) => decoded,
        BundleDecodeOutcome::Rejected(_) => return Err(FollowedSiteAdmitError::Rejected),
    };
    let mut count = 0u32;
    for item in &decoded.items {
        let ItemStatus::Valid(_) = &item.status else {
            return Err(FollowedSiteAdmitError::Rejected);
        };
        // 2. Family gate — defense-in-depth over the decode schema gate: only
        //    owned /mod + /articles may ride this channel. (Today the decode
        //    schema gate already excludes every other owned path, so this is a
        //    belt-and-suspenders check that survives a future schema loosening.)
        let entry = decode_entry_canonic(item.frame.entry_bytes())
            .map_err(|_| FollowedSiteAdmitError::Rejected)?;
        if !is_followed_site_family(&entry) {
            return Err(FollowedSiteAdmitError::Rejected);
        }
        count += 1;
    }
    if count == 0 {
        return Err(FollowedSiteAdmitError::Rejected);
    }

    // 3. Admit under followed_root = root, and require the eligible count to
    //    equal the family-gated count — a divergence rejects the whole bundle.
    let preview = match store.inspect(bytes, ImportContext::with_followed_root(route, root))? {
        InspectOutcome::Preview(preview) => preview,
        InspectOutcome::Rejected(_) => return Err(FollowedSiteAdmitError::Rejected),
    };
    if preview.eligible_count()? != count as usize {
        return Err(FollowedSiteAdmitError::Rejected);
    }

    // 4. Plan + commit. Does NOT touch any sync inventory.
    let plan = preview.plan_all()?;
    match plan.commit()? {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
    Ok(count)
}
