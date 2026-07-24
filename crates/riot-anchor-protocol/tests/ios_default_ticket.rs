//! Cross-language guard for the iOS app's baked River City Wire ticket.
//!
//! The app ships this ticket as configuration, so the Rust authority decoder is
//! the source of truth for whether its validity window is actually durable.

use riot_anchor_protocol::codec::decode_canonical;
use riot_anchor_protocol::records::RootSignedTicketCoreEnvelopeV2;

const EIGHTY_DAYS_SECONDS: u64 = 80 * 24 * 60 * 60;
const NINETY_DAYS_SECONDS: u64 = 90 * 24 * 60 * 60;

fn ios_default_ticket_hex() -> String {
    let app_model = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/ios/Riot/AppModel.swift"
    ))
    .expect("read iOS AppModel.swift");
    let declaration = app_model
        .split("public static let communityTicketHex =")
        .nth(1)
        .expect("communityTicketHex declaration");
    declaration
        .split('"')
        .nth(1)
        .expect("communityTicketHex string literal")
        .to_owned()
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "ticket hex has an even length");
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).expect("ticket hex is UTF-8");
            u8::from_str_radix(digits, 16).expect("ticket is valid hex")
        })
        .collect()
}

#[test]
fn ios_default_ticket_has_a_durable_authority_window() {
    let bytes = decode_hex(&ios_default_ticket_hex());
    let envelope: RootSignedTicketCoreEnvelopeV2 =
        decode_canonical(&bytes, 1024).expect("ticket is canonical");
    let lifetime = envelope
        .core
        .expiry_unix_seconds
        .checked_sub(envelope.core.issued_unix_seconds)
        .expect("ticket expiry follows issuance");

    assert!(
        lifetime >= EIGHTY_DAYS_SECONDS,
        "baked iOS ticket lasts only {lifetime} seconds"
    );
    assert!(
        lifetime <= NINETY_DAYS_SECONDS,
        "baked iOS ticket exceeds the authority's 90-day maximum"
    );
}
