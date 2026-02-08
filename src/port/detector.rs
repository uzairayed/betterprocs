use std::io::{self, Write};
use std::process::Command;

use anyhow::Result;

use super::parser::extract_ports;
use crate::process::types::ProcessConfig;

#[derive(Debug)]
pub struct PortConflict {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    pub our_process: String,
}

/// Detect port conflicts for all configured processes.
pub fn detect_conflicts(configs: &[ProcessConfig]) -> Vec<PortConflict> {
    let mut conflicts = Vec::new();

    for config in configs {
        let ports = extract_ports(config);
        for port in ports {
            if let Some((pid, name)) = find_process_on_port(port) {
                conflicts.push(PortConflict {
                    port,
                    pid,
                    process_name: name,
                    our_process: config.name.clone(),
                });
            }
        }
    }

    conflicts
}

/// Find which process is listening on a given port using lsof.
fn find_process_on_port(port: u16) -> Option<(u32, String)> {
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = stdout.trim().lines().next()?;
    let pid: u32 = pid_str.parse().ok()?;

    // Get process name
    let name_output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;

    let name = String::from_utf8_lossy(&name_output.stdout)
        .trim()
        .to_string();

    Some((pid, if name.is_empty() { format!("PID {}", pid) } else { name }))
}

/// Show port conflicts to the user and ask what to do.
/// Returns true if the user wants to continue, false to quit.
pub fn handle_conflicts(conflicts: &[PortConflict]) -> Result<bool> {
    if conflicts.is_empty() {
        return Ok(true);
    }

    eprintln!("\nPort conflicts detected:");
    for c in conflicts {
        eprintln!(
            "  Port {}: used by {} (PID {}) — needed by \"{}\"",
            c.port, c.process_name, c.pid, c.our_process
        );
    }
    eprintln!();
    eprint!("[K]ill conflicting processes  [I]gnore  [Q]uit: ");
    io::stderr().flush()?;

    // Read single character response
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "k" | "kill" => {
            for c in conflicts {
                eprint!("  Killing {} (PID {})... ", c.process_name, c.pid);
                let result = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(c.pid as i32),
                    nix::sys::signal::Signal::SIGTERM,
                );
                if result.is_ok() {
                    // Wait briefly for it to die
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    eprintln!("done");
                } else {
                    eprintln!("failed");
                }
            }
            Ok(true)
        }
        "i" | "ignore" | "" => Ok(true),
        "q" | "quit" => Ok(false),
        _ => Ok(true),
    }
}
