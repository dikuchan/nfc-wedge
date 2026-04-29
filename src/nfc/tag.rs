use pcsc::Card;
use anyhow::{Result, anyhow};

use super::apdu;

const MAX_TYPE2_PAGES: u8 = 231; // NTAG216 total pages
const READ_SIZE: u8 = 16; // 4 pages per read

/// Get Data APDU for retrieving card UID.
const GET_UID_APDU: [u8; 5] = [0xFF, 0xCA, 0x00, 0x00, 0x00];

/// Reads data from a Type 2 tag (NTAG216 and compatible).
///
/// # Errors
///
/// Returns error if tag is not Type 2 or read fails.
pub fn read_tag(card: &Card) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut page = 4; // CC starts at page 4

    while page < MAX_TYPE2_PAGES {
        let cmd = apdu::type2_read_binary(page, READ_SIZE);
        let mut recv = [0u8; 256];
        let rsp = match card.transmit(&cmd, &mut recv) {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("transmit failed at page {page}: {e}");
                break;
            }
        };

        if !apdu::is_success(rsp) {
            tracing::debug!("read stopped at page {page} (SW non-success)");
            break;
        }

        let payload = apdu::payload(rsp);
        data.extend_from_slice(payload);

        // Scan for terminator TLV (0xFE)
        if let Some(pos) = payload.iter().position(|&b| b == 0xFE) {
            let total_len = data.len() - (payload.len() - pos);
            data.truncate(total_len);
            tracing::debug!("found 0xFE terminator at offset {total_len}");
            break;
        }

        page += 4;
    }

    tracing::debug!("read {} raw bytes from Type 2 memory", data.len());

    // Find NDEF Message TLV (0x03)
    if let Some(pos) = data.iter().position(|&b| b == 0x03) {
        tracing::debug!("NDEF TLV header found at offset {pos}");
        let (_, ndef_data) = parse_tlv(&data[pos..])
            .map_err(|e| anyhow!("TLV parse failed: {e}"))?;
        
        tracing::info!("extracted {} bytes of NDEF content", ndef_data.len());
        return Ok(ndef_data.to_vec());
    }

    if !data.is_empty() {
        tracing::warn!("tag has data but no NDEF TLV found");
        return Ok(data);
    }

    Err(anyhow!("no data retrieved from Type 2 tag"))
}

/// Parses TLV starting at data. Returns (total_len, payload).
fn parse_tlv(data: &[u8]) -> Result<(usize, &[u8]), &'static str> {
    if data.len() < 2 {
        return Err("TLV too short");
    }
    if data[0] != 0x03 {
        return Err("not NDEF TLV");
    }

    let len_byte = data[1];
    let (len, header_len): (usize, usize) = if len_byte == 0xFF && data.len() >= 4 {
        (u16::from_be_bytes([data[2], data[3]]) as usize, 4)
    } else {
        (len_byte as usize, 2)
    };

    if data.len() < header_len + len {
        return Err("TLV length exceeds buffer");
    }

    Ok((header_len + len, &data[header_len..header_len + len]))
}

/// Retrieves the card UID using PC/SC Get Data command.
///
/// # Errors
///
/// Returns error if the reader does not support UID retrieval or transmission fails.
pub fn get_uid(card: &Card) -> Result<Vec<u8>> {
    let mut recv = [0u8; 32];
    let rsp = card.transmit(&GET_UID_APDU, &mut recv)
        .map_err(|e| anyhow!("failed to get UID: {e}"))?;

    if !apdu::is_success(rsp) {
        return Err(anyhow!("get UID command returned error"));
    }

    let uid = apdu::payload(rsp).to_vec();
    if uid.is_empty() {
        return Err(anyhow!("UID is empty"));
    }

    Ok(uid)
}
