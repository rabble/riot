use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// An entry, together with an authorisation token that may or may not [authorise](https://willowprotocol.org/specs/data-model/index.html#is_authorised_write) the entry.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::PossiblyAuthorisedEntry;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let my_entry = Entry::builder()
    ///     .namespace_id(namespace_id.clone())
    ///     .subspace_id(subspace_id.clone())
    ///     .path(path!("/ideas"))
    ///     .timestamp(12345)
    ///     .payload(b"chocolate with mustard")
    ///     .build();
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// let auth_token = AuthorisationToken::new_for_entry(&my_entry, &cap, &secret).unwrap();
    ///
    /// let pae = PossiblyAuthorisedEntry::new(
    ///     my_entry.clone(),
    ///     auth_token,
    /// );
    /// assert!(pae.into_authorised_entry().is_ok());
    /// # }
    /// ```
    ///
    /// [Specification](https://willowprotocol.org/specs/data-model/index.html#PossiblyAuthorisedEntry)
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    PossiblyAuthorisedEntry; wdm::PossiblyAuthorisedEntry<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest, AuthorisationToken>
}

impl fmt::Debug for PossiblyAuthorisedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PossiblyAuthorisedEntry {
    /// Creates a new [`PossiblyAuthorisedEntry`] from an [`Entry`] and an [`AuthorisationToken`].
    pub fn new(entry: Entry, authorisation_token: AuthorisationToken) -> Self {
        wdm::PossiblyAuthorisedEntry::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            SubspaceId,
            PayloadDigest,
            AuthorisationToken,
        > {
            entry: entry.into(),
            authorisation_token,
        }
        .into()
    }

    /// Returns a reference to the entry.
    pub fn entry(&self) -> &Entry {
        (&self.0.entry).into()
    }

    /// Sets the entry.
    pub fn set_entry(&mut self, entry: Entry) {
        self.0.entry = entry.into()
    }

    /// Returns a reference to the authorisation token.
    pub fn authorisation_token(&self) -> &AuthorisationToken {
        &self.0.authorisation_token
    }

    /// Sets the authorisation token.
    pub fn set_authorisation_token(&mut self, authorisation_token: AuthorisationToken) {
        self.0.authorisation_token = authorisation_token
    }

    /// Takes ownership of `self` and returns the entry and authorisation token by value.
    pub fn into_parts(self) -> (Entry, AuthorisationToken) {
        (self.0.entry.into(), self.0.authorisation_token)
    }

