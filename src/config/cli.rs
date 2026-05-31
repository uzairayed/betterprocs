use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run as a headless MCP server over stdio (for AI agents)
    Mcp,
}

#[derive(Parser, Debug)]
#[command(name = "betterprocs", about = "A better terminal process manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Commands to run (e.g. "npm run dev" "cargo run")
    pub commands: Vec<String>,

    /// Path to config file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Read scripts from package.json
    #[arg(long, global = true)]
    pub npm: bool,

    /// Auto-exit when all processes stop
    #[arg(long, global = true)]
    pub auto_exit: bool,

    /// Working directory
    #[arg(long, global = true)]
    pub cwd: Option<PathBuf>,

    /// Process names (comma-separated, matches positional commands)
    #[arg(long, value_delimiter = ',', global = true)]
    pub names: Vec<String>,
}
