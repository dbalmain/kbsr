use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Timeout in seconds before auto-marking incorrect (default: 5)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Maximum attempts before showing answer (default: 3)
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u8,

    /// Path to decks directory
    #[serde(default = "default_decks_dir")]
    pub decks_dir: PathBuf,

    /// Path to database file
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
}

fn default_timeout() -> u64 {
    5
}

fn default_max_attempts() -> u8 {
    3
}

fn default_decks_dir() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("kbsr").join("decks"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .map(|p| p.join("kbsr").join("kbsr.db"))
        .unwrap_or_else(|| PathBuf::from("kbsr.db"))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout(),
            max_attempts: default_max_attempts(),
            decks_dir: default_decks_dir(),
            db_path: default_db_path(),
        }
    }
}

impl Config {
    /// Load config from file or return defaults
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Path to config file
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("kbsr").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("config.toml"))
    }

    /// Ensure required directories exist
    pub fn ensure_dirs(&self) -> Result<()> {
        if let Some(parent) = self.decks_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&self.decks_dir)?;

        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(())
    }
}
