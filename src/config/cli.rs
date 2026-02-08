use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "betterprocs", about = "A better terminal process manager")]
pub struct Cli {
    /// Commands to run (e.g. "npm run dev" "cargo run")
    pub commands: Vec<String>,

    /// Path to config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Read scripts from package.json
    #[arg(long)]
    pub npm: bool,

    /// Auto-exit when all processes stop
    #[arg(long)]
    pub auto_exit: bool,

    /// Working directory
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Process names (comma-separated, matches positional commands)
    #[arg(long, value_delimiter = ',')]
    pub names: Vec<String>,
}
