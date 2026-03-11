use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table, Wrap,
};
use ratatui::Terminal;

use velos_core::VelosError;

// ── Catppuccin Mocha palette ────────────────────────────────────────
mod cat {
    use ratatui::style::Color;
    pub const BASE: Color = Color::Rgb(30, 30, 46);
    pub const SURFACE0: Color = Color::Rgb(49, 50, 68);
    pub const SURFACE1: Color = Color::Rgb(69, 71, 90);
    pub const SURFACE2: Color = Color::Rgb(88, 91, 112);
    pub const OVERLAY0: Color = Color::Rgb(108, 112, 134);
    pub const SUBTEXT0: Color = Color::Rgb(166, 172, 205);
    pub const SUBTEXT1: Color = Color::Rgb(186, 194, 222);
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
    pub const BLUE: Color = Color::Rgb(137, 180, 250);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);
}

// ── App state ───────────────────────────────────────────────────────

#[derive(PartialEq)]
enum Mode {
    Normal,
    Filter,
    Detail,
    Signal,
    Help,
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    Name,
    Cpu,
    Mem,
    Uptime,
    Restarts,
}

impl SortColumn {
    fn next(self) -> Self {
        match self {
            Self::Name => Self::Cpu,
            Self::Cpu => Self::Mem,
            Self::Mem => Self::Uptime,
            Self::Uptime => Self::Restarts,
            Self::Restarts => Self::Name,
        }
    }
}

struct Notification {
    message: String,
    expires: Instant,
}

struct AppState {
    processes: Vec<ProcessRow>,
    selected: usize,
    logs: Vec<LogLine>,
    log_scroll: u16,
    should_quit: bool,
    mem_history: Vec<Vec<u64>>,
    cpu_history: Vec<Vec<u64>>,
    mode: Mode,
    filter_text: String,
    sort_by: SortColumn,
    sort_asc: bool,
    signal_selected: usize,
    notifications: Vec<Notification>,
    prev_restarts: Vec<(u32, u32)>, // (id, restart_count)
}

struct ProcessRow {
    id: u32,
    name: String,
    pid: u32,
    status: String,
    memory: u64,
    cpu_percent: f32,
    uptime: u64,
    restarts: u32,
}

struct LogLine {
    stream: &'static str,
    message: String,
}

const SIGNALS: &[&str] = &["SIGTERM", "SIGKILL", "SIGHUP", "SIGUSR1", "SIGUSR2"];

