//! `OwnedMasthead` — the composite-site owner identity.
//!
//! Combines the owned-namespace root secret (authority to mint the owner write
//! capability and to delegate) with the owner's own subspace signing secret
//! (the author key the owner writes entries as, and the signer for delegations).
//! Unit 0 scope: generation, owner capability minting, section delegation
//! issuance (reserved-path enforced), and sealed persistence.

use willow25::prelude::*;
use zeroize::Zeroize;

use super::identity::os_fill;
use super::owned::OwnedRoot;
use super::WillowError;
use crate::willow::site_paths::is_under_articles;

pub struct OwnedMasthead {
    root: OwnedRoot,
    owner_subspace_secret: SubspaceSecret,
}

impl OwnedMasthead {
    /// Generate a fresh masthead: a new owned namespace root + a fresh owner
    /// subspace secret.
    pub fn generate() -> Result<Self, WillowError> {
        let root = OwnedRoot::generate()?;
        let mut seed = [0u8; 32];
        os_fill(&mut seed)?;
        let owner_subspace_secret = SubspaceSecret::from_bytes(&seed);
        seed.zeroize();
        Ok(Self {
            root,
            owner_subspace_secret,
        })
    }

    /// The owned namespace id (site root of trust).
    pub fn namespace_id(&self) -> &NamespaceId {
        self.root.namespace_id()
    }

    /// The owner's subspace id (receiver of the owner write capability).
    pub fn owner_subspace_id(&self) -> SubspaceId {
        self.owner_subspace_secret.corresponding_subspace_id()
    }

    /// Mint the owner's owned write capability (grants `Area::full()` over the site namespace).
    pub fn owner_write_capability(&self) -> WriteCapability {
        WriteCapability::new_owned(self.root.namespace_secret_ref(), self.owner_subspace_id())
    }

    /// Delegate a section-scoped, time-boxed write capability to an editor.
    /// REFUSES (belt) any `new_area` whose path is not under `/articles/`, so the
    /// owner can never mint a cap reaching `/manifest` or `/mod/`.
    pub fn delegate_section(
        &self,
        editor_subspace_id: SubspaceId,
        new_area: Area,
    ) -> Result<WriteCapability, WillowError> {
        if !is_under_articles(new_area.path()) {
            return Err(WillowError::DelegationAreaEscapesArticles);
        }
        let mut cap = self.owner_write_capability();
        cap.try_delegate(&self.owner_subspace_secret, new_area, editor_subspace_id)
            .map_err(|_| WillowError::DoesNotAuthorise)?;
        Ok(cap)
    }

    /// Sign an entry as the site owner (owner cap, granted `Area::full()`).
    /// The signer is the owner's SubspaceSecret — the namespace secret is used only
    /// when *minting* the cap, never when authorising an entry.
    pub fn authorise_owner_entry(&self, entry: Entry) -> Result<AuthorisedEntry, WillowError> {
        entry
            .into_authorised_entry(&self.owner_write_capability(), &self.owner_subspace_secret)
            .map_err(|_| WillowError::DoesNotAuthorise)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::willow::site_paths::ARTICLES_COMPONENT;
    use crate::willow::{verify_entry, MANIFEST_COMPONENT};

    fn a_time_range() -> TimeRange {
        TimeRange::new(0u64.into(), Some(u64::MAX.into()))
    }

    fn entry_in(namespace: &NamespaceId, subspace: SubspaceId, path: &[&[u8]]) -> Entry {
        Entry::builder()
            .namespace_id(namespace.clone())
            .subspace_id(subspace)
            .path(Path::from_slices(path).expect("path"))
            .timestamp(1_000u64)
            .payload(b"payload-bytes")
            .build()
    }

    #[test]
    fn owner_capability_authorises_and_verifies() {
        let m = OwnedMasthead::generate().unwrap();
        let entry = entry_in(
            m.namespace_id(),
            m.owner_subspace_id(),
            &[MANIFEST_COMPONENT],
        );
        let authorised = m
            .authorise_owner_entry(entry.clone())
            .expect("owner authorises");
        assert!(
            verify_entry(&entry, authorised.authorisation_token()),
            "owner-signed entry must verify"
        );
    }

    #[test]
    fn delegated_editor_can_write_articles_but_not_manifest() {
        let m = OwnedMasthead::generate().unwrap();
        let editor = SubspaceSecret::from_bytes(&[11u8; 32]);
        let editor_id = editor.corresponding_subspace_id();
        let area = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[ARTICLES_COMPONENT, b"news"]).expect("path"),
            a_time_range(),
        );
        let editor_cap = m
            .delegate_section(editor_id.clone(), area)
            .expect("delegate");

