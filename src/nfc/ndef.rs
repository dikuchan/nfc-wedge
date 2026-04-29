use ndef_rs::NdefMessage;
use ndef_rs::payload::TextPayload;

/// Extracts human-readable text from NDEF data.
///
/// # Errors
///
/// Returns `None` if no Text record is found or data is invalid.
pub fn extract_text(data: &[u8]) -> Option<String> {
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
        match TextPayload::try_from(record) {
            Ok(p) => {
                let txt = p.text().to_string();
                tracing::debug!("found Text record: {}", txt);
                Some(txt)
            }
            Err(e) => {
                tracing::debug!("record is not Text: {e}");
                None
            }
        }
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
