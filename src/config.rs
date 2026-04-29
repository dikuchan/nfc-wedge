use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Application configuration persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_reader: Option<String>,
    pub append_enter: bool,
    pub language: String,
    #[serde(default = "default_cooldown_ms")]
    pub cooldown_ms: u64,
}

fn default_cooldown_ms() -> u64 {
    2000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_reader: None,
            append_enter: true,
            language: "ru".to_string(),
            cooldown_ms: default_cooldown_ms(),
        }
    }
}

impl Config {
    /// Load config from the application config directory, or return defaults if missing.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            tracing::info!("config not found at {:?}, using defaults", path);
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config from {:?}", path))?;
        let config: Config = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse config from {:?}", path))?;
        Ok(config)
    }

    /// Save config atomically by writing to a temp file and renaming.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir {:?}", parent))?;
        }
        let tmp = path.with_extension("tmp");
        let contents = serde_json::to_string_pretty(self)
            .context("failed to serialize config")?;
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("failed to create temp config {:?}", tmp))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write temp config {:?}", tmp))?;
        drop(file);
        fs::rename(&tmp, &path)
            .with_context(|| format!("failed to rename config {:?} -> {:?}", tmp, path))?;
        tracing::info!("config saved to {:?}", path);
        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "nfc-wedge")
            .context("failed to determine project directories")?;
        Ok(dirs.config_dir().join("config.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let mut cfg = Config::default();
        cfg.default_reader = Some("ACS ACR1552U".to_string());
        cfg.append_enter = false;

        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.default_reader, Some("ACS ACR1552U".to_string()));
        assert!(!parsed.append_enter);
        assert_eq!(parsed.language, "ru");
    }
}
