//! `OwnedMasthead` — the composite-site owner identity.
//!
//! Combines the owned-namespace root secret (authority to mint the owner write
//! capability and to delegate) with the owner's own subspace signing secret
//! (the author key the owner writes entries as, and the signer for delegations).
//! Unit 0 scope: generation, owner capability minting, section delegation
//! issuance (reserved-path enforced), and sealed persistence.

use willow25::prelude::*;
use zeroize::{Zeroize, Zeroizing};

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};

use super::identity::os_fill;
use super::owned::OwnedRoot;
use super::WillowError;
use crate::willow::site_paths::is_under_articles;

const MASTHEAD_MAGIC: &[u8] = b"RIOTMH\x01\0";
const MASTHEAD_AAD: &[u8] = b"riot/owned-masthead/sealed/v1";
const SEALED_MASTHEAD_PLAINTEXT: usize = 64;
const SEALED_MASTHEAD_NONCE_BYTES: usize = 24;
const SEALED_MASTHEAD_TAG_BYTES: usize = 16;
const SEALED_MASTHEAD_BYTES: usize = MASTHEAD_MAGIC.len()
    + SEALED_MASTHEAD_NONCE_BYTES
    + SEALED_MASTHEAD_PLAINTEXT
    + SEALED_MASTHEAD_TAG_BYTES;

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

    /// Authenticates and encrypts the owner's root secret material (namespace
    /// root secret ‖ owner subspace secret) under an application-provided 32-byte
    /// wrapping key. Only the opaque fixed-size blob is persisted; plaintext
    /// secret material is zeroized immediately. Mirrors the identity sealing
    /// construction but with a distinct magic/AAD and an owned-namespace payload.
    pub fn seal(&self, wrapping_key: &[u8; 32]) -> Result<Vec<u8>, WillowError> {
        let mut nonce = [0u8; SEALED_MASTHEAD_NONCE_BYTES];
        os_fill(&mut nonce)?;

        let mut plaintext = Zeroizing::new([0u8; SEALED_MASTHEAD_PLAINTEXT]);
        plaintext[..32].copy_from_slice(self.root.namespace_secret_ref().as_bytes());
        plaintext[32..].copy_from_slice(self.owner_subspace_secret.as_bytes());
        let cipher = XChaCha20Poly1305::new(Key::from_slice(wrapping_key));
        let encrypted = cipher.encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext[..],
                aad: MASTHEAD_AAD,
            },
        );
        let ciphertext = encrypted.map_err(|_| WillowError::IdentitySealFailed)?;

        let mut sealed = Vec::with_capacity(SEALED_MASTHEAD_BYTES);
        sealed.extend_from_slice(MASTHEAD_MAGIC);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&ciphertext);
        nonce.zeroize();
        debug_assert_eq!(sealed.len(), SEALED_MASTHEAD_BYTES);
        Ok(sealed)
    }

    /// Restores an owner masthead only after the fixed envelope and AEAD tag
    /// validate and the decoded namespace reconstructs as an owned root. No
    /// partially constructed masthead is returned on any failure.
    pub fn open_sealed(wrapping_key: &[u8; 32], sealed: &[u8]) -> Result<Self, WillowError> {
        if sealed.len() != SEALED_MASTHEAD_BYTES
            || &sealed[..MASTHEAD_MAGIC.len()] != MASTHEAD_MAGIC
        {
            return Err(WillowError::SealedMastheadInvalid);
        }
        let nonce_start = MASTHEAD_MAGIC.len();
        let ciphertext_start = nonce_start + SEALED_MASTHEAD_NONCE_BYTES;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(wrapping_key));
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&sealed[nonce_start..ciphertext_start]),
                Payload {
                    msg: &sealed[ciphertext_start..],
                    aad: MASTHEAD_AAD,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| WillowError::SealedMastheadInvalid)?;
        if plaintext.len() != SEALED_MASTHEAD_PLAINTEXT {
            return Err(WillowError::SealedMastheadInvalid);
        }

        let mut namespace_secret_bytes = Zeroizing::new([0u8; 32]);
        namespace_secret_bytes.copy_from_slice(&plaintext[..32]);
        let mut owner_subspace_secret_bytes = Zeroizing::new([0u8; 32]);
        owner_subspace_secret_bytes.copy_from_slice(&plaintext[32..]);

        let root =
            OwnedRoot::from_namespace_secret(NamespaceSecret::from_bytes(&namespace_secret_bytes))?;
        let owner_subspace_secret = SubspaceSecret::from_bytes(&owner_subspace_secret_bytes);
        Ok(Self {
            root,
            owner_subspace_secret,
        })
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
    fn sealed_masthead_roundtrips_and_hides_secrets() {
        let m = OwnedMasthead::generate().unwrap();
        let ns = m.namespace_id().clone();
        let owner = m.owner_subspace_id();
        let key = [0x5a; 32];

        let sealed = m.seal(&key).expect("seal");
        // secret bytes must not appear in cleartext in the sealed blob
        let secret_bytes = m.root.namespace_secret_ref().as_bytes();
        assert!(sealed.windows(32).all(|w| w != secret_bytes));

        let restored = OwnedMasthead::open_sealed(&key, &sealed).expect("open");
        assert_eq!(
            *restored.namespace_id(),
            ns,
            "namespace id survives roundtrip"
        );
        assert_eq!(
            restored.owner_subspace_id(),
            owner,
            "owner subspace survives roundtrip"
        );
    }

    #[test]
    fn open_sealed_masthead_rejects_wrong_key() {
        let m = OwnedMasthead::generate().unwrap();
        let sealed = m.seal(&[0x01; 32]).unwrap();
        assert!(
            matches!(
                OwnedMasthead::open_sealed(&[0x02; 32], &sealed),
                Err(WillowError::SealedMastheadInvalid)
            ),
            "wrong wrapping key must fail closed"
        );
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
