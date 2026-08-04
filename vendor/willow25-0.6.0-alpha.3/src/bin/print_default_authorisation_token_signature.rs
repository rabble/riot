//! Computes and prints the signature for the [`default_authorisation_token`].
use ed25519_dalek::{SIGNATURE_LENGTH, Signature};
use willow25::defaults::default_authorisation_token;
use willow25::prelude::*;

fn main() {
    let default_entry = Entry::builder()
        .default_namespace_id()
        .default_subspace_id()
        .path(Path::new())
        .timestamp(0)
        .default_payload()
        .build();
    println!("default entry: {default_entry:?}");
    println!(
        "default payload digest bytes: {:?}",
        default_entry.payload_digest().as_bytes()
    );

    let token = default_authorisation_token();
    let signature = token.signature().clone();
    let inner: Signature = signature.into();
    let bytes: [u8; SIGNATURE_LENGTH] = inner.to_bytes();
    println!("{:?}", bytes);
}
