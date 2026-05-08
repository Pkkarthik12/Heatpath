use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::ui::{SortMode, ViewMode};

#[derive(Debug, Clone, Deserialize)]
pub struct HeatpathConfig {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub decay: DecayConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub ignore: IgnoreConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefaultsConfig {
    pub depth: Option<usize>,
    #[serde(default)]
    pub sort: SortMode,
    #[serde(default)]
    pub mode: ViewMode,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecayConfig {
    pub enabled: bool,
    pub window_days: i64,
    pub rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitConfig {
    pub enabled: bool,
    pub commit_boost: f64,
    pub lookback_days: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub patterns: Vec<String>,
}

impl HeatpathConfig {
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
    }
}

impl Default for HeatpathConfig {
    fn default() -> Self {
        Self {
            defaults: DefaultsConfig::default(),
            decay: DecayConfig::default(),
            git: GitConfig::default(),
            ignore: IgnoreConfig::default(),
        }
    }
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            depth: Some(4),
            sort: SortMode::Touches,
            mode: ViewMode::Session,
        }
    }
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_days: 30,
            rate: 0.10,
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            commit_boost: 0.20,
            lookback_days: 14,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join("heatpath").join("config.toml"))
}
