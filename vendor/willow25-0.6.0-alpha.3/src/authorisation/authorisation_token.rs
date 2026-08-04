use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use meadowcap::DoesNotAuthorise;

use ufotofu::codec::{Blame, Decodable, DecodeError, Encodable};
use ufotofu::codec_relative::{RelativeDecodable, RelativeEncodable};
use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// An [authorisation token](https://willowprotocol.org/specs/data-model/index.html#AuthorisationToken), cryptographically certifying that an entry was created by somebody who had the permission to do so.
    ///
    /// #### Examples
    ///
    /// Authorising an entry of your own:
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
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
    /// assert!(auth_token.does_authorise(&my_entry));
    /// # }
    /// ```
    ///
    /// Verifying an untrusted entry, creating the authorisation token from raw parts, which fails if those parts are invalid:
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
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
    /// let auth_token = AuthorisationToken::new(
    ///     cap,
    ///     SubspaceSignature::from([18; 64]), // Invalid signature, verification will fail.
    /// );
    ///
    /// assert!(!auth_token.does_authorise(&my_entry));
    /// # }
    /// ```
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    AuthorisationToken; meadowcap::McAuthorisationToken<MCL, MCC, MPL, NamespaceId, NamespaceSignature, SubspaceId, SubspaceSignature, SubspaceSecret>
}

impl fmt::Debug for AuthorisationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AuthorisationToken {
    /// Manually creates a new [`AuthorisationToken`] from a write capability and a signature (as opposed to using [`AuthorisationToken::new_for_entry`](AuthorisationToken::new_for_entry) to create the correct signature yourself).
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
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
    /// let auth_token = AuthorisationToken::new(
    ///     cap,
    ///     SubspaceSignature::from([18; 64]), // Invalid signature, verification will fail.
    /// );
    ///
    /// assert!(!auth_token.does_authorise(&my_entry));
    /// # }
    /// ```
    pub fn new(capability: WriteCapability, signature: SubspaceSignature) -> Self {
        meadowcap::McAuthorisationToken::new(capability.into(), signature).into()
    }

    /// Returns a reference to the write capability.
    pub fn capability(&self) -> &WriteCapability {
        self.0.capability().into()
    }

    /// Returns a reference to the signature.
    pub fn signature(&self) -> &SubspaceSignature {
        self.0.signature()
    }

    /// Takes ownership of the authorisation token and returns its capability and signature by value.
    pub fn into_parts(self) -> (WriteCapability, SubspaceSignature) {
        let (cap, sig) = self.0.into_parts();
        (cap.into(), sig)
    }

    /// Creates an authorisation token for a given entry and capability by computing the correct signature from the given secret.
    ///
    /// Returns an error if the capability does not grant write access to the entry being authorised.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
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
    /// assert!(auth_token.does_authorise(&my_entry));
    /// # }
    /// ```
    pub fn new_for_entry<E>(
        entry: &E,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
    ) -> Result<Self, DoesNotAuthorise>
    where
        E: EntrylikeExt + ?Sized,
    {
        match McIngredients::new(write_capability.clone(), secret.clone()) {
            Some(ingredients) => wdm::AuthorisationToken::<
                MCL,
                MCC,
                MPL,
                NamespaceId,
                SubspaceId,
                PayloadDigest,
            >::new_for_entry(entry, &ingredients),
            None => Err(DoesNotAuthorise),
        }
    }

    /// Return `true` iff `self` is a valid authorisation token for the given entry.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
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
    /// assert!(auth_token.does_authorise(&my_entry));
    /// # }
    /// ```
    pub fn does_authorise<E>(&self, entry: &E) -> bool
    where
        E: wdm::EntrylikeExt<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest> + ?Sized,
    {
        wdm::AuthorisationToken::<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>::does_authorise(&self.0, entry)
    }
}

/// You only need this trait impl if you work with fully generic code from the [`willow_data_model`] crate.
impl wdm::AuthorisationToken<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>
    for AuthorisationToken
{
    type Ingredients = McIngredients;

    type CreationError = DoesNotAuthorise;

    fn new_for_entry<E>(
        entry: &E,
        ingredients: &Self::Ingredients,
    ) -> Result<Self, Self::CreationError>
    where
        E: wdm::EntrylikeExt<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest> + ?Sized,
    {
        meadowcap::McAuthorisationToken::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
            SubspaceSecret,
        >::new_for_entry(entry, ingredients.into())
        .map(Into::into)
    }

    fn does_authorise<E>(&self, entry: &E) -> bool
    where
        E: wdm::EntrylikeExt<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest> + ?Sized,
    {
        self.0.does_authorise(entry)
    }
}

wrapper! {
    /// Only needed if you work with fully generic code from the [`willow_data_model`] crate; the ingredients for the [`willow_data_model::authorisation::AuthorisationToken`] impl of [`AuthorisationToken`].
    #[derive(PartialEq, Eq, Clone)]
    McIngredients; meadowcap::McIngredients<MCL, MCC, MPL, NamespaceId, NamespaceSignature, SubspaceId, SubspaceSignature, SubspaceSecret>
}

impl fmt::Debug for McIngredients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl McIngredients {
    /// Returns a reference to the write capability.
    pub fn capability(&self) -> &WriteCapability {
        self.0.capability().into()
    }

