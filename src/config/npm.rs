use anyhow::{Context, Result};
use std::path::Path;

use crate::process::types::ProcessConfig;

pub fn detect_npm_scripts(dir: &Path) -> Result<Vec<ProcessConfig>> {
    let pkg_path = dir.join("package.json");
    let content = std::fs::read_to_string(&pkg_path)
        .with_context(|| format!("Failed to read {}", pkg_path.display()))?;

    let pkg: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse package.json")?;

    let scripts = pkg
        .get("scripts")
        .and_then(|s| s.as_object())
        .context("No scripts found in package.json")?;

    let configs: Vec<ProcessConfig> = scripts
        .iter()
        .filter_map(|(name, _cmd)| {
            Some(ProcessConfig {
                name: name.clone(),
                command: format!("npm run {}", name),
                autostart: false, // Let user choose which to start
                ..Default::default()
            })
        })
        .collect();

    Ok(configs)
}
