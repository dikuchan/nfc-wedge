use pcsc::Card;

use super::apdu;

const MAX_TYPE2_PAGES: u8 = 64; // Safety limit for NTAG215/216
const READ_SIZE: u8 = 16; // 4 pages per read

/// Attempt to read data from a connected card.
/// Tries Type 2 first (escape commands), then Type 4 (SELECT + READ BINARY).
/// Returns raw bytes or error.
pub fn read_tag(card: &Card) -> anyhow::Result<Vec<u8>> {
    match try_type2(card) {
        Ok(data) if !data.is_empty() => {
            tracing::info!("read {} bytes via Type 2", data.len());
            return Ok(data);
        }
        Ok(_) => tracing::debug!("Type 2 returned empty, trying Type 4"),
        Err(e) => tracing::debug!("Type 2 failed: {e}, trying Type 4"),
    }

    match try_type4(card) {
        Ok(data) if !data.is_empty() => {
            tracing::info!("read {} bytes via Type 4", data.len());
            Ok(data)
        }
        Ok(_) => Err(anyhow::anyhow!("no readable data on card")),
        Err(e) => Err(e),
    }
}

fn try_type2(card: &Card) -> anyhow::Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut page = 4; // CC starts at page 4 on NTAG/Ultralight

    while page < MAX_TYPE2_PAGES {
        let cmd = apdu::type2_read_binary(page, READ_SIZE);
        let rsp = transmit(card, &cmd)
            .map_err(|e| anyhow::anyhow!("Type 2 read failed: {e}"))?;

        if !apdu::is_success(&rsp) {
            // Not a Type 2 tag or read failed
            return Err(anyhow::anyhow!("not a Type 2 tag"));
        }

        let payload = apdu::payload(&rsp);
        data.extend_from_slice(payload);

        // Scan for terminator in this chunk
        if let Some(pos) = payload.iter().position(|&b| b == 0xFE) {
            data.truncate(data.len() - (payload.len() - pos));
            break;
        }

        page += 4; // advance by 4 pages (16 bytes)
    }

    // Find NDEF TLV: 0x03 followed by length
    if let Some(pos) = find_ndef_tlv(&data) {
        let (_, ndef_data) = parse_tlv(&data[pos..])
            .map_err(|e| anyhow::anyhow!("TLV parse failed: {e}"))?;
        return Ok(ndef_data.to_vec());
    }

    // If no NDEF TLV found but we have data, return raw bytes
    if !data.is_empty() {
        return Ok(data);
    }

    Err(anyhow::anyhow!("no data from Type 2"))
}

fn try_type4(card: &Card) -> anyhow::Result<Vec<u8>> {
    // SELECT NDEF application
    let cmd = apdu::select_ndef_application();
    let rsp = transmit(card, &cmd)
        .map_err(|e| anyhow::anyhow!("SELECT NDEF app failed: {e}"))?;
    if !apdu::is_success(&rsp) {
        return Err(anyhow::anyhow!("NDEF app not selected"));
    }

    // SELECT NDEF file
    let cmd = apdu::select_ndef_file();
    let rsp = transmit(card, &cmd)
        .map_err(|e| anyhow::anyhow!("SELECT file failed: {e}"))?;
    if !apdu::is_success(&rsp) {
        return Err(anyhow::anyhow!("NDEF file not selected"));
    }

    // Read first 2 bytes to get NDEF message length
    let cmd = apdu::read_binary(0, 2);
    let rsp = transmit(card, &cmd)
        .map_err(|e| anyhow::anyhow!("read length failed: {e}"))?;
    let payload = apdu::payload(&rsp);
    if payload.len() < 2 {
        return Err(anyhow::anyhow!("short length response"));
    }

    let ndef_len = u16::from_be_bytes([payload[0], payload[1]]) as usize;

    // Read full NDEF message (skip 2-byte length prefix)
    let mut data = Vec::with_capacity(ndef_len);
    let mut offset = 2u16;
    while data.len() < ndef_len {
        let chunk = (ndef_len - data.len()).min(256) as u8;
        let cmd = apdu::read_binary(offset, chunk);
        let rsp = transmit(card, &cmd)
            .map_err(|e| anyhow::anyhow!("read chunk failed: {e}"))?;
        let payload = apdu::payload(&rsp);
        data.extend_from_slice(payload);
        offset += chunk as u16;
    }

    data.truncate(ndef_len);
    Ok(data)
}

fn transmit(card: &Card, cmd: &[u8]) -> Result<Vec<u8>, pcsc::Error> {
    let mut recv = [0u8; 512];
    let rsp = card.transmit(cmd, &mut recv)?;
    Ok(rsp.to_vec())
}

/// Find NDEF Message TLV (0x03) in data. Returns index of 0x03.
fn find_ndef_tlv(data: &[u8]) -> Option<usize> {
    data.iter().position(|&b| b == 0x03)
}

/// Parse TLV starting at `offset`. Returns (total_tlv_len, payload_bytes).
fn parse_tlv(data: &[u8]) -> Result<(usize, &[u8]), &'static str> {
    if data.len() < 2 {
        return Err("too short for TLV");
    }
    let tag = data[0];
    if tag != 0x03 {
        return Err("not NDEF TLV");
    }

    let len_byte = data[1];
    let (len, header_len): (usize, usize);
    if len_byte == 0xFF && data.len() >= 4 {
        len = u16::from_be_bytes([data[2], data[3]]) as usize;
        header_len = 4;
    } else {
        len = len_byte as usize;
        header_len = 2;
    }

    if data.len() < header_len + len {
        return Err("TLV length exceeds data");
    }

    Ok((header_len + len, &data[header_len..header_len + len]))
}
