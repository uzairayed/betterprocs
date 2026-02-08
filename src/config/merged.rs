use anyhow::{bail, Result};

use super::cli::Cli;
use super::npm::detect_npm_scripts;
use super::yaml::try_load_yaml;
use crate::process::types::ProcessConfig;

pub struct AppConfig {
    pub processes: Vec<ProcessConfig>,
    pub auto_exit: bool,
}

pub fn load_config(cli: &Cli) -> Result<AppConfig> {
    let mut processes = Vec::new();

    // Source 1: CLI positional commands
    for (i, cmd) in cli.commands.iter().enumerate() {
        let name = if i < cli.names.len() {
            cli.names[i].clone()
        } else {
            // Derive name from command
            cmd.split_whitespace()
                .next()
                .unwrap_or("proc")
                .to_string()
        };

        processes.push(ProcessConfig {
            name,
            command: cmd.clone(),
            autostart: true,
            ..Default::default()
        });
    }

    // Source 2: YAML file (if no CLI commands provided)
    let mut auto_exit_from_yaml = false;
    if processes.is_empty() {
        if let Some(yaml_config) = try_load_yaml(&cli.config)? {
            auto_exit_from_yaml = yaml_config.auto_exit();
            processes.extend(yaml_config.into_process_configs());
        }
    }

    // Source 3: package.json (if --npm flag)
    if cli.npm {
        let dir = cli
            .cwd
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        match detect_npm_scripts(&dir) {
            Ok(npm_procs) => processes.extend(npm_procs),
            Err(e) => eprintln!("Warning: Could not load npm scripts: {}", e),
        }
    }

    if processes.is_empty() {
        bail!(
            "No processes configured.\n\
             Usage:\n  \
             betterprocs \"cmd1\" \"cmd2\"      Run commands directly\n  \
             betterprocs                     Load from betterprocs.yaml\n  \
             betterprocs --npm               Load scripts from package.json"
        );
    }

    Ok(AppConfig {
        processes,
        auto_exit: cli.auto_exit || auto_exit_from_yaml,
    })
}
