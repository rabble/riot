//! Signing a profile card into a detached Willow entry.
//!
//! [`resolver::write_profile_card`](super::resolver::write_profile_card) commits
//! a card to a live [`EvidenceStore`](crate::store::EvidenceStore). This produces
//! the same signed record as a standalone [`SignedWillowEntry`] for callers that
//! serialize records OUT of a store — the `export-newswire` xtask, which builds a
//! signed fixture in-process. It is the profile twin of
//! [`newswire::create_signed_news_post`](crate::newswire::create_signed_news_post):
//! the author signs a record it is authorised to write, and the caller owns
//! persistence. Factored here as the supported public path rather than duplicating
//! the signing primitives at each call site.

use crate::willow::identity::EvidenceAuthor;
use crate::willow::{authorise_entry, encode_capability, encode_entry, Entry, SignedWillowEntry};

use super::card::{encode_profile_card, ProfileCard};
use super::path::profile_card_path;
use super::ProfileError;

/// Signs `card` into a [`SignedWillowEntry`] at the author's canonical profile
/// slot (`profile/<subspace>/card`), carrying the same proof bytes
/// (entry / capability / signature) as any other signed Willow record.
///
/// `willow_timestamp_micros` is the entry's Willow (TAI/J2000 µs) timestamp; the
/// single card slot is last-write-wins, so a rewrite must use a strictly later
/// timestamp. The name inside `card` is self-claimed and unverified — a reader
/// must still pass it through [`render_display_name`](super::resolver::render_display_name).
pub fn create_signed_profile_card(
    author: &EvidenceAuthor,
    card: &ProfileCard,
    willow_timestamp_micros: u64,
) -> Result<SignedWillowEntry, ProfileError> {
    let payload = encode_profile_card(card)?;
    let path = profile_card_path(author.subspace_id().as_bytes())?;
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(willow_timestamp_micros)
        .payload(&payload)
        .build();
    let authorised = authorise_entry(author, entry).map_err(ProfileError::Willow)?;
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    Ok(SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::card::decode_profile_card;
    use crate::profile::path::classify_profile_path;
    use crate::willow::{
        decode_capability_canonic, decode_entry_canonic, generate_communal_author, verify_entry,
        AuthorisationToken,
    };
    use willow25::groupings::Keylike;
    use willow25::prelude::SubspaceSignature;

    #[test]
    fn a_minted_card_is_a_verifiable_entry_at_the_authors_own_slot() {
        let author = generate_communal_author().expect("author");
        let card = ProfileCard {
            display_name: "Harbor Desk".to_string(),
        };

        let signed = create_signed_profile_card(&author, &card, 1_000_000).expect("mint card");

        // The payload round-trips to the same card.
        assert_eq!(
            decode_profile_card(&signed.payload_bytes).expect("decode payload"),
            card,
        );

        // The entry sits at the author's OWN card slot — the path classifies to
        // the author's subspace, and the entry's subspace matches it.
        let entry = decode_entry_canonic(&signed.entry_bytes).expect("decode entry");
        let slot = classify_profile_path(entry.path()).expect("a profile card path");
        assert_eq!(
            &slot,
            author.subspace_id().as_bytes(),
            "card is at its own slot"
        );
        assert_eq!(
            entry.subspace_id().as_bytes(),
            author.subspace_id().as_bytes()
        );

        // The real Ed25519 signature verifies — same check the export verifier runs.
        let capability = decode_capability_canonic(&signed.capability_bytes).expect("decode cap");
        let token = AuthorisationToken::new(capability, SubspaceSignature::from(signed.signature));
        assert!(
            verify_entry(&entry, &token),
            "the minted card signature verifies"
        );
    }

    #[test]
    fn an_empty_display_name_is_rejected_before_signing() {
        let author = generate_communal_author().expect("author");
        let card = ProfileCard {
            display_name: String::new(),
        };
        assert_eq!(
            create_signed_profile_card(&author, &card, 1_000_000),
            Err(ProfileError::FieldInvalid),
        );
    }
}