    /// Checks whether `self.authorisation_token` [authorises](https://willowprotocol.org/specs/data-model/index.html#is_authorised_write) `self.entry`. If so, converts self into an [`AuthorisedEntry`], otherwise returns `Err(self)`.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::PossiblyAuthorisedEntry;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let my_entry = Entry::builder()
    ///     .namespace_id(namespace_id.clone())
    ///     .subspace_id(subspace_id.clone())
    ///     .path(path!("/ideas"))
    ///     .timestamp(12345)
    ///     .payload(b"chocolate with mustard")
    ///     .build();
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// let auth_token = AuthorisationToken::new_for_entry(&my_entry, &cap, &secret).unwrap();
    ///
    /// let pae = PossiblyAuthorisedEntry::new(
    ///     my_entry.clone(),
    ///     auth_token.clone(),
    /// );
    /// assert!(pae.into_authorised_entry().is_ok());
    ///
    /// let someone_elses_entry = Entry::prefilled_builder(&my_entry)
    ///     .subspace_id(SubspaceId::from_bytes(&[18; 32]))
    ///     .build();
    ///
    /// let pae2 = PossiblyAuthorisedEntry::new(
    ///     someone_elses_entry.clone(),
    ///     auth_token,
    /// );
    /// assert!(pae2.into_authorised_entry().is_err());
    /// # }
    /// ```
    #[allow(clippy::result_large_err)]
    pub fn into_authorised_entry(self) -> Result<AuthorisedEntry, Self> {
        self.0
            .into_authorised_entry()
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Converts self into an [`AuthorisedEntry`], without checking if `self.authorisation_token` [authorise](https://willowprotocol.org/specs/data-model/index.html#is_authorised_write) `self.entry`.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::PossiblyAuthorisedEntry;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id(namespace_id.clone())
    ///     .subspace_id(subspace_id.clone())
    ///     .path(path!("/ideas"))
    ///     .timestamp(12345)
    ///     .payload(b"chocolate with mustard")
    ///     .build();
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// let auth_token = AuthorisationToken::new_for_entry(&entry, &cap, &secret).unwrap();
    ///
    /// let pae = PossiblyAuthorisedEntry::new(
    ///     entry.clone(),
    ///     auth_token.clone(),
    /// );
    ///
    /// let authed = unsafe { pae.into_authorised_entry_unchecked() };
    /// assert_eq!(authed.entry(), &entry);
    /// assert_eq!(authed.authorisation_token(), &auth_token);
    /// # }
    /// ```
    ///
    /// #### Safety
    ///
    /// Undefined behaviour may occur if `self.authorisation_token.does_authorise(&self.entry)` is `false`. If it returns `true`, this method is safe to call.
    pub unsafe fn into_authorised_entry_unchecked(self) -> AuthorisedEntry {
        // SAFETY: the requirements for `self.0.into_authorised_entry_unchecked` to be safe are exactly those
        // required by this function itself.
        unsafe { self.0.into_authorised_entry_unchecked() }.into()
    }
}

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for PossiblyAuthorisedEntry {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        self.0.wdm_subspace_id()
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        self.0.wdm_path()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for PossiblyAuthorisedEntry {
    fn wdm_timestamp(&self) -> Timestamp {
        self.0.wdm_timestamp()
    }
}

impl wdm::Namespaced<NamespaceId> for PossiblyAuthorisedEntry {
    fn wdm_namespace_id(&self) -> &NamespaceId {
        self.0.wdm_namespace_id()
    }
}

impl wdm::Entrylike<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>
    for PossiblyAuthorisedEntry
{
    fn wdm_payload_length(&self) -> u64 {
        self.0.wdm_payload_length()
    }

    fn wdm_payload_digest(&self) -> &PayloadDigest {
        self.0.wdm_payload_digest()
    }
}

wrapper! {
    /// An entry, together with an authorisation token that [authorises](https://willowprotocol.org/specs/data-model/index.html#is_authorised_write) the entry.
    ///
    /// There are two typical scenarios for creating these: authorising an [`Entry`] yourself (via [`Entry::into_authorised_entry`] or [`crate::entry::EntrylikeExt::authorise`]), or receiving and verifying an untrusted authorisation token ([`PossiblyAuthorisedEntry::into_authorised_entry`]).
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::PossiblyAuthorisedEntry;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id(namespace_id.clone())
    ///     .subspace_id(subspace_id.clone())
    ///     .path(path!("/ideas"))
    ///     .timestamp(12345)
    ///     .payload(b"chocolate with mustard")
    ///     .build();
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// let authed = entry.authorise(&cap, &secret);
    /// let authed_alternative = entry.into_authorised_entry(&cap, &secret);
    ///
    /// assert_eq!(authed, authed_alternative);
    /// # }
    /// ```
    ///
    /// [Specification](https://willowprotocol.org/specs/data-model/index.html#AuthorisedEntry)
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    AuthorisedEntry; wdm::AuthorisedEntry<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest, AuthorisationToken>
}

impl fmt::Debug for AuthorisedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AuthorisedEntry {
    /// Consumes self, returning the entry and the authorisation token.
    pub fn into_parts(self) -> (Entry, AuthorisationToken) {
        let (entry, at) = self.0.into_parts();
        (entry.into(), at)
    }

    /// Returns a reference to the entry.
    pub fn entry(&self) -> &Entry {
        self.0.entry().into()
    }

    /// Returns a reference to the authorisation token.
    pub fn authorisation_token(&self) -> &AuthorisationToken {
        self.0.authorisation_token()
    }
}

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for AuthorisedEntry {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        self.0.wdm_subspace_id()
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        self.0.wdm_path()
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for AuthorisedEntry {
    fn wdm_timestamp(&self) -> Timestamp {
        self.0.wdm_timestamp()
    }
}

impl wdm::Namespaced<NamespaceId> for AuthorisedEntry {
    fn wdm_namespace_id(&self) -> &NamespaceId {
        self.0.wdm_namespace_id()
    }
}

impl wdm::Entrylike<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest> for AuthorisedEntry {
    fn wdm_payload_length(&self) -> u64 {
        self.0.wdm_payload_length()
    }

    fn wdm_payload_digest(&self) -> &PayloadDigest {
        self.0.wdm_payload_digest()
    }
}