pub async fn run() -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let procs = client.list().await?;

    let mut state = AppState {
        processes: procs
            .iter()
            .map(|p| ProcessRow {
                id: p.id,
                name: p.name.clone(),
                pid: p.pid,
                status: p.status_str().to_string(),
                memory: p.memory_bytes,
                cpu_percent: p.cpu_percent,
                uptime: p.uptime_ms,
                restarts: p.restart_count,
            })
            .collect(),
        selected: 0,
        logs: Vec::new(),
        log_scroll: 0,
        should_quit: false,
        mem_history: Vec::new(),
        cpu_history: Vec::new(),
        mode: Mode::Normal,
        filter_text: String::new(),
        sort_by: SortColumn::Name,
        sort_asc: true,
        signal_selected: 0,
        notifications: Vec::new(),
        prev_restarts: procs.iter().map(|p| (p.id, p.restart_count)).collect(),
    };

    enable_raw_mode().map_err(|e| VelosError::ProtocolError(format!("terminal: {e}")))?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .map_err(|e| VelosError::ProtocolError(format!("terminal: {e}")))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| VelosError::ProtocolError(format!("terminal: {e}")))?;

    let result = run_loop(&mut terminal, &mut state).await;

    disable_raw_mode().ok();
    io::stdout().execute(LeaveAlternateScreen).ok();

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<(), VelosError> {
    loop {
        terminal
            .draw(|f| draw_ui(f, state))
            .map_err(|e| VelosError::ProtocolError(format!("draw: {e}")))?;

        if event::poll(Duration::from_secs(2)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match state.mode {
                    Mode::Filter => match key.code {
                        KeyCode::Esc => {
                            state.mode = Mode::Normal;
                            state.filter_text.clear();
                        }
                        KeyCode::Enter => {
                            state.mode = Mode::Normal;
                        }
                        KeyCode::Backspace => {
                            state.filter_text.pop();
                        }
                        KeyCode::Char(c) => {
                            state.filter_text.push(c);
                            state.selected = 0;
                        }
                        _ => {}
                    },
                    Mode::Detail => match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            state.mode = Mode::Normal;
                        }
                        _ => {}
                    },
                    Mode::Signal => match key.code {
                        KeyCode::Esc => {
                            state.mode = Mode::Normal;
                        }
                        KeyCode::Up => {
                            if state.signal_selected > 0 {
                                state.signal_selected -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if state.signal_selected + 1 < SIGNALS.len() {
                                state.signal_selected += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(proc) =
                                filtered_processes(state).get(state.selected).map(|p| p.id)
                            {
                                let sig = match state.signal_selected {
                                    0 => 15, // SIGTERM
                                    1 => 9,  // SIGKILL
                                    2 => 1,  // SIGHUP
                                    3 => 10, // SIGUSR1
                                    4 => 12, // SIGUSR2
                                    _ => 15,
                                };
                                if let Ok(mut c) = super::connect().await {
                                    let _ = c.signal(proc, sig).await;
                                }
                            }
                            state.mode = Mode::Normal;
                        }
                        _ => {}
                    },
                    Mode::Help => match key.code {
                        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                            state.mode = Mode::Normal;
                        }
                        _ => {}
                    },
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            state.should_quit = true;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if state.selected > 0 {
                                state.selected -= 1;
                                state.log_scroll = 0;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let max = filtered_processes(state).len();
                            if state.selected + 1 < max {
                                state.selected += 1;
                                state.log_scroll = 0;
                            }
                        }
                        KeyCode::Char('r') => {
                            if let Some(proc) =
                                filtered_processes(state).get(state.selected).map(|p| p.id)
                            {
                                if let Ok(mut c) = super::connect().await {
                                    let _ = c.restart(proc).await;
                                }
                            }
                        }
                        KeyCode::Char('s') => {
                            if let Some(proc) =
                                filtered_processes(state).get(state.selected).map(|p| p.id)
                            {
                                if let Ok(mut c) = super::connect().await {
                                    let _ = c.stop(proc).await;
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            if let Some(proc) =
                                filtered_processes(state).get(state.selected).map(|p| p.id)
                            {
                                if let Ok(mut c) = super::connect().await {
                                    let _ = c.delete(proc).await;
                                }
                            }
                        }
                        KeyCode::Char('/') => {
                            state.mode = Mode::Filter;
                            state.filter_text.clear();
                        }
                        KeyCode::Tab => {
                            state.sort_by = state.sort_by.next();
                        }
                        KeyCode::Enter => {
                            if !filtered_processes(state).is_empty() {
                                state.mode = Mode::Detail;
                            }
                        }
                        KeyCode::Char('K') => {
                            if !filtered_processes(state).is_empty() {
                                state.mode = Mode::Signal;
                                state.signal_selected = 0;
                            }
                        }
                        KeyCode::Char('?') => {
                            state.mode = Mode::Help;
                        }
                        KeyCode::PageUp => {
                            state.log_scroll = state.log_scroll.saturating_add(5);
                        }
                        KeyCode::PageDown => {
                            state.log_scroll = state.log_scroll.saturating_sub(5);
                        }
                        _ => {}
                    },
                }
            }
        }

        if state.should_quit {
            return Ok(());
        }

        // Expire notifications
        state.notifications.retain(|n| n.expires > Instant::now());

        // Refresh data
        if let Ok(mut client) = super::connect().await {
            if let Ok(procs) = client.list().await {
                // Detect crashes by restart_count increase
                for p in &procs {
                    if let Some(prev) = state.prev_restarts.iter().find(|(id, _)| *id == p.id) {
                        if p.restart_count > prev.1 {
                            state.notifications.push(Notification {
                                message: format!("{} restarted (x{})", p.name, p.restart_count),
                                expires: Instant::now() + Duration::from_secs(5),
                            });
                        }
                    }
                }
                state.prev_restarts = procs.iter().map(|p| (p.id, p.restart_count)).collect();

                state.processes = procs
                    .iter()
                    .map(|p| ProcessRow {
                        id: p.id,
                        name: p.name.clone(),
                        pid: p.pid,
                        status: p.status_str().to_string(),
                        memory: p.memory_bytes,
                        cpu_percent: p.cpu_percent,
                        uptime: p.uptime_ms,
                        restarts: p.restart_count,
                    })
                    .collect();
                if state.selected >= state.processes.len() && !state.processes.is_empty() {
                    state.selected = state.processes.len() - 1;
                }

                // Record history
                if state.mem_history.len() != state.processes.len() {
                    state.mem_history.resize(state.processes.len(), Vec::new());
                    state.cpu_history.resize(state.processes.len(), Vec::new());
                }
                for (i, p) in state.processes.iter().enumerate() {
                    if i < state.mem_history.len() {
                        state.mem_history[i].push(p.memory / 1024);
                        if state.mem_history[i].len() > 120 {
                            state.mem_history[i].remove(0);
                        }
                    }
                    if i < state.cpu_history.len() {
                        state.cpu_history[i].push((p.cpu_percent * 10.0) as u64);
                        if state.cpu_history[i].len() > 120 {
                            state.cpu_history[i].remove(0);
                        }
                    }
                }
            }

            // Auto-fetch logs for selected process
            let selected_id = filtered_processes(state).get(state.selected).map(|p| p.id);
            if let Some(id) = selected_id {
                if let Ok(entries) = client.logs(id, 50).await {
                    state.logs = entries
                        .iter()
                        .map(|e| LogLine {
                            stream: if e.stream == 1 { "err" } else { "out" },
                            message: e.message.clone(),
                        })
                        .collect();
                }
            }
        }
    }
}

