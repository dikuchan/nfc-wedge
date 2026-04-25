/// APDU builders and response helpers for NFC tag communication.

/// PC/SC escape command to read memory on Type 2 tags (MIFARE Ultralight / NTAG).
/// `FF B0 00 [page] [len]`
pub fn type2_read_binary(page: u8, len: u8) -> Vec<u8> {
    vec![0xFF, 0xB0, 0x00, page, len]
}

/// SELECT NDEF application by AID for Type 4 tags.
/// `00 A4 04 00 07 D2760000850101 00`
pub fn select_ndef_application() -> Vec<u8> {
    vec![
        0x00, 0xA4, 0x04, 0x00, 0x07,
        0xD2, 0x76, 0x00, 0x00, 0x85, 0x01, 0x01,
        0x00,
    ]
}

/// SELECT NDEF file by ID for Type 4 tags.
/// `00 A4 00 0C 02 00 E1 00`
pub fn select_ndef_file() -> Vec<u8> {
    vec![0x00, 0xA4, 0x00, 0x0C, 0x02, 0x00, 0xE1, 0x00]
}

/// READ BINARY for Type 4 tags.
/// `00 B0 [offset_hi] [offset_lo] [len]`
pub fn read_binary(offset: u16, len: u8) -> Vec<u8> {
    let mut cmd = vec![0x00, 0xB0];
    cmd.extend_from_slice(&offset.to_be_bytes());
    cmd.push(len);
    cmd
}

/// Parse status word from last 2 bytes of response. Returns (sw1, sw2).
pub fn parse_sw(response: &[u8]) -> Option<(u8, u8)> {
    if response.len() >= 2 {
        let sw2 = response[response.len() - 1];
        let sw1 = response[response.len() - 2];
        Some((sw1, sw2))
    } else {
        None
    }
}

/// Returns true if SW == 9000 (success).
pub fn is_success(response: &[u8]) -> bool {
    parse_sw(response) == Some((0x90, 0x00))
}

/// Extract payload (all bytes except SW).
pub fn payload(response: &[u8]) -> &[u8] {
    if response.len() >= 2 {
        &response[..response.len() - 2]
    } else {
        response
    }
}
