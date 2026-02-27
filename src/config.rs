use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(alias = "device_a")]
    pub speakers: String,
    #[serde(alias = "device_b")]
    pub headphones: String,
    pub hotkey: String,
}

/// Path to the config file: %APPDATA%\AudioSwitcher\config.json
pub fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().expect("Could not find AppData directory");
    path.push("AudioSwitcher");
    path.push("config.json");
    path
}

/// Load config from disk. Returns None if file doesn't exist or is invalid.
pub fn load() -> Option<Config> {
    let path = config_path();
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Save config to disk, creating the directory if needed.
pub fn save(config: &Config) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create config directory");
    }
    let data = serde_json::to_string_pretty(config).expect("Failed to serialize config");
    fs::write(&path, data).expect("Failed to write config file");
}