    /// Returns a reference to the keypair.
    pub fn keypair(&self) -> &SubspaceSecret {
        self.0.keypair()
    }

    /// Takes ownership of the ingredients and returns the capability and keypair by value.
    pub fn into_parts(self) -> (WriteCapability, SubspaceSecret) {
        let (cap, secret) = self.0.into_parts();
        (cap.into(), secret)
    }

    /// Creates new [`McIngredients`], returning `None` if the keypair does not match the receiver of the capability.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::McIngredients;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let (_subspace_id2, secret2) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(McIngredients::new(cap.clone(), secret).is_some());
    /// assert!(McIngredients::new(cap.clone(), secret2).is_none());
    /// # }
    /// ```
    pub fn new(capability: WriteCapability, secret: SubspaceSecret) -> Option<Self> {
        meadowcap::McIngredients::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
            SubspaceSecret,
        >::new(capability.into(), secret)
        .map(Into::into)
    }
}

// The below version of PriorAuthedEntryEntryPair does *not* use the wrapper! macro, and is instead implemented from scratch.
// Similarly, the RelativeEncodable and RelativeDecodable implementations are also duplicated from meadowcap/src/mc_authorisation_token.rs.
// This is to avoid incompatibilities of our wrapper types and use of their internal types with the rust compiler.

/// A pair of a prior [`AuthorisedEntry`] and [`Entry`], used to encoded and decode [`McAuthorisationToken`]s privately.
///
/// [`McAuthorisationToken`]: meadowcap::McAuthorisationToken
#[derive(PartialEq, Clone, Debug)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub struct PriorAuthedEntryEntryPair {
    /// The previous [`AuthorisedEntry`] which was decoded.
    prior_authed_entry: AuthorisedEntry,
    /// An entry included by the [`WriteCapability`] being privately encoded against.
    entry: Entry,
}

impl PriorAuthedEntryEntryPair {
    /// Returns a new [`PriorAuthedEntryEntryPair`] with the given `prior_authed_entry` and `entry`.
    pub fn new(prior_authed_entry: AuthorisedEntry, entry: Entry) -> Self {
        Self {
            prior_authed_entry,
            entry,
        }
    }

    /// Returns the previous [`AuthorisedEntry`] which was decoded against.
    pub fn prior_authed_entry(&self) -> &AuthorisedEntry {
        &self.prior_authed_entry
    }

    /// Returns the entry included by the [`WriteCapability`] being privately encoded against.
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// Consumes `self` to return the underlying [`AuthorisedEntry`] and [`Entry`].
    pub fn into_parts(self) -> (AuthorisedEntry, Entry) {
        (self.prior_authed_entry, self.entry)
    }
}

/// Implements relative encoding according to the [EncodeMeadowcapAuthorisationTokenRelative](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodeMeadowcapAuthorisationTokenRelative) encoding function.
impl RelativeEncodable<PriorAuthedEntryEntryPair> for AuthorisationToken {
    async fn relative_encode<C>(
        &self,
        rel: &PriorAuthedEntryEntryPair,
        consumer: &mut C,
    ) -> Result<(), C::Error>
    where
        C: ufotofu::BulkConsumer<Item = u8> + ?Sized,
    {
        let prev = &rel.prior_authed_entry;
        let entry = &rel.entry;

        let pair = crate::authorisation::PriorCapEntryPair::new(
            prev.authorisation_token().capability().clone(),
            entry.clone(),
        );

        self.capability().relative_encode(&pair, consumer).await?;
        self.signature().encode(consumer).await?;

        Ok(())
    }

    /// Returns false if the relative entry's namespace is not `self`'s [granted area](https://willowprotocol.org/specs/meadowcap/index.html#communal_cap_granted_namespace), or if the relative entry is not included by `self`'s [granted area](https://willowprotocol.org/specs/meadowcap/index.html#communal_cap_granted_area).
    fn can_be_encoded_relative_to(&self, rel: &PriorAuthedEntryEntryPair) -> bool {
        self.capability().granted_namespace() == rel.entry.namespace_id()
            && self.capability().granted_area().includes(&rel.entry)
    }
}

/// Implements relative decoding according to the [EncodeMeadowcapAuthorisationTokenRelative](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodeMeadowcapAuthorisationTokenRelative) encoding function.
impl RelativeDecodable<PriorAuthedEntryEntryPair> for AuthorisationToken {
    type ErrorReason = Blame;

    async fn relative_decode<P>(
        rel: &PriorAuthedEntryEntryPair,
        producer: &mut P,
    ) -> Result<Self, ufotofu::codec::DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: ufotofu::BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let prev = &rel.prior_authed_entry;
        let entry = &rel.entry;

        let pair = crate::authorisation::PriorCapEntryPair::new(
            prev.authorisation_token().capability().clone(),
            entry.clone(),
        );

        let write_cap = WriteCapability::relative_decode(&pair, producer).await?;
        let signature = SubspaceSignature::decode(producer)
            .await
            .map_err(|_| DecodeError::Other(Blame::TheirFault))?;

        Ok(Self::new(write_cap, signature))
    }
}
