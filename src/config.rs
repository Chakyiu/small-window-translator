use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_IPC_PORT: u16 = 18765;
pub const DEFAULT_HOTKEY: &str = "Alt+D";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hotkey: String,
    pub source_lang: String,
    pub target_lang: String,
    pub ipc_port: u16,
    pub start_at_login: bool,
    pub deepl: DeepLConfig,
    pub openai: OpenAiConfig,
    pub libre: LibreConfig,
    pub google: GoogleConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            source_lang: "auto".to_string(),
            target_lang: "zh".to_string(),
            ipc_port: DEFAULT_IPC_PORT,
            start_at_login: false,
            deepl: DeepLConfig::default(),
            openai: OpenAiConfig::default(),
            libre: LibreConfig::default(),
            google: GoogleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeepLConfig {
    pub enabled: bool,
    pub api_key: String,
    pub use_pro: bool,
}

impl Default for DeepLConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: String::new(),
            use_pro: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAiConfig {
    pub enabled: bool,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub whisper_model: String,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            whisper_model: "whisper-1".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibreConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub api_key: String,
}

impl Default for LibreConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: String::new(),
            api_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleConfig {
    /// Unofficial no-key endpoint. Default off: ToS and breakage risk.
    pub enabled: bool,
}

impl Default for GoogleConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "swtrans", "swtrans")
            .context("could not resolve config directory")?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    fn legacy_config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("dev", "sw-dict", "sw-dict")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    pub fn load() -> Self {
        if let Ok(path) = Self::config_path() {
            if path.exists() {
                return Self::load_from(&path).unwrap_or_default();
            }
        }
        if let Some(legacy) = Self::legacy_config_path() {
            if let Ok(cfg) = Self::load_from(&legacy) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn has_saved_file() -> bool {
        Self::config_path()
            .map(|p| p.exists())
            .unwrap_or(false)
            || Self::legacy_config_path().is_some_and(|p| p.exists())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&raw)?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = Self::config_path()?;
        self.save_to(&path)?;
        Ok(path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        fs::write(path, raw)?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn round_trip_toml() {
        let dir = env::temp_dir().join(format!(
            "swtrans-cfg-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let mut cfg = Config::default();
        cfg.target_lang = "ja".into();
        cfg.deepl.api_key = "secret".into();
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.target_lang, "ja");
        assert_eq!(loaded.deepl.api_key, "secret");
        assert!(!loaded.google.enabled);
        let _ = fs::remove_dir_all(dir);
    }
}
