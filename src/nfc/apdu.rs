/// APDU builders and response helpers for NFC tag communication.

/// PC/SC escape command to read memory on Type 2 tags (MIFARE Ultralight / NTAG).
/// `FF B0 00 [page] [len]`
pub fn type2_read_binary(page: u8, len: u8) -> Vec<u8> {
    vec![0xFF, 0xB0, 0x00, page, len]
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
