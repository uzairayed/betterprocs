use anyhow::Result;
use crossterm::event;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

use crate::config::merged::AppConfig;
use crate::process::manager::ProcessManager;
use crate::system::browser::PortKiller;
use crate::system::killer;
use crate::tui::{actions::Action, input::handle_input, renderer::render};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Processes,
    PortKiller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    ProcessList,
    Terminal,
    TerminalZoomed,
}

pub struct UiState {
    pub selected_process: usize,
    pub scope: Scope,
    pub show_keymap: bool,
    /// Mouse selection start (col, row) in absolute terminal coordinates
    pub selection_start: Option<(u16, u16)>,
    /// Mouse selection end (col, row) in absolute terminal coordinates
    pub selection_end: Option<(u16, u16)>,
}

pub struct App {
    pub should_quit: bool,
    pub active_tab: ActiveTab,
    pub ui_state: UiState,
    pub process_manager: ProcessManager,
    pub port_killer: PortKiller,
    pub auto_exit: bool,
}

/// Calculate the output pane dimensions from the total terminal size.
fn pane_size(term_cols: u16, term_rows: u16) -> (u16, u16) {
    let list_width = term_cols / 4;
    let pane_cols = term_cols.saturating_sub(list_width + 2);
    let pane_rows = term_rows.saturating_sub(4);
    (pane_rows.max(1), pane_cols.max(1))
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let (term_cols, term_rows) =
            crossterm::terminal::size().unwrap_or((80, 24));
        let (pane_rows, pane_cols) = pane_size(term_cols, term_rows);

        let mut pm = ProcessManager::new();
        for proc_config in config.processes {
            pm.add_process(proc_config, pane_rows, pane_cols);
        }

        Self {
            should_quit: false,
            active_tab: ActiveTab::Processes,
            ui_state: UiState {
                selected_process: 0,
                scope: Scope::ProcessList,
                show_keymap: true,
                selection_start: None,
                selection_end: None,
            },
            process_manager: pm,
            port_killer: PortKiller::new(),
            auto_exit: config.auto_exit,
        }
    }

    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            self.process_manager.drain_output();
            self.process_manager.check_autorestart();

            self.ui_state.selected_process = self
                .process_manager
                .sort_by_status(self.ui_state.selected_process);

            if matches!(self.active_tab, ActiveTab::PortKiller) {
                self.port_killer.maybe_refresh();
            }

            terminal.draw(|frame| render(frame, self))?;

            if event::poll(Duration::from_millis(50))? {
                let evt = event::read()?;
                let action = handle_input(&evt, self);
                self.dispatch(action);
            }

            if self.auto_exit
                && self.process_manager.process_count() > 0
                && self.process_manager.all_stopped()
            {
                self.should_quit = true;
            }

            if self.should_quit {
                self.process_manager.stop_all();
                break;
            }
        }

        Ok(())
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::SelectNext => {
                if matches!(self.active_tab, ActiveTab::PortKiller) {
                    self.port_killer.select_next();
                } else {
                    let count = self.process_manager.process_count();
                    if count > 0 {
                        self.ui_state.selected_process =
                            (self.ui_state.selected_process + 1) % count;
                    }
                }
            }
            Action::SelectPrev => {
                if matches!(self.active_tab, ActiveTab::PortKiller) {
                    self.port_killer.select_prev();
                } else {
                    let count = self.process_manager.process_count();
                    if count > 0 {
                        if self.ui_state.selected_process == 0 {
                            self.ui_state.selected_process = count - 1;
                        } else {
                            self.ui_state.selected_process -= 1;
                        }
                    }
                }
            }
            Action::StartProcess => {
                let idx = self.ui_state.selected_process;
                let _ = self.process_manager.start(idx);
            }
            Action::StopProcess => {
                if matches!(self.active_tab, ActiveTab::PortKiller) {
                    if let Some(pid) = self.port_killer.selected_pid() {
                        let _ = killer::kill_process(pid, false);
                        self.port_killer.refresh();
                    }
                } else {
                    let idx = self.ui_state.selected_process;
                    let _ = self.process_manager.stop(idx);
                }
            }
            Action::ForceKill => {
                if matches!(self.active_tab, ActiveTab::PortKiller) {
                    if let Some(pid) = self.port_killer.selected_pid() {
                        let _ = killer::kill_process(pid, true);
                        self.port_killer.refresh();
                    }
                } else {
                    let idx = self.ui_state.selected_process;
                    let _ = self.process_manager.force_kill(idx);
                }
            }
            Action::RestartProcess => {
                let idx = self.ui_state.selected_process;
                let _ = self.process_manager.restart(idx);
            }
            Action::SelectIndex(idx) => {
                let count = self.process_manager.process_count();
                if idx < count {
                    self.ui_state.selected_process = idx;
                }
            }
            Action::ClickOutputPane => {
                self.ui_state.scope = Scope::Terminal;
                self.ui_state.selection_start = None;
                self.ui_state.selection_end = None;
            }
            Action::MouseDragStart(col, row) => {
                self.ui_state.selection_start = Some((col, row));
                self.ui_state.selection_end = None;
            }
            Action::MouseDragEnd(col, row) => {
                self.ui_state.selection_end = Some((col, row));
            }
            Action::CopySelection => {
                self.copy_selection_to_clipboard();
                self.ui_state.selection_start = None;
                self.ui_state.selection_end = None;
            }
            Action::FocusProcessList => {
                self.ui_state.scope = Scope::ProcessList;
            }
            Action::FocusTerminal => {
                self.ui_state.scope = Scope::Terminal;
            }
            Action::ToggleZoom => {
                self.ui_state.scope = match self.ui_state.scope {
                    Scope::TerminalZoomed => Scope::Terminal,
                    _ => Scope::TerminalZoomed,
                };
            }
            Action::ToggleKeymap => {
                self.ui_state.show_keymap = !self.ui_state.show_keymap;
            }
            Action::ScrollUp(n) => {
                if let Some(handle) = self
                    .process_manager
                    .processes
                    .get_mut(self.ui_state.selected_process)
                {
                    handle.screen.scroll_up(n as usize);
                }
            }
            Action::ScrollDown(n) => {
                if let Some(handle) = self
                    .process_manager
                    .processes
                    .get_mut(self.ui_state.selected_process)
                {
                    handle.screen.scroll_down(n as usize);
                }
            }
            Action::SendInput(data) => {
                if let Some(handle) = self
                    .process_manager
                    .processes
                    .get_mut(self.ui_state.selected_process)
                {
                    let _ = handle.write_input(&data);
                }
            }
            Action::Resize(w, h) => {
                let (pane_rows, pane_cols) = pane_size(w, h);
                self.process_manager.resize_all(pane_rows, pane_cols);
            }
            Action::SwitchToPortKiller => {
                self.active_tab = ActiveTab::PortKiller;
                self.port_killer.refresh();
            }
            Action::SwitchToProcesses => {
                self.active_tab = ActiveTab::Processes;
            }
            Action::PortKillerType(c) => {
                self.port_killer.type_char(c);
            }
            Action::PortKillerBackspace => {
                self.port_killer.backspace();
            }
            Action::PortKillerClear => {
                self.port_killer.clear_input();
            }
            Action::None => {}
        }
    }

    fn copy_selection_to_clipboard(&self) {
        let (start, end) = match (self.ui_state.selection_start, self.ui_state.selection_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return,
        };

        let handle = match self.process_manager.processes.get(self.ui_state.selected_process) {
            Some(h) => h,
            None => return,
        };

        let screen = handle.screen.screen();

        // Calculate the output pane offset
        // The pane inner area starts after: process list (25%) + border, status bar + border
        let (term_cols, _) = crossterm::terminal::size().unwrap_or((80, 24));
        let list_width = term_cols / 4;
        let pane_x_offset = list_width + 1; // left border of output pane
        let pane_y_offset: u16 = 2; // status bar + top border

        // Convert terminal coordinates to screen-relative coordinates
        let start_col = start.0.saturating_sub(pane_x_offset);
        let start_row = start.1.saturating_sub(pane_y_offset);
        let end_col = end.0.saturating_sub(pane_x_offset);
        let end_row = end.1.saturating_sub(pane_y_offset);

        // Ensure start <= end (row-wise)
        let (sr, sc, er, ec) = if start_row < end_row || (start_row == end_row && start_col <= end_col) {
            (start_row, start_col, end_row, end_col)
        } else {
            (end_row, end_col, start_row, start_col)
        };

        // Extract text from vt100 screen
        let mut text = String::new();
        for row in sr..=er {
            let col_start = if row == sr { sc } else { 0 };
            let col_end = if row == er { ec } else { screen.size().1.saturating_sub(1) };

            for col in col_start..=col_end {
                if let Some(cell) = screen.cell(row, col) {
                    let contents = cell.contents();
                    if contents.is_empty() {
                        text.push(' ');
                    } else {
                        text.push_str(&contents);
                    }
                }
            }

            // Trim trailing spaces on each line and add newline between rows
            if row < er {
                let trimmed = text.trim_end().len();
                text.truncate(trimmed);
                text.push('\n');
            }
        }

        let text = text.trim_end().to_string();
        if text.is_empty() {
            return;
        }

        // Copy to clipboard
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
    }
}
