use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Timeout in seconds before auto-marking incorrect (default: 10)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Maximum attempts before showing answer (default: 3)
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u8,

    /// Response time threshold in ms for Easy rating (default: 2000)
    #[serde(default = "default_easy_threshold")]
    pub easy_threshold_ms: u64,

    /// Response time threshold in ms for Hard rating (default: 5000)
    #[serde(default = "default_hard_threshold")]
    pub hard_threshold_ms: u64,

    /// Delay in milliseconds to show success indicator (default: 500)
    #[serde(default = "default_success_delay")]
    pub success_delay_ms: u64,

    /// Delay in milliseconds to show failed flash before retry (default: 500)
    #[serde(default = "default_failed_flash_delay")]
    pub failed_flash_delay_ms: u64,

    /// Keybind to pause the app (default: "Super+Ctrl+P")
    #[serde(default = "default_pause_keybind")]
    pub pause_keybind: String,

    /// Keybind to quit the app (default: "Super+Ctrl+Q")
    #[serde(default = "default_quit_keybind")]
    pub quit_keybind: String,

    /// Shuffle cards before each study session (default: true)
    #[serde(default = "default_shuffle_cards")]
    pub shuffle_cards: bool,

    /// FSRS desired retention rate 0.0-1.0 (default: 0.9)
    #[serde(default = "default_desired_retention")]
    pub desired_retention: f32,

    /// Interval multiplier to compress/expand FSRS intervals (default: 0.12)
    /// Lower values = more frequent reviews. 0.12 makes "Easy" on a new card ≈ 1 day.
    #[serde(default = "default_interval_modifier")]
    pub interval_modifier: f32,

    /// Maximum interval in days between reviews (default: 30)
    #[serde(default = "default_max_interval_days")]
    pub max_interval_days: f32,

    /// Path to decks directory
    #[serde(default = "default_decks_dir")]
    pub decks_dir: PathBuf,

    /// Path to database file
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
}

fn default_timeout() -> u64 {
    10
}

fn default_max_attempts() -> u8 {
    3
}

fn default_easy_threshold() -> u64 {
    3000
}

fn default_hard_threshold() -> u64 {
    6000
}

fn default_success_delay() -> u64 {
    500
}

fn default_failed_flash_delay() -> u64 {
    500
}

fn default_pause_keybind() -> String {
    "Super+Ctrl+P".to_string()
}

fn default_quit_keybind() -> String {
    "Super+Ctrl+Q".to_string()
}

fn default_shuffle_cards() -> bool {
    true
}

fn default_desired_retention() -> f32 {
    0.9
}

fn default_interval_modifier() -> f32 {
    0.12
}

fn default_max_interval_days() -> f32 {
    30.0
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
            easy_threshold_ms: default_easy_threshold(),
            hard_threshold_ms: default_hard_threshold(),
            success_delay_ms: default_success_delay(),
            failed_flash_delay_ms: default_failed_flash_delay(),
            pause_keybind: default_pause_keybind(),
            quit_keybind: default_quit_keybind(),
            shuffle_cards: default_shuffle_cards(),
            desired_retention: default_desired_retention(),
            interval_modifier: default_interval_modifier(),
            max_interval_days: default_max_interval_days(),
            decks_dir: default_decks_dir(),
            db_path: default_db_path(),
        }
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(suffix) = path.strip_prefix("~")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(suffix);
    }
    path.to_path_buf()
}

impl Config {
    /// Load config from file or return defaults
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut config: Config = toml::from_str(&content)?;
            config.decks_dir = expand_tilde(&config.decks_dir);
            config.db_path = expand_tilde(&config.db_path);
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
