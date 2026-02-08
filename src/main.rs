mod app;
mod config;
mod port;
mod process;
mod system;
mod terminal;
mod tui;

use anyhow::Result;
use app::App;
use clap::Parser;
use config::cli::Cli;
use config::merged::load_config;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config before entering TUI (errors print to normal terminal)
    let app_config = load_config(&cli)?;

    // Port conflict detection (runs before TUI)
    let conflicts = port::detector::detect_conflicts(&app_config.processes);
    if !port::detector::handle_conflicts(&conflicts)? {
        return Ok(());
    }

    // Install panic hook that restores terminal before printing panic info
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;

    // Run app
    let mut app = App::new(app_config);
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}
