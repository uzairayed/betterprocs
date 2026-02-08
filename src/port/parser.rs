use regex::Regex;

use crate::process::types::ProcessConfig;

/// Extract port numbers from a process config.
/// Uses the explicit `port` field first, then heuristic regex on the command string.
pub fn extract_ports(config: &ProcessConfig) -> Vec<u16> {
    let mut ports = Vec::new();

    // Explicit port from config
    if let Some(port) = config.port {
        ports.push(port);
    }

    // Heuristic extraction from command string
    let cmd = &config.command;
    if !cmd.is_empty() {
        // Match patterns like: --port 3000, -p 8080, PORT=3000
        let patterns = [
            r"(?:--port|--PORT|-p)\s+(\d{2,5})",
            r"PORT=(\d{2,5})",
            r"localhost:(\d{2,5})",
        ];

        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                for cap in re.captures_iter(cmd) {
                    if let Some(m) = cap.get(1) {
                        if let Ok(port) = m.as_str().parse::<u16>() {
                            if port >= 80 {
                                ports.push(port);
                            }
                        }
                    }
                }
            }
        }
    }

    ports.sort();
    ports.dedup();
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_port() {
        let config = ProcessConfig {
            port: Some(3000),
            ..Default::default()
        };
        assert_eq!(extract_ports(&config), vec![3000]);
    }

    #[test]
    fn test_port_from_command() {
        let config = ProcessConfig {
            command: "node server.js --port 8080".to_string(),
            ..Default::default()
        };
        assert_eq!(extract_ports(&config), vec![8080]);
    }

    #[test]
    fn test_port_env_var() {
        let config = ProcessConfig {
            command: "PORT=3000 node app.js".to_string(),
            ..Default::default()
        };
        assert_eq!(extract_ports(&config), vec![3000]);
    }
}
