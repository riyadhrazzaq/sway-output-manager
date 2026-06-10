//! Persistent user preferences for Sway output arrangements.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::sway::Arrangement;

/// App configuration stored on disk.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub preferences: HashMap<String, OutputPreference>,
}

/// Remembered preference for a single output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPreference {
    pub action: Arrangement,
    #[serde(default)]
    pub anchor_output: Option<String>,
}

impl AppConfig {
    /// Load configuration from the default config path.
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file at {}", path.display()))?;
        let config = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse config file at {}", path.display()))?;
        Ok(config)
    }

    /// Persist configuration to the default config path.
    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory at {}", parent.display())
            })?;
        }

        let contents = serde_json::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, contents)
            .with_context(|| format!("failed to write config file at {}", path.display()))?;
        Ok(())
    }

    /// Look up the saved preference for an output.
    pub fn preference_for(&self, output_name: &str) -> Option<&OutputPreference> {
        self.preferences.get(output_name)
    }

    /// Store a new preference for an output.
    pub fn set_preference(
        &mut self,
        output_name: String,
        action: Arrangement,
        anchor_output: Option<String>,
    ) {
        self.preferences.insert(
            output_name,
            OutputPreference {
                action,
                anchor_output,
            },
        );
    }
}

fn config_path() -> Result<PathBuf> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| anyhow!("unable to locate a config directory"))?;
    Ok(config_dir.join("swaywm-output-manager").join("config.json"))
}
