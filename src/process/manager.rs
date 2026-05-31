use anyhow::Result;

use super::handle::ProcessHandle;
use super::types::{ProcessConfig, ProcessStatus};

pub struct ProcessManager {
    pub processes: Vec<ProcessHandle>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    pub fn add_process(&mut self, config: ProcessConfig, rows: u16, cols: u16) {
        let handle = ProcessHandle::new(config, rows, cols);
        self.processes.push(handle);
    }

    pub fn start(&mut self, index: usize) -> Result<()> {
        if let Some(handle) = self.processes.get_mut(index) {
            handle.spawn()?;
        }
        Ok(())
    }

    pub fn stop(&mut self, index: usize) -> Result<()> {
        if let Some(handle) = self.processes.get_mut(index) {
            handle.stop(true)?;
        }
        Ok(())
    }

    pub fn force_kill(&mut self, index: usize) -> Result<()> {
        if let Some(handle) = self.processes.get_mut(index) {
            handle.stop(false)?;
        }
        Ok(())
    }

    pub fn restart(&mut self, index: usize) -> Result<()> {
        if let Some(handle) = self.processes.get_mut(index) {
            handle.restart()?;
        }
        Ok(())
    }

    /// Drain output from all processes. Returns true if any had new output.
    pub fn drain_output(&mut self) -> bool {
        let mut any_output = false;
        for handle in &mut self.processes {
            if handle.drain_output() {
                any_output = true;
            }
        }
        any_output
    }

    /// Check for autorestart
    pub fn check_autorestart(&mut self) {
        for handle in &mut self.processes {
            if handle.config.autorestart && !handle.status.is_running() {
                if !matches!(handle.status, ProcessStatus::NotStarted) {
                    let _ = handle.spawn();
                }
            }
        }
    }

    /// Stop all running processes
    pub fn stop_all(&mut self) {
        for handle in &mut self.processes {
            if handle.status.is_running() {
                let _ = handle.stop(true);
            }
        }
    }

    pub fn all_stopped(&self) -> bool {
        self.processes
            .iter()
            .all(|h| !h.status.is_running())
    }

    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// Find a process index by its (unique) name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.processes.iter().position(|h| h.config.name == name)
    }

    /// Spawn a brand-new process from a shell command at runtime, assigning a
    /// unique name. Returns the assigned name. Used by the MCP `run_command`
    /// tool so an agent can launch arbitrary commands.
    pub fn run_command(
        &mut self,
        command: String,
        name: Option<String>,
        cwd: Option<std::path::PathBuf>,
        env: std::collections::HashMap<String, String>,
        rows: u16,
        cols: u16,
    ) -> Result<String> {
        let base = name.unwrap_or_else(|| {
            command
                .split_whitespace()
                .next()
                .unwrap_or("proc")
                .to_string()
        });
        let unique = self.unique_name(&base);

        let config = ProcessConfig {
            name: unique.clone(),
            command,
            autostart: false,
            cwd,
            env,
            ..Default::default()
        };
        self.add_process(config, rows, cols);
        let idx = self.processes.len() - 1;
        self.start(idx)?;
        Ok(unique)
    }

    /// Derive a name not already in use, appending `-2`, `-3`, … on collision.
    fn unique_name(&self, base: &str) -> String {
        if self.index_of(base).is_none() {
            return base.to_string();
        }
        let mut n = 2;
        loop {
            let candidate = format!("{}-{}", base, n);
            if self.index_of(&candidate).is_none() {
                return candidate;
            }
            n += 1;
        }
    }

    /// Sort processes: running first, then crashed, stopped, not started.
    /// Returns the new index of the process that was at `selected` before sorting.
    pub fn sort_by_status(&mut self, selected: usize) -> usize {
        if self.processes.is_empty() {
            return 0;
        }
        // Track which process was selected by its name
        let selected_name = self
            .processes
            .get(selected)
            .map(|h| h.config.name.clone());

        self.processes
            .sort_by_key(|h| h.status.sort_order());

        // Find where the previously selected process ended up
        selected_name
            .and_then(|name| self.processes.iter().position(|h| h.config.name == name))
            .unwrap_or(0)
    }

    /// Resize all process PTYs
    pub fn resize_all(&mut self, rows: u16, cols: u16) {
        for handle in &mut self.processes {
            handle.resize_pty(rows, cols);
        }
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    /// Drain output repeatedly (mimicking the MCP pump) until `pred` holds on
    /// the named process's tail text, or the timeout elapses.
    fn drain_until(pm: &mut ProcessManager, name: &str, pred: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            pm.drain_output();
            let idx = pm.index_of(name).expect("process exists");
            let text = pm.processes[idx].screen.tail_text(200);
            if pred(&text) || Instant::now() >= deadline {
                return text;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn index_of_finds_and_misses() {
        let mut pm = ProcessManager::new();
        pm.add_process(
            ProcessConfig { name: "alpha".into(), autostart: false, ..Default::default() },
            10,
            40,
        );
        assert_eq!(pm.index_of("alpha"), Some(0));
        assert_eq!(pm.index_of("missing"), None);
    }

    #[test]
    fn run_command_assigns_unique_names() {
        let mut pm = ProcessManager::new();
        let n1 = pm
            .run_command("sleep 5".into(), Some("job".into()), None, HashMap::new(), 10, 40)
            .unwrap();
        let n2 = pm
            .run_command("sleep 5".into(), Some("job".into()), None, HashMap::new(), 10, 40)
            .unwrap();
        assert_eq!(n1, "job");
        assert_eq!(n2, "job-2");
        pm.stop_all();
    }

    #[test]
    fn run_command_output_is_readable() {
        let mut pm = ProcessManager::new();
        let name = pm
            .run_command(
                "sh -c 'echo hello-mcp; sleep 2'".into(),
                Some("greet".into()),
                None,
                HashMap::new(),
                10,
                40,
            )
            .unwrap();
        let text = drain_until(&mut pm, &name, |t| t.contains("hello-mcp"));
        assert!(text.contains("hello-mcp"), "output was: {:?}", text);
        pm.stop_all();
    }

    #[test]
    fn send_input_drives_process() {
        let mut pm = ProcessManager::new();
        // `cat` echoes back whatever we send to its stdin.
        let name = pm
            .run_command("cat".into(), Some("echoer".into()), None, HashMap::new(), 10, 40)
            .unwrap();
        let idx = pm.index_of(&name).unwrap();
        pm.processes[idx].write_input(b"ping-pong\r").unwrap();
        let text = drain_until(&mut pm, &name, |t| t.contains("ping-pong"));
        assert!(text.contains("ping-pong"), "output was: {:?}", text);
        pm.stop_all();
    }
}