        // POSITIVE: entry under /articles/news signed by the editor authorises.
        let good = entry_in(
            m.namespace_id(),
            editor_id.clone(),
            &[ARTICLES_COMPONENT, b"news", b"post-1"],
        );
        let authorised = good
            .clone()
            .into_authorised_entry(&editor_cap, &editor)
            .expect("editor authorises");
        assert!(verify_entry(&good, authorised.authorisation_token()));

        // NEGATIVE: same editor cap cannot author a /manifest entry (outside granted area).
        let bad = entry_in(m.namespace_id(), editor_id, &[MANIFEST_COMPONENT]);
        assert!(
            bad.into_authorised_entry(&editor_cap, &editor).is_err(),
            "a delegated /articles cap must NOT authorise a /manifest write"
        );
    }

    #[test]
    fn delegate_section_under_articles_succeeds_and_scopes_receiver() {
        let m = OwnedMasthead::generate().unwrap();
        let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
        let editor_id = editor.corresponding_subspace_id();
        let area = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[ARTICLES_COMPONENT, b"news"]).expect("path"),
            a_time_range(),
        );
        let cap = m
            .delegate_section(editor_id.clone(), area)
            .expect("delegation under /articles must succeed");
        assert!(
            !cap.delegations().is_empty(),
            "delegated cap must carry a delegation link"
        );
        assert_eq!(
            cap.receiver(),
            &editor_id,
            "final receiver must be the editor"
        );
        assert_eq!(cap.granted_namespace(), m.namespace_id());
    }

    #[test]
    fn delegate_escaping_articles_is_refused() {
        let m = OwnedMasthead::generate().unwrap();
        let editor = SubspaceSecret::from_bytes(&[9u8; 32]);
        let editor_id = editor.corresponding_subspace_id();
        let bad_area = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[crate::willow::site_paths::MANIFEST_COMPONENT]).expect("path"),
            a_time_range(),
        );
        assert!(
            matches!(
                m.delegate_section(editor_id, bad_area),
                Err(WillowError::DelegationAreaEscapesArticles)
            ),
            "a delegation whose area escapes /articles must be refused"
        );
    }

    #[test]
    fn generated_masthead_has_owned_namespace_and_owner_subspace() {
        let m = OwnedMasthead::generate().expect("masthead");
        assert!(
            m.namespace_id().is_owned(),
            "masthead namespace must be owned"
        );
        assert_eq!(m.owner_subspace_id(), m.owner_subspace_id());
    }

    #[test]
    fn owner_capability_is_owned_full_area_zero_delegation() {
        let m = OwnedMasthead::generate().unwrap();
        let cap = m.owner_write_capability();
        assert!(cap.is_owned(), "owner cap must be owned-rooted");
        assert!(
            cap.delegations().is_empty(),
            "owner cap must have zero delegations"
        );
        assert_eq!(
            cap.granted_namespace(),
            m.namespace_id(),
            "cap namespace must be the site root"
        );
        assert_eq!(
            cap.receiver(),
            &m.owner_subspace_id(),
            "cap receiver must be the owner subspace"
        );
        assert_eq!(cap.granted_area(), willow25::prelude::Area::full());
    }
}
