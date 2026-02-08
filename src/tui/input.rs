use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::{ActiveTab, App, Scope};

use super::actions::Action;

pub fn handle_input(event: &Event, app: &App) -> Action {
    match event {
        Event::Key(key) => {
            if matches!(app.active_tab, ActiveTab::PortKiller) {
                return handle_port_killer_keys(key);
            }

            match app.ui_state.scope {
                Scope::ProcessList => handle_process_list_keys(key),
                Scope::Terminal | Scope::TerminalZoomed => handle_terminal_keys(key),
            }
        }
        Event::Mouse(mouse) => handle_mouse(mouse, app),
        Event::Resize(w, h) => Action::Resize(*w, *h),
        _ => Action::None,
    }
}

fn handle_process_list_keys(key: &KeyEvent) -> Action {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::SelectNext,
        KeyCode::Char('k') | KeyCode::Up => Action::SelectPrev,
        KeyCode::Char('s') => Action::StartProcess,
        KeyCode::Char('x') => Action::StopProcess,
        KeyCode::Char('X') => Action::ForceKill,
        KeyCode::Char('r') => Action::RestartProcess,
        KeyCode::Char('c') => Action::ClearLogs,
        KeyCode::Tab | KeyCode::Enter => Action::FocusTerminal,
        KeyCode::Char('z') => Action::ToggleZoom,
        KeyCode::Char('?') => Action::ToggleKeymap,
        KeyCode::Char('`') | KeyCode::F(2) => Action::SwitchToPortKiller,
        _ => Action::None,
    }
}

fn handle_port_killer_keys(key: &KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
    }

    match key.code {
        KeyCode::Esc | KeyCode::F(1) | KeyCode::Tab | KeyCode::Char('`') => Action::SwitchToProcesses,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Down => Action::SelectNext,
        KeyCode::Up => Action::SelectPrev,
        KeyCode::Char('x') => Action::StopProcess,
        KeyCode::Char('X') => Action::ForceKill,
        KeyCode::Char(c) if c.is_ascii_digit() || c == ',' || c == ' ' => {
            Action::PortKillerType(c)
        }
        KeyCode::Backspace => Action::PortKillerBackspace,
        KeyCode::Delete => Action::PortKillerClear,
        _ => Action::None,
    }
}

fn handle_terminal_keys(key: &KeyEvent) -> Action {
    if key.code == KeyCode::Tab {
        return Action::FocusProcessList;
    }

    if key.code == KeyCode::Char('q') {
        return Action::Quit;
    }

    if key.code == KeyCode::Char('`') {
        return Action::SwitchToPortKiller;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('a') => return Action::FocusProcessList,
            _ => {}
        }
    }

    if let Some(bytes) = key_event_to_bytes(key) {
        Action::SendInput(bytes)
    } else {
        Action::None
    }
}

fn handle_mouse(mouse: &MouseEvent, app: &App) -> Action {
    let (term_cols, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let list_width = term_cols / 4;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let x = mouse.column;
            let y = mouse.row;

            // Click on status bar (row 0) — check for tab clicks
            if y == 0 {
                // The status bar looks like: " betterprocs  X/Y running  [Processes] [Port Killer]"
                // Rather than computing exact spans, find the tab text positions
                let running = app.process_manager.processes.iter()
                    .filter(|p| p.status.is_running()).count();
                let total = app.process_manager.process_count();
                let prefix_len = " betterprocs ".len()
                    + format!(" {}/{} running ", running, total).len()
                    + 1; // space
                let processes_start = prefix_len;
                let processes_end = processes_start + "[Processes]".len();
                let portkiller_start = processes_end + 1; // space
                let portkiller_end = portkiller_start + "[Port Killer]".len();

                let col = x as usize;
                if col >= processes_start && col < processes_end {
                    return Action::SwitchToProcesses;
                }
                if col >= portkiller_start && col <= portkiller_end {
                    return Action::SwitchToPortKiller;
                }
                return Action::None;
            }

            if x < list_width && !matches!(app.ui_state.scope, Scope::TerminalZoomed) {
                // Click in process list area — always focus it
                if y >= 2 {
                    let idx = (y - 2) as usize;
                    if idx < app.process_manager.process_count() {
                        return Action::SelectIndex(idx);
                    }
                }
                // Clicked empty space in process list — just focus it
                Action::FocusProcessList
            } else {
                // Click in output pane — start selection
                Action::MouseDragStart(mouse.column, mouse.row)
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Continue drag selection
            Action::MouseDragEnd(mouse.column, mouse.row)
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.ui_state.selection_start.is_some()
                && app.ui_state.selection_end.is_some()
            {
                Action::CopySelection
            } else {
                // Only focus output pane if the click was in the output area
                if mouse.column >= list_width
                    || matches!(app.ui_state.scope, Scope::TerminalZoomed)
                {
                    Action::ClickOutputPane
                } else {
                    Action::None
                }
            }
        }
        MouseEventKind::ScrollUp => Action::ScrollUp(3),
        MouseEventKind::ScrollDown => Action::ScrollDown(3),
        _ => Action::None,
    }
}

fn key_event_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char(c) => {
            if ctrl {
                let byte = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                if byte <= 26 {
                    Some(vec![byte])
                } else {
                    Some(c.to_string().into_bytes())
                }
            } else {
                Some(c.to_string().into_bytes())
            }
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        KeyCode::F(n) => {
            let seq = match n {
                1 => b"\x1bOP".to_vec(),
                2 => b"\x1bOQ".to_vec(),
                3 => b"\x1bOR".to_vec(),
                4 => b"\x1bOS".to_vec(),
                5 => b"\x1b[15~".to_vec(),
                6 => b"\x1b[17~".to_vec(),
                7 => b"\x1b[18~".to_vec(),
                8 => b"\x1b[19~".to_vec(),
                9 => b"\x1b[20~".to_vec(),
                10 => b"\x1b[21~".to_vec(),
                11 => b"\x1b[23~".to_vec(),
                12 => b"\x1b[24~".to_vec(),
                _ => return None,
            };
            Some(seq)
        }
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Tab => Some(vec![b'\t']),
        _ => None,
    }
}
