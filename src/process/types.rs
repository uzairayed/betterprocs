use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ProcessStatus {
    NotStarted,
    Running { pid: u32 },
    Stopped { exit_code: i32 },
    Crashed {},
}

impl ProcessStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, ProcessStatus::Running { .. })
    }

    /// Sort priority: Running=0 (first), Crashed=1, Stopped=2, NotStarted=3
    pub fn sort_order(&self) -> u8 {
        match self {
            ProcessStatus::Running { .. } => 0,
            ProcessStatus::Crashed { .. } => 1,
            ProcessStatus::Stopped { .. } => 2,
            ProcessStatus::NotStarted => 3,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ProcessStatus::NotStarted => "NOT STARTED",
            ProcessStatus::Running { .. } => "RUNNING",
            ProcessStatus::Stopped { exit_code: 0, .. } => "STOPPED",
            ProcessStatus::Stopped { .. } => "EXITED",
            ProcessStatus::Crashed { .. } => "CRASHED",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessConfig {
    pub name: String,
    #[serde(default)]
    pub command: String,
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

fn default_true() -> bool {
    true
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            cmd: None,
            cwd: None,
            env: HashMap::new(),
            autostart: true,
            autorestart: false,
            port: None,
        }
    }
}
