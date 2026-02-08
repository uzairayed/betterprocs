use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::process::types::ProcessConfig;

#[derive(Debug, Deserialize)]
pub struct YamlConfig {
    pub procs: IndexMap<String, YamlProcEntry>,
    #[serde(default)]
    pub settings: YamlSettings,
}

impl YamlConfig {
    pub fn auto_exit(&self) -> bool {
        self.settings.auto_exit
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum YamlProcEntry {
    /// Simple string form: "npm run dev"
    Simple(String),
    /// Full config form
    Full(YamlProcConfig),
}

#[derive(Debug, Deserialize)]
pub struct YamlProcConfig {
    pub shell: Option<String>,
    pub cmd: Option<Vec<String>>,
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub autostart: bool,
    #[serde(default)]
    pub autorestart: bool,
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct YamlSettings {
    #[serde(default)]
    pub auto_exit: bool,
    pub mouse: Option<bool>,
    pub scrollback: Option<usize>,
}

fn default_true() -> bool {
    true
}

pub fn load_yaml(path: &Path) -> Result<YamlConfig> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: YamlConfig =
        serde_yaml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(config)
}

pub fn try_load_yaml(explicit_path: &Option<PathBuf>) -> Result<Option<YamlConfig>> {
    // If explicit path given, it must exist
    if let Some(path) = explicit_path {
        return Ok(Some(load_yaml(path)?));
    }

    // Try default paths
    for name in &["betterprocs.yaml", "betterprocs.yml", "mprocs.yaml"] {
        let path = Path::new(name);
        if path.exists() {
            return Ok(Some(load_yaml(path)?));
        }
    }

    Ok(None)
}

impl YamlConfig {
    pub fn into_process_configs(self) -> Vec<ProcessConfig> {
        self.procs
            .into_iter()
            .map(|(name, entry)| match entry {
                YamlProcEntry::Simple(cmd) => ProcessConfig {
                    name,
                    command: cmd,
                    autostart: true,
                    ..Default::default()
                },
                YamlProcEntry::Full(cfg) => ProcessConfig {
                    name,
                    command: cfg.shell.unwrap_or_default(),
                    cmd: cfg.cmd,
                    cwd: cfg.cwd,
                    env: cfg.env,
                    autostart: cfg.autostart,
                    autorestart: cfg.autorestart,
                    port: cfg.port,
                },
            })
            .collect()
    }
}
