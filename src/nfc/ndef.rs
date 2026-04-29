use ndef_rs::NdefMessage;

/// Extracts human-readable text from NDEF data.
///
/// # Errors
///
/// Returns `None` if no Text record is found or data is invalid.
pub fn extract_text(data: &[u8]) -> Option<String> {
    tracing::debug!("NDEF raw bytes: {:02x?}", data);
    let message = match NdefMessage::decode(data) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::debug!("NDEF decode failed: {e}");
            return None;
        }
    };
    let records = message.records();
    tracing::debug!("NDEF message has {} records", records.len());
    records.into_iter().find_map(|record| {
        // Check if this is a Text record (TNF=WellKnown, Type="T")
        if record.tnf() != ndef_rs::TNF::WellKnown {
            return None;
        }
        if record.record_type() != b"T" {
            return None;
        }
        
        // Parse NDEF Text record payload manually
        // Format: [Status byte][Language code][Text]
        let payload = record.payload();
        if payload.is_empty() {
            return None;
        }
        
        let status = payload[0];
        let lang_len = (status & 0x3F) as usize; // Lower 6 bits = language code length
        
        if payload.len() < 1 + lang_len {
            tracing::warn!("Text record payload too short");
            return None;
        }
        
        // Skip status byte + language code to get actual text
        let text_bytes = &payload[1 + lang_len..];
        let text = String::from_utf8_lossy(text_bytes).to_string();
        
        tracing::debug!("found Text record (lang_len={lang_len}): {}", text);
        Some(text)
    })
}

/// Fallback for non-NDEF or raw data.
/// Trims trailing nulls and converts lossy UTF-8.
pub fn fallback_text(data: &[u8]) -> String {
    let trimmed = data.iter()
        .rposition(|&b| b != 0x00)
        .map_or(&[][..], |pos| &data[..=pos]);
    
    String::from_utf8_lossy(trimmed).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_trimming() {
        let data = b"Hello\0\0";
        assert_eq!(fallback_text(data), "Hello");
    }

    #[test]
    fn test_fallback_empty() {
        let data = b"\0\0";
        assert_eq!(fallback_text(data), "");
    }
}
