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