fn filtered_processes(state: &AppState) -> Vec<&ProcessRow> {
    let mut procs: Vec<&ProcessRow> = state
        .processes
        .iter()
        .filter(|p| {
            state.filter_text.is_empty()
                || p.name
                    .to_lowercase()
                    .contains(&state.filter_text.to_lowercase())
        })
        .collect();

    match state.sort_by {
        SortColumn::Name => procs.sort_by(|a, b| a.name.cmp(&b.name)),
        SortColumn::Cpu => procs.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortColumn::Mem => procs.sort_by(|a, b| b.memory.cmp(&a.memory)),
        SortColumn::Uptime => procs.sort_by(|a, b| b.uptime.cmp(&a.uptime)),
        SortColumn::Restarts => procs.sort_by(|a, b| b.restarts.cmp(&a.restarts)),
    }

    if !state.sort_asc && state.sort_by != SortColumn::Name {
        procs.reverse();
    }

    procs
}

// ── Drawing ─────────────────────────────────────────────────────────

fn draw_ui(f: &mut ratatui::Frame, state: &AppState) {
    // Set background
    let bg = Block::default().style(Style::default().bg(cat::BASE));
    f.render_widget(bg, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(6),    // process table
            Constraint::Length(5), // graphs (cpu + mem)
            Constraint::Min(6),    // logs
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    draw_header(f, state, chunks[0]);
    draw_process_table(f, state, chunks[1]);
    draw_graphs(f, state, chunks[2]);
    draw_logs(f, state, chunks[3]);
    draw_footer(f, state, chunks[4]);

    // Overlays
    match state.mode {
        Mode::Detail => draw_detail_panel(f, state),
        Mode::Signal => draw_signal_popup(f, state),
        Mode::Help => draw_help_popup(f),
        _ => {}
    }

    // Toast notifications
    draw_notifications(f, state);
}

fn draw_header(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let total_cpu: f32 = state.processes.iter().map(|p| p.cpu_percent).sum();
    let total_mem: u64 = state.processes.iter().map(|p| p.memory).sum();
    let max_uptime = state.processes.iter().map(|p| p.uptime).max().unwrap_or(0);

    let header = Line::from(vec![
        Span::styled(
            " Velos Monitor ",
            Style::default()
                .fg(cat::LAVENDER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("| ", Style::default().fg(cat::SURFACE2)),
        Span::styled(
            format!("{} ", state.processes.len()),
            Style::default().fg(cat::TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("procs ", Style::default().fg(cat::SUBTEXT0)),
        Span::styled("| ", Style::default().fg(cat::SURFACE2)),
        Span::styled("CPU ", Style::default().fg(cat::SUBTEXT0)),
        Span::styled(
            format!("{:.1}% ", total_cpu),
            Style::default().fg(cat::PEACH).add_modifier(Modifier::BOLD),
        ),
        Span::styled("| ", Style::default().fg(cat::SURFACE2)),
        Span::styled("MEM ", Style::default().fg(cat::SUBTEXT0)),
        Span::styled(
            format!("{} ", format_bytes(total_mem)),
            Style::default().fg(cat::BLUE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("| ", Style::default().fg(cat::SURFACE2)),
        Span::styled(
            format!("up {} ", format_uptime(max_uptime)),
            Style::default().fg(cat::GREEN),
        ),
    ]);
    let widget = Paragraph::new(header).style(Style::default().bg(cat::SURFACE0));
    f.render_widget(widget, area);
}

fn draw_process_table(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let procs = filtered_processes(state);

    let col_label = |col: SortColumn, base: &str| -> String {
        if state.sort_by == col {
            format!("{base} \u{25bc}")
        } else {
            base.to_string()
        }
    };

    let header_cells = [
        ("ID", None),
        (&col_label(SortColumn::Name, "Name"), None),
        ("PID", None),
        ("Status", None),
        (&col_label(SortColumn::Cpu, "CPU%"), Some(cat::PEACH)),
        (&col_label(SortColumn::Mem, "MEM"), Some(cat::BLUE)),
        (&col_label(SortColumn::Uptime, "Uptime"), None),
        (&col_label(SortColumn::Restarts, "Restarts"), None),
    ]
    .iter()
    .map(|(h, color)| {
        Cell::from(h.to_string()).style(
            Style::default()
                .fg(color.unwrap_or(cat::LAVENDER))
                .add_modifier(Modifier::BOLD),
        )
    })
    .collect::<Vec<_>>();
    let header_row = Row::new(header_cells).height(1);

    let rows: Vec<Row> = procs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_selected = i == state.selected;
            let row_bg = if is_selected {
                cat::SURFACE1
            } else if i % 2 == 0 {
                cat::BASE
            } else {
                cat::SURFACE0
            };

            let status_color = match p.status.as_str() {
                "running" => cat::GREEN,
                "errored" => cat::RED,
                "stopped" => cat::YELLOW,
                "starting" => cat::MAUVE,
                _ => cat::TEXT,
            };

            let cpu_bar = make_cpu_bar(p.cpu_percent, 8);

            let sel_marker = if is_selected { "\u{25b8} " } else { "  " };

            Row::new(vec![
                Cell::from(format!("{}{}", sel_marker, p.id)),
                Cell::from(p.name.clone()).style(Style::default().fg(cat::TEXT).add_modifier(
                    if is_selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                )),
                Cell::from(if p.pid > 0 {
                    p.pid.to_string()
                } else {
                    "-".to_string()
                })
                .style(Style::default().fg(cat::SUBTEXT0)),
                Cell::from(p.status.clone()).style(
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(cpu_bar),
                Cell::from(format_bytes(p.memory)).style(Style::default().fg(cat::BLUE)),
                Cell::from(format_uptime(p.uptime)).style(Style::default().fg(cat::SUBTEXT1)),
                Cell::from(p.restarts.to_string()).style(Style::default().fg(if p.restarts > 0 {
                    cat::YELLOW
                } else {
                    cat::SUBTEXT0
                })),
            ])
            .style(Style::default().bg(row_bg))
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),  // ID
            Constraint::Min(12),    // Name
            Constraint::Length(8),  // PID
            Constraint::Length(10), // Status
            Constraint::Length(14), // CPU%
            Constraint::Length(10), // MEM
            Constraint::Length(10), // Uptime
            Constraint::Length(9),  // Restarts
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cat::SURFACE2))
            .title(Span::styled(
                " Processes ",
                Style::default()
                    .fg(cat::LAVENDER)
                    .add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(cat::BASE)),
    );

    f.render_widget(table, area);
}

fn draw_graphs(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let graph_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let procs = filtered_processes(state);
    let selected_name = procs
        .get(state.selected)
        .map(|p| p.name.as_str())
        .unwrap_or("?");
    let selected_idx = procs
        .get(state.selected)
        .and_then(|p| state.processes.iter().position(|sp| sp.id == p.id));

    // CPU sparkline
    let cpu_data: Vec<u64> = selected_idx
        .and_then(|idx| state.cpu_history.get(idx))
        .cloned()
        .unwrap_or_default();
    let cpu_val = procs
        .get(state.selected)
        .map(|p| p.cpu_percent)
        .unwrap_or(0.0);
    let cpu_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(cat::SURFACE2))
                .title(Span::styled(
                    format!(" CPU ({selected_name}) {cpu_val:.1}% "),
                    Style::default().fg(cat::PEACH).add_modifier(Modifier::BOLD),
                ))
                .style(Style::default().bg(cat::BASE)),
        )
        .data(&cpu_data)
        .style(Style::default().fg(cat::PEACH));
    f.render_widget(cpu_spark, graph_chunks[0]);

    // Memory sparkline
    let mem_data: Vec<u64> = selected_idx
        .and_then(|idx| state.mem_history.get(idx))
        .cloned()
        .unwrap_or_default();
    let mem_val = procs.get(state.selected).map(|p| p.memory).unwrap_or(0);
    let mem_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(cat::SURFACE2))
                .title(Span::styled(
                    format!(" Memory ({selected_name}) {} ", format_bytes(mem_val)),
                    Style::default().fg(cat::BLUE).add_modifier(Modifier::BOLD),
                ))
                .style(Style::default().bg(cat::BASE)),
        )
        .data(&mem_data)
        .style(Style::default().fg(cat::BLUE));
    f.render_widget(mem_spark, graph_chunks[1]);
}

fn draw_logs(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let procs = filtered_processes(state);
    let selected_name = procs
        .get(state.selected)
        .map(|p| p.name.as_str())
        .unwrap_or("?");

    let max_lines = area.height.saturating_sub(2) as usize;
    let total = state.logs.len();
    let scroll = state.log_scroll as usize;
    let start = total.saturating_sub(max_lines + scroll);
    let end = total.saturating_sub(scroll);

    let log_lines: Vec<Line> = state.logs[start..end]
        .iter()
        .map(|l| {
            let (tag_color, tag) = if l.stream == "err" {
                (cat::RED, "ERR")
            } else {
                (cat::GREEN, "OUT")
            };
            Line::from(vec![
                Span::styled(
                    format!("[{tag}] "),
                    Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &l.message,
                    Style::default().fg(if l.stream == "err" {
                        cat::RED
                    } else {
                        cat::SUBTEXT1
                    }),
                ),
            ])
        })
        .collect();

    let scroll_indicator = if scroll > 0 {
        format!(" +{scroll} ")
    } else {
        " auto ".to_string()
    };

    let logs_widget = Paragraph::new(log_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cat::SURFACE2))
            .title(vec![
                Span::styled(
                    format!(" Logs ({selected_name}) "),
                    Style::default()
                        .fg(cat::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(scroll_indicator, Style::default().fg(cat::OVERLAY0)),
            ])
            .style(Style::default().bg(cat::BASE)),
    );
    f.render_widget(logs_widget, area);
}

fn draw_footer(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let footer = if state.mode == Mode::Filter {
        Line::from(vec![
            Span::styled(
                " Filter: ",
                Style::default().fg(cat::PEACH).add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.filter_text, Style::default().fg(cat::TEXT)),
            Span::styled("\u{2588}", Style::default().fg(cat::LAVENDER)),
            Span::styled(
                "  (Esc cancel, Enter apply)",
                Style::default().fg(cat::OVERLAY0),
            ),
        ])
    } else {
        let mut spans = Vec::new();
        spans.push(Span::raw(" "));
        let keys: &[(&str, &str)] = &[
            ("q", "quit"),
            ("\u{2191}\u{2193}/jk", "select"),
            ("/", "filter"),
            ("Tab", "sort"),
            ("Enter", "detail"),
            ("r", "restart"),
            ("s", "stop"),
            ("d", "delete"),
            ("K", "signal"),
            ("?", "help"),
        ];
        for (i, (key, desc)) in keys.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " \u{2502} ",
                    Style::default().fg(cat::SURFACE2),
                ));
            }
            spans.push(Span::styled(
                *key,
                Style::default()
                    .fg(cat::LAVENDER)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {desc}"),
                Style::default().fg(cat::OVERLAY0),
            ));
        }
        Line::from(spans)
    };
    let widget = Paragraph::new(footer).style(Style::default().bg(cat::SURFACE0));
    f.render_widget(widget, area);
}

