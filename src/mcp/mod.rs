//! Headless MCP server mode.
//!
//! Exposes betterprocs' process management over the Model Context Protocol so
//! an AI agent can launch, inspect, drive, and stop processes without a TUI.
//! Run with `betterprocs mcp` — stdin/stdout become the MCP transport, so
//! nothing here may write to stdout (diagnostics go to stderr only).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::config::merged::AppConfig;
use crate::process::manager::ProcessManager;
use crate::process::types::ProcessStatus;

/// Fixed virtual terminal size for spawned PTYs (no real terminal to size to).
const PTY_ROWS: u16 = 40;
const PTY_COLS: u16 = 120;
/// Default number of output lines returned by read/copy tools.
const DEFAULT_LINES: usize = 200;

// ----- Tool parameter types -------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProcessName {
    /// Name of the process (as shown by `list_processes`).
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// Shell command line to run, e.g. "npm run dev".
    pub command: String,
    /// Optional name for the process. Defaults to the first word of the command.
    pub name: Option<String>,
    /// Optional working directory to run the command in.
    pub cwd: Option<String>,
    /// Optional environment variables to set for the process.
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadOutputArgs {
    /// Name of the process to read output from.
    pub name: String,
    /// Maximum number of trailing lines to return (default 200).
    pub lines: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendInputArgs {
    /// Name of the process to send input to.
    pub name: String,
    /// Text to write to the process's stdin.
    pub text: String,
    /// Append a carriage return to submit the input (default true).
    pub submit: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PortArg {
    /// TCP port number.
    pub port: u16,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KillPortArgs {
    /// TCP port whose listening process should be killed.
    pub port: u16,
    /// Use SIGKILL instead of SIGTERM (default false).
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    /// Substring to match against process name or full command line.
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KillPidArgs {
    /// PID of the process to kill.
    pub pid: u32,
    /// Use SIGKILL instead of SIGTERM (default false).
    pub force: Option<bool>,
}

// ----- Tool output types ----------------------------------------------------
// MCP requires structured tool output to have an object root schema, so each
// structured result is a named struct rather than a free-form JSON value.

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProcessEntry {
    pub name: String,
    pub status: String,
    pub pid: Option<u32>,
    pub command: String,
    pub cwd: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProcessList {
    pub processes: Vec<ProcessEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SpawnedProcess {
    pub name: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PortListener {
    pub port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProcMatch {
    pub pid: u32,
    pub name: String,
    pub command: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProcMatches {
    pub processes: Vec<ProcMatch>,
}

// ----- Server ---------------------------------------------------------------

#[derive(Clone)]
pub struct BetterprocsServer {
    manager: Arc<Mutex<ProcessManager>>,
    // Read by the generated `#[tool_handler]` dispatch; dead-code analysis
    // can't see through the macro expansion.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

fn invalid(msg: impl Into<std::borrow::Cow<'static, str>>) -> ErrorData {
    ErrorData::invalid_params(msg, None)
}

fn internal(msg: impl Into<std::borrow::Cow<'static, str>>) -> ErrorData {
    ErrorData::internal_error(msg, None)
}

#[tool_router]
impl BetterprocsServer {
    fn new(manager: Arc<Mutex<ProcessManager>>) -> Self {
        Self {
            manager,
            tool_router: Self::tool_router(),
        }
    }

    /// List all managed processes with their status, pid, command, and port.
    #[tool(description = "List all managed processes with status, pid, command, cwd, and port.")]
    async fn list_processes(&self) -> Result<Json<ProcessList>, ErrorData> {
        let pm = self.manager.lock().await;
        let processes = pm
            .processes
            .iter()
            .map(|h| ProcessEntry {
                name: h.config.name.clone(),
                status: h.status.label().to_string(),
                pid: match h.status {
                    ProcessStatus::Running { pid } => Some(pid),
                    _ => None,
                },
                command: h.config.command.clone(),
                cwd: h.config.cwd.as_ref().map(|p| p.display().to_string()),
                port: h.config.port,
            })
            .collect();
        Ok(Json(ProcessList { processes }))
    }

    /// Start a configured process by name.
    #[tool(description = "Start a process by name.")]
    async fn start_process(&self, Parameters(args): Parameters<ProcessName>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        pm.start(idx).map_err(|e| internal(e.to_string()))?;
        Ok(format!("started '{}'", args.name))
    }

    /// Stop a process gracefully (SIGTERM, then SIGKILL if needed).
    #[tool(description = "Stop a process gracefully (SIGTERM, escalating to SIGKILL).")]
    async fn stop_process(&self, Parameters(args): Parameters<ProcessName>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        pm.stop(idx).map_err(|e| internal(e.to_string()))?;
        Ok(format!("stopped '{}'", args.name))
    }

    /// Force-kill a process immediately (SIGKILL).
    #[tool(description = "Force-kill a process immediately (SIGKILL).")]
    async fn force_kill_process(&self, Parameters(args): Parameters<ProcessName>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        pm.force_kill(idx).map_err(|e| internal(e.to_string()))?;
        Ok(format!("force-killed '{}'", args.name))
    }

    /// Restart a process (stop then start).
    #[tool(description = "Restart a process (stop then start).")]
    async fn restart_process(&self, Parameters(args): Parameters<ProcessName>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        pm.restart(idx).map_err(|e| internal(e.to_string()))?;
        Ok(format!("restarted '{}'", args.name))
    }

    /// Run a brand-new arbitrary shell command as a managed process.
    #[tool(description = "Run a new arbitrary shell command as a managed process. Returns the assigned name and pid.")]
    async fn run_command(&self, Parameters(args): Parameters<RunCommandArgs>) -> Result<Json<SpawnedProcess>, ErrorData> {
        let mut pm = self.manager.lock().await;
        let name = pm
            .run_command(
                args.command,
                args.name,
                args.cwd.map(std::path::PathBuf::from),
                args.env.unwrap_or_default(),
                PTY_ROWS,
                PTY_COLS,
            )
            .map_err(|e| internal(e.to_string()))?;
        let pid = pm
            .index_of(&name)
            .and_then(|i| pm.processes.get(i))
            .and_then(|h| match h.status {
                ProcessStatus::Running { pid } => Some(pid),
                _ => None,
            });
        Ok(Json(SpawnedProcess { name, pid }))
    }

    /// Read recent terminal output of a process (includes scrollback).
    #[tool(description = "Read recent terminal output of a process, including scrollback. Returns plain text.")]
    async fn read_output(&self, Parameters(args): Parameters<ReadOutputArgs>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        let lines = args.lines.unwrap_or(DEFAULT_LINES);
        let handle = pm.processes.get_mut(idx).ok_or_else(|| internal("process disappeared"))?;
        Ok(handle.screen.tail_text(lines))
    }

    /// Send input to a process's stdin (submits with a carriage return by default).
    #[tool(description = "Send text to a process's stdin. Submits with a carriage return unless submit=false.")]
    async fn send_input(&self, Parameters(args): Parameters<SendInputArgs>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        let mut data = args.text.into_bytes();
        if args.submit.unwrap_or(true) {
            data.push(b'\r');
        }
        let handle = pm.processes.get_mut(idx).ok_or_else(|| internal("process disappeared"))?;
        handle.write_input(&data).map_err(|e| internal(e.to_string()))?;
        Ok("input sent".to_string())
    }

    /// Copy recent terminal output to the OS clipboard and return it.
    #[tool(description = "Copy recent terminal output of a process to the OS clipboard and return the text.")]
    async fn copy_output(&self, Parameters(args): Parameters<ReadOutputArgs>) -> Result<String, ErrorData> {
        let mut pm = self.manager.lock().await;
        let idx = pm.index_of(&args.name).ok_or_else(|| invalid(format!("no process named '{}'", args.name)))?;
        let lines = args.lines.unwrap_or(DEFAULT_LINES);
        let handle = pm.processes.get_mut(idx).ok_or_else(|| internal("process disappeared"))?;
        let text = handle.screen.tail_text(lines);
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text.clone());
        }
        Ok(text)
    }

    /// Find which process is listening on a TCP port.
    #[tool(description = "Find which process is listening on a TCP port (via lsof).")]
    async fn find_port(&self, Parameters(args): Parameters<PortArg>) -> Result<Json<PortListener>, ErrorData> {
        let found = tokio::task::spawn_blocking(move || crate::port::detector::find_process_on_port(args.port))
            .await
            .map_err(|e| internal(e.to_string()))?;
        Ok(Json(match found {
            Some((pid, name)) => PortListener { port: args.port, pid: Some(pid), process_name: Some(name) },
            None => PortListener { port: args.port, pid: None, process_name: None },
        }))
    }

    /// Kill whatever process is listening on a TCP port.
    #[tool(description = "Kill the process listening on a TCP port. Uses SIGTERM unless force=true.")]
    async fn kill_port(&self, Parameters(args): Parameters<KillPortArgs>) -> Result<String, ErrorData> {
        let port = args.port;
        let found = tokio::task::spawn_blocking(move || crate::port::detector::find_process_on_port(port))
            .await
            .map_err(|e| internal(e.to_string()))?;
        match found {
            Some((pid, name)) => {
                crate::system::killer::kill_process(pid, args.force.unwrap_or(false))
                    .map_err(|e| internal(e.to_string()))?;
                Ok(format!("killed {} (pid {}) on port {}", name, pid, port))
            }
            None => Err(invalid(format!("nothing listening on port {}", port))),
        }
    }

    /// Search running OS processes by name or command line.
    #[tool(description = "Search all running OS processes whose name or command line matches a substring.")]
    async fn search_processes(&self, Parameters(args): Parameters<SearchArgs>) -> Result<Json<ProcMatches>, ErrorData> {
        let results = tokio::task::spawn_blocking(move || crate::system::proc_search::search_processes(&args.query))
            .await
            .map_err(|e| internal(e.to_string()))?;
        let processes = results
            .into_iter()
            .map(|p| ProcMatch { pid: p.pid, name: p.name, command: p.command })
            .collect();
        Ok(Json(ProcMatches { processes }))
    }

    /// Kill an arbitrary OS process by PID.
    #[tool(description = "Kill an OS process by PID. Uses SIGTERM unless force=true.")]
    async fn kill_pid(&self, Parameters(args): Parameters<KillPidArgs>) -> Result<String, ErrorData> {
        crate::system::killer::kill_process(args.pid, args.force.unwrap_or(false))
            .map_err(|e| internal(e.to_string()))?;
        Ok(format!("sent signal to pid {}", args.pid))
    }
}

#[tool_handler]
impl ServerHandler for BetterprocsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "betterprocs process manager. Use list_processes to see managed \
                 processes, run_command to launch new ones, read_output/copy_output to \
                 read their terminal output, send_input to type into them, and \
                 start/stop/restart/force_kill to control them. find_port/kill_port and \
                 search_processes/kill_pid manage arbitrary OS processes and ports.",
            )
    }
}

/// Run the headless MCP server over stdio until the client disconnects.
pub async fn run_mcp(config: AppConfig) -> Result<()> {
    // Warn (on stderr) about port conflicts without the interactive prompt —
    // stdin is the MCP transport here.
    let conflicts = crate::port::detector::detect_conflicts(&config.processes);
    for c in &conflicts {
        eprintln!(
            "warning: port {} for \"{}\" is in use by {} (pid {})",
            c.port, c.our_process, c.process_name, c.pid
        );
    }

    // Build the process manager. Configured processes honor their autostart
    // flag (started inside ProcessHandle::new), matching TUI behavior.
    let mut pm = ProcessManager::new();
    for proc_config in config.processes {
        pm.add_process(proc_config, PTY_ROWS, PTY_COLS);
    }
    let manager = Arc::new(Mutex::new(pm));

    // Output pump: drains PTY output into vt100 screens and reaps exited
    // children, replacing the TUI event loop's role.
    let pump_manager = manager.clone();
    let pump = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(50));
        loop {
            ticker.tick().await;
            let mut pm = pump_manager.lock().await;
            pm.drain_output();
            pm.check_autorestart();
        }
    });

    let server = BetterprocsServer::new(manager.clone());
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    // Client disconnected: stop the pump and tear down child processes.
    pump.abort();
    manager.lock().await.stop_all();
    Ok(())
}
