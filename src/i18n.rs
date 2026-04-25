use std::collections::HashMap;

/// Simple JSON-based i18n. Russian is compiled in as the default.
pub struct I18n {
    map: HashMap<String, String>,
}

impl I18n {
    pub fn new(lang: &str) -> anyhow::Result<Self> {
        let json_str = match lang {
            "ru" => include_str!("../i18n/ru.json"),
            _ => include_str!("../i18n/ru.json"),
        };
        let map: HashMap<String, String> = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("invalid i18n json: {e}"))?;
        Ok(Self { map })
    }

    /// Translate a key. Falls back to the raw key if missing.
    pub fn t(&self, key: &str) -> String {
        self.map.get(key).cloned().unwrap_or_else(|| key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_known_key() {
        let i18n = I18n::new("ru").unwrap();
        assert_eq!(i18n.t("waiting_card"), "Ожидание карты...");
    }

    #[test]
    fn translate_fallback() {
        let i18n = I18n::new("ru").unwrap();
        assert_eq!(i18n.t("unknown_key"), "unknown_key");
    }
}