fn draw_detail_panel(f: &mut ratatui::Frame, state: &AppState) {
    let procs = filtered_processes(state);
    let proc = match procs.get(state.selected) {
        Some(p) => p,
        None => return,
    };

    let area = centered_rect(60, 60, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        detail_line("Name", &proc.name, cat::TEXT),
        detail_line("ID", &proc.id.to_string(), cat::TEXT),
        detail_line(
            "PID",
            &if proc.pid > 0 {
                proc.pid.to_string()
            } else {
                "-".into()
            },
            cat::TEXT,
        ),
        detail_line(
            "Status",
            &proc.status,
            match proc.status.as_str() {
                "running" => cat::GREEN,
                "errored" => cat::RED,
                "stopped" => cat::YELLOW,
                _ => cat::TEXT,
            },
        ),
        detail_line("CPU", &format!("{:.1}%", proc.cpu_percent), cat::PEACH),
        detail_line("Memory", &format_bytes(proc.memory), cat::BLUE),
        detail_line("Uptime", &format_uptime(proc.uptime), cat::GREEN),
        detail_line(
            "Restarts",
            &proc.restarts.to_string(),
            if proc.restarts > 0 {
                cat::YELLOW
            } else {
                cat::TEXT
            },
        ),
        Line::from(""),
        Line::from(Span::styled(
            " Press Esc or Enter to close ",
            Style::default().fg(cat::OVERLAY0),
        )),
    ];

    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(cat::LAVENDER))
                .title(Span::styled(
                    format!(" {} ", proc.name),
                    Style::default()
                        .fg(cat::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ))
                .style(Style::default().bg(cat::BASE)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(detail, area);
}

fn draw_signal_popup(f: &mut ratatui::Frame, state: &AppState) {
    let area = centered_rect(30, 30, f.area());
    f.render_widget(Clear, area);

    let items: Vec<Line> = SIGNALS
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            let style = if i == state.signal_selected {
                Style::default()
                    .fg(cat::BASE)
                    .bg(cat::LAVENDER)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(cat::TEXT)
            };
            Line::from(Span::styled(format!("  {sig}  "), style))
        })
        .collect();

    let popup = Paragraph::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cat::PEACH))
            .title(Span::styled(
                " Send Signal ",
                Style::default().fg(cat::PEACH).add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(cat::BASE)),
    );
    f.render_widget(popup, area);
}

