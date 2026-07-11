//! Minimal hex codec for the conference fixture sign/verify tools. The
//! workspace has no `hex` crate dependency; this mirrors the existing
//! hand-rolled helpers (`sha256_hex` in `xtask::main`, `decode_hex` in
//! `riot-core`'s `conference_fixture.rs` test).

pub fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn decode(value: &str, label: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{label} must be lowercase hexadecimal"));
    }
    (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16)
                .map_err(|_| format!("{label} must be lowercase hexadecimal"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_arbitrary_bytes() {
        let bytes = [0u8, 1, 2, 254, 255, 16, 128];
        assert_eq!(decode(&encode(&bytes), "test").unwrap(), bytes);
    }

    #[test]
    fn rejects_odd_length() {
        assert!(decode("abc", "test").is_err());
    }

    #[test]
    fn rejects_non_hex() {
        assert!(decode("zz", "test").is_err());
    }
}
