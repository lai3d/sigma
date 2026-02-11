use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    pub api_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub api_url: String,
    pub api_key: Option<String>,
}

impl Config {
    /// Load config: file → env vars → CLI flags (later overrides earlier)
    pub fn load(cli_api_url: Option<&str>, cli_api_key: Option<&str>) -> Result<Self> {
        let file_config = load_config_file().unwrap_or_default();

        let api_url = cli_api_url
            .map(String::from)
            .or_else(|| std::env::var("SIGMA_API_URL").ok())
            .or(file_config.api_url)
            .unwrap_or_else(|| "http://localhost:3000/api".to_string());

        let api_key = cli_api_key
            .map(String::from)
            .or_else(|| std::env::var("SIGMA_API_KEY").ok())
            .or(file_config.api_key);

        Ok(Config { api_url, api_key })
    }
}

fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Could not determine config directory")?;
    Ok(config_dir.join("sigma").join("config.toml"))
}

fn load_config_file() -> Result<ConfigFile> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config: ConfigFile =
        toml::from_str(&contents).with_context(|| "Failed to parse config file")?;
    Ok(config)
}

pub fn set_config_value(key: &str, value: &str) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    let mut config = load_config_file().unwrap_or_default();
    match key {
        "api_url" => config.api_url = Some(value.to_string()),
        "api_key" => config.api_key = Some(value.to_string()),
        _ => anyhow::bail!("Unknown config key: {}", key),
    }

    let toml_str = toml::to_string_pretty(&config)?;
    std::fs::write(&path, toml_str)
        .with_context(|| format!("Failed to write config file: {}", path.display()))?;

    println!("Saved {} to {}", key, path.display());
    Ok(())
}