fn draw_help_popup(f: &mut ratatui::Frame) {
    let area = centered_rect(50, 70, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        help_line("q / Esc", "Quit"),
        help_line("\u{2191}\u{2193} / j k", "Navigate processes"),
        help_line("/", "Filter by name"),
        help_line("Tab", "Cycle sort column"),
        help_line("Enter", "Process details"),
        help_line("r", "Restart process"),
        help_line("s", "Stop process"),
        help_line("d", "Delete process"),
        help_line("K", "Send signal"),
        help_line("PgUp / PgDn", "Scroll logs"),
        help_line("?", "Toggle help"),
        Line::from(""),
        Line::from(Span::styled(
            " Press Esc to close ",
            Style::default().fg(cat::OVERLAY0),
        )),
    ];

    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cat::MAUVE))
            .title(Span::styled(
                " Keybindings ",
                Style::default().fg(cat::MAUVE).add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(cat::BASE)),
    );
    f.render_widget(popup, area);
}

fn draw_notifications(f: &mut ratatui::Frame, state: &AppState) {
    for (i, notif) in state.notifications.iter().enumerate() {
        let y = f.area().height.saturating_sub(3 + (i as u16 * 3));
        let x = f.area().width.saturating_sub(35);
        let area = Rect::new(x, y, 33, 3);
        if area.y < 2 {
            continue;
        }
        f.render_widget(Clear, area);
        let popup = Paragraph::new(Line::from(vec![
            Span::styled(" \u{26a0} ", Style::default().fg(cat::YELLOW)),
            Span::styled(
                &notif.message,
                Style::default().fg(cat::RED).add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(cat::RED))
                .style(Style::default().bg(cat::SURFACE0)),
        );
        f.render_widget(popup, area);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn make_cpu_bar(percent: f32, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f32).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!(
        "{}{} {:.1}%",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty),
        percent
    )
}

fn detail_line<'a>(label: &'a str, value: &str, color: Color) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {label:<12}"),
            Style::default()
                .fg(cat::OVERLAY0)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {key:<16}"),
            Style::default()
                .fg(cat::LAVENDER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(cat::TEXT)),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.0} KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1} MB");
    }
    let gb = mb / 1024.0;
    format!("{gb:.2} GB")
}

fn format_uptime(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}
