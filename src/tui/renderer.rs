use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::{ActiveTab, App, Scope};
use crate::process::types::ProcessStatus;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1), // Status bar
        Constraint::Min(0),   // Main area
        Constraint::Length(1), // Keymap bar
    ])
    .split(area);

    render_status_bar(frame, chunks[0], app);
    render_main_area(frame, chunks[1], app);

    if app.ui_state.show_keymap {
        render_keymap_bar(frame, chunks[2], app);
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let running = app
        .process_manager
        .processes
        .iter()
        .filter(|p| p.status.is_running())
        .count();
    let total = app.process_manager.process_count();

    let title = Line::from(vec![
        Span::styled(
            " betterprocs ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}/{} running ", running, total),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" "),
        Span::styled(
            "[Processes]",
            if matches!(app.active_tab, ActiveTab::Processes) {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::raw(" "),
        Span::styled(
            "[Port Killer]",
            if matches!(app.active_tab, ActiveTab::PortKiller) {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
    ]);

    frame.render_widget(Paragraph::new(title), area);
}

fn render_main_area(frame: &mut Frame, area: Rect, app: &App) {
    match app.active_tab {
        ActiveTab::PortKiller => {
            render_port_killer(frame, area, app);
        }
        ActiveTab::Processes => {
            if matches!(app.ui_state.scope, Scope::TerminalZoomed) {
                render_output_pane(frame, area, app);
                return;
            }

            let chunks = Layout::horizontal([
                Constraint::Percentage(25),
                Constraint::Min(0),
            ])
            .split(area);

            render_process_list(frame, chunks[0], app);
            render_output_pane(frame, chunks[1], app);
        }
    }
}

fn render_process_list(frame: &mut Frame, area: Rect, app: &App) {
    let focused = matches!(app.ui_state.scope, Scope::ProcessList);

    let block = Block::default()
        .title(" Processes ")
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    if app.process_manager.processes.is_empty() {
        let items = vec![ListItem::new(Line::from(vec![
            Span::styled("◌ ", Style::default().fg(Color::DarkGray)),
            Span::raw("No processes"),
        ]))];
        let list = List::new(items).block(block);
        frame.render_widget(list, area);
        return;
    }

    let items: Vec<ListItem> = app
        .process_manager
        .processes
        .iter()
        .map(|handle| {
            let (icon, icon_style) = match &handle.status {
                ProcessStatus::Running { .. } => (
                    "●",
                    Style::default().fg(Color::Green),
                ),
                ProcessStatus::Stopped { exit_code: 0, .. } => (
                    "○",
                    Style::default().fg(Color::DarkGray),
                ),
                ProcessStatus::Stopped { .. } => (
                    "✗",
                    Style::default().fg(Color::Yellow),
                ),
                ProcessStatus::Crashed { .. } => (
                    "✗",
                    Style::default().fg(Color::Red),
                ),
                ProcessStatus::NotStarted => (
                    "◌",
                    Style::default().fg(Color::DarkGray),
                ),
            };

            let status_label = handle.status.label();

            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", icon), icon_style),
                Span::raw(&handle.config.name),
                Span::styled(
                    format!(" [{}]", status_label),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let mut state = ListState::default().with_selected(Some(app.ui_state.selected_process));

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_output_pane(frame: &mut Frame, area: Rect, app: &App) {
    let focused = matches!(
        app.ui_state.scope,
        Scope::Terminal | Scope::TerminalZoomed
    );

    let selected = app.ui_state.selected_process;
    let handle = app.process_manager.processes.get(selected);

    let title = match handle {
        Some(h) => format!(" {} - {} ", h.config.name, h.status.label()),
        None => " Output ".to_string(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Compute selection range in terminal-absolute coordinates
    let selection = compute_selection(app);
    let copy_flash = app.ui_state.copy_flash > 0;

    // Render terminal output from vt100 screen
    // vt100's set_scrollback() makes cell() return scrollback-aware content,
    // so we just render row 0..height directly.
    if let Some(handle) = handle {
        let screen = handle.screen.screen();

        for row in 0..inner.height {
            let abs_y = inner.y + row;
            let line = render_screen_row(screen, row, inner.width, inner.x, abs_y, &selection, copy_flash);
            frame.render_widget(
                Paragraph::new(line),
                Rect::new(inner.x, abs_y, inner.width, 1),
            );
        }
    }
}

/// Normalized selection: (start_row, start_col, end_row, end_col) in absolute terminal coords.
/// Returns None if no active selection.
fn compute_selection(app: &App) -> Option<(u16, u16, u16, u16)> {
    let start = app.ui_state.selection_start?;
    let end = app.ui_state.selection_end?;

    // Normalize so start <= end (row-major order)
    if start.1 < end.1 || (start.1 == end.1 && start.0 <= end.0) {
        Some((start.1, start.0, end.1, end.0))
    } else {
        Some((end.1, end.0, start.1, start.0))
    }
}

/// Check if a cell at (abs_x, abs_y) is within the selection.
fn is_selected(abs_x: u16, abs_y: u16, sel: &Option<(u16, u16, u16, u16)>) -> bool {
    let (sr, sc, er, ec) = match sel {
        Some(s) => *s,
        None => return false,
    };

    if abs_y < sr || abs_y > er {
        return false;
    }
    if abs_y == sr && abs_y == er {
        // Single row selection
        return abs_x >= sc && abs_x <= ec;
    }
    if abs_y == sr {
        return abs_x >= sc;
    }
    if abs_y == er {
        return abs_x <= ec;
    }
    // Middle rows are fully selected
    true
}

const SELECT_STYLE: Style = Style::new().bg(Color::Indexed(240)).fg(Color::White);
const COPIED_STYLE: Style = Style::new().bg(Color::Green).fg(Color::Black);

fn render_screen_row(
    screen: &vt100::Screen,
    row: u16,
    cols: u16,
    abs_x_start: u16,
    abs_y: u16,
    selection: &Option<(u16, u16, u16, u16)>,
    copy_flash: bool,
) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_style = Style::default();

    for col in 0..cols {
        let cell = screen.cell(row, col);
        let abs_x = abs_x_start + col;
        let selected = is_selected(abs_x, abs_y, selection);

        let base_style = match &cell {
            Some(cell) => vt100_cell_to_style(cell),
            None => Style::default(),
        };

        let style = if selected {
            if copy_flash { COPIED_STYLE } else { SELECT_STYLE }
        } else {
            base_style
        };

        let ch = cell
            .map(|c| c.contents().chars().next().unwrap_or(' '))
            .unwrap_or(' ');

        if style != current_style && !current_text.is_empty() {
            spans.push(Span::styled(current_text.clone(), current_style));
            current_text.clear();
        }
        current_style = style;
        current_text.push(ch);
    }

    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    Line::from(spans)
}

fn render_port_killer(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Port input
        Constraint::Min(0),   // Port table
    ])
    .split(area);

    // Port input field
    let input_text = if app.port_killer.port_input.is_empty() {
        "Type port numbers to filter (e.g. 3000, 5173, 8080)...".to_string()
    } else {
        app.port_killer.port_input.clone()
    };

    let input_style = if app.port_killer.port_input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let input = Paragraph::new(Span::styled(input_text, input_style)).block(
        Block::default()
            .title(" Filter Ports ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(input, chunks[0]);

    // Port table
    let block = Block::default()
        .title(format!(
            " Listening Ports ({}) ",
            app.port_killer.entries().len()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    if inner.height < 2 {
        return;
    }

    let entries = app.port_killer.entries();

    if entries.is_empty() {
        let msg = if app.port_killer.port_input.is_empty() {
            "No processes listening on any ports."
        } else {
            "No processes found on those ports."
        };
        frame.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))),
            Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(1), 1),
        );
        return;
    }

    let header = Row::new(vec!["Port", "PID", "Process", "Protocol"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let rows: Vec<Row> = entries
        .iter()
        .map(|e| {
            Row::new(vec![
                format!(":{}", e.port),
                e.pid.to_string(),
                e.process_name.clone(),
                e.protocol.clone(),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Length(8),
    ];

    let mut state = TableState::default().with_selected(Some(app.port_killer.selected));

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(table, inner, &mut state);
}

fn vt100_cell_to_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default();

    // Foreground color
    style = style.fg(vt100_color_to_ratatui(cell.fgcolor()));

    // Background color
    let bg = cell.bgcolor();
    if !matches!(bg, vt100::Color::Default) {
        style = style.bg(vt100_color_to_ratatui(bg));
    }

    // Attributes
    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }

    style
}

fn vt100_color_to_ratatui(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn render_keymap_bar(frame: &mut Frame, area: Rect, app: &App) {
    let keys = if matches!(app.active_tab, ActiveTab::PortKiller) {
        vec![
            ("0-9", "type port"),
            ("Up/Down", "select"),
            ("x", "kill"),
            ("X", "force kill"),
            ("Del", "clear"),
            ("`", "processes"),
        ]
    } else {
        match app.ui_state.scope {
            Scope::ProcessList => vec![
                ("q", "quit"),
                ("j/k", "navigate"),
                ("s", "start"),
                ("x", "stop"),
                ("r", "restart"),
                ("c", "clear"),
                ("Tab", "terminal"),
                ("z", "zoom"),
("`", "ports"),
            ],
            Scope::Terminal | Scope::TerminalZoomed => vec![
                ("Tab", "back"),
                ("drag", "select+copy"),
            ],
        }
    };

    let spans: Vec<Span> = keys
        .into_iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{} ", desc), Style::default().fg(Color::DarkGray)),
            ]
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
