use std::process::Command;
use std::time::Instant;

pub struct PortEntry {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    pub protocol: String,
}

pub struct PortKiller {
    pub port_input: String,
    pub selected: usize,
    entries: Vec<PortEntry>,
    last_refresh: Instant,
}

impl PortKiller {
    pub fn new() -> Self {
        let mut pk = Self {
            port_input: String::new(),
            selected: 0,
            entries: Vec::new(),
            last_refresh: Instant::now(),
        };
        pk.refresh();
        pk
    }

    pub fn refresh(&mut self) {
        self.entries = scan_listening_ports(&self.port_input);
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
        self.last_refresh = Instant::now();
    }

    pub fn maybe_refresh(&mut self) {
        if self.last_refresh.elapsed().as_secs() >= 3 {
            self.refresh();
        }
    }

    pub fn type_char(&mut self, c: char) {
        if c.is_ascii_digit() || c == ',' || c == ' ' {
            self.port_input.push(c);
            self.selected = 0;
            self.refresh();
        }
    }

    pub fn backspace(&mut self) {
        self.port_input.pop();
        self.selected = 0;
        self.refresh();
    }

    pub fn clear_input(&mut self) {
        self.port_input.clear();
        self.selected = 0;
        self.refresh();
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            if self.selected == 0 {
                self.selected = self.entries.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn selected_pid(&self) -> Option<u32> {
        self.entries.get(self.selected).map(|e| e.pid)
    }

    pub fn entries(&self) -> &[PortEntry] {
        &self.entries
    }
}

/// Scan for processes listening on ports using lsof.
/// If `filter` is non-empty, only show ports matching the filter (comma-separated).
fn scan_listening_ports(filter: &str) -> Vec<PortEntry> {
    // Parse filter into specific port numbers
    let filter_ports: Vec<u16> = filter
        .split(|c: char| c == ',' || c == ' ')
        .filter_map(|s| s.trim().parse::<u16>().ok())
        .collect();

    let output = if filter_ports.is_empty() {
        // Show all listening ports
        Command::new("lsof")
            .args(["-iTCP", "-sTCP:LISTEN", "-nP", "-F", "pcn"])
            .output()
            .ok()
    } else {
        // Show specific ports only
        let port_args: Vec<String> = filter_ports
            .iter()
            .map(|p| format!("-iTCP:{}", p))
            .collect();
        let mut cmd = Command::new("lsof");
        for arg in &port_args {
            cmd.arg(arg);
        }
        cmd.args(["-sTCP:LISTEN", "-nP", "-F", "pcn"]).output().ok()
    };

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_lsof_output(&stdout)
}

/// Parse lsof -F pcn output format.
/// Lines starting with 'p' = PID, 'c' = command name, 'n' = name (contains port).
fn parse_lsof_output(output: &str) -> Vec<PortEntry> {
    let mut entries = Vec::new();
    let mut current_pid: Option<u32> = None;
    let mut current_name: Option<String> = None;

    for line in output.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.parse().ok();
        } else if let Some(cmd) = line.strip_prefix('c') {
            current_name = Some(cmd.to_string());
        } else if let Some(addr) = line.strip_prefix('n') {
            if let (Some(pid), Some(ref name)) = (current_pid, &current_name) {
                // addr looks like "*:3000" or "127.0.0.1:8080" or "[::1]:5173"
                if let Some(port) = extract_port_from_addr(addr) {
                    // Avoid duplicates (same pid+port)
                    if !entries
                        .iter()
                        .any(|e: &PortEntry| e.pid == pid && e.port == port)
                    {
                        entries.push(PortEntry {
                            port,
                            pid,
                            process_name: name.clone(),
                            protocol: "TCP".to_string(),
                        });
                    }
                }
            }
        }
    }

    entries.sort_by_key(|e| e.port);
    entries
}

fn extract_port_from_addr(addr: &str) -> Option<u16> {
    // Handle formats: "*:3000", "127.0.0.1:8080", "[::1]:5173", "localhost:3001"
    let port_str = addr.rsplit(':').next()?;
    port_str.parse().ok()
}
