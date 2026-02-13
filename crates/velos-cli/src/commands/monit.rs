use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table};
use ratatui::Terminal;

use velos_core::VelosError;

struct AppState {
    processes: Vec<ProcessRow>,
    selected: usize,
    logs: Vec<String>,
    should_quit: bool,
    mem_history: Vec<Vec<u64>>,
}

struct ProcessRow {
    id: u32,
    name: String,
    pid: u32,
    status: String,
    memory: u64,
    uptime: u64,
    restarts: u32,
}

pub async fn run() -> Result<(), VelosError> {
    // Initial data fetch
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
                uptime: p.uptime_ms,
                restarts: p.restart_count,
            })
            .collect(),
        selected: 0,
        logs: Vec::new(),
        should_quit: false,
        mem_history: Vec::new(),
    };

    // Setup terminal
    enable_raw_mode().map_err(|e| VelosError::ProtocolError(format!("terminal: {e}")))?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen).map_err(|e| VelosError::ProtocolError(format!("terminal: {e}")))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| VelosError::ProtocolError(format!("terminal: {e}")))?;

    let result = run_loop(&mut terminal, &mut state).await;

    // Restore terminal
    disable_raw_mode().ok();
    io::stdout().execute(LeaveAlternateScreen).ok();

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<(), VelosError> {
    loop {
        // Draw
        terminal
            .draw(|f| draw_ui(f, state))
            .map_err(|e| VelosError::ProtocolError(format!("draw: {e}")))?;

        // Handle input (non-blocking, 2s timeout for refresh)
        if event::poll(Duration::from_secs(2)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        state.should_quit = true;
                    }
                    KeyCode::Up => {
                        if state.selected > 0 {
                            state.selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if state.selected + 1 < state.processes.len() {
                            state.selected += 1;
                        }
                    }
                    KeyCode::Char('r') => {
                        // Restart selected
                        if let Some(proc) = state.processes.get(state.selected) {
                            let mut client = super::connect().await.ok();
                            if let Some(ref mut c) = client {
                                let _ = c.restart(proc.id).await;
                            }
                        }
                    }
                    KeyCode::Char('s') => {
                        // Stop selected
                        if let Some(proc) = state.processes.get(state.selected) {
                            let mut client = super::connect().await.ok();
                            if let Some(ref mut c) = client {
                                let _ = c.stop(proc.id).await;
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        // Delete selected
                        if let Some(proc) = state.processes.get(state.selected) {
                            let mut client = super::connect().await.ok();
                            if let Some(ref mut c) = client {
                                let _ = c.delete(proc.id).await;
                            }
                        }
                    }
                    KeyCode::Char('l') => {
                        // Fetch logs for selected
                        if let Some(proc) = state.processes.get(state.selected) {
                            let mut client = super::connect().await.ok();
                            if let Some(ref mut c) = client {
                                if let Ok(entries) = c.logs(proc.id, 20).await {
                                    state.logs = entries
                                        .iter()
                                        .map(|e| {
                                            let tag = if e.stream == 1 { "err" } else { "out" };
                                            format!("[{}] {}", tag, e.message)
                                        })
                                        .collect();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if state.should_quit {
            return Ok(());
        }

        // Refresh data
        if let Ok(mut client) = super::connect().await {
            if let Ok(procs) = client.list().await {
                state.processes = procs
                    .iter()
                    .map(|p| ProcessRow {
                        id: p.id,
                        name: p.name.clone(),
                        pid: p.pid,
                        status: p.status_str().to_string(),
                        memory: p.memory_bytes,
                        uptime: p.uptime_ms,
                        restarts: p.restart_count,
                    })
                    .collect();
                if state.selected >= state.processes.len() && !state.processes.is_empty() {
                    state.selected = state.processes.len() - 1;
                }

                // Record memory history
                if state.mem_history.len() != state.processes.len() {
                    state.mem_history.resize(state.processes.len(), Vec::new());
                }
                for (i, p) in state.processes.iter().enumerate() {
                    if i < state.mem_history.len() {
                        state.mem_history[i].push(p.memory / 1024); // KB for sparkline
                        // Keep last 60 samples (2 minutes at 2s interval)
                        if state.mem_history[i].len() > 60 {
                            state.mem_history[i].remove(0);
                        }
                    }
                }
            }
        }
    }
}

fn draw_ui(f: &mut ratatui::Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(8),    // process table
            Constraint::Length(3), // memory bar
            Constraint::Min(5),    // logs
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    // Header
    let total_mem: u64 = state.processes.iter().map(|p| p.memory).sum();
    let header_text = format!(
        " Velos Monitor | Processes: {} | Total Memory: {}",
        state.processes.len(),
        format_bytes(total_mem)
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Process table
    let selected_style = Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD);
    let header_cells = ["ID", "Name", "PID", "Status", "Memory", "Uptime", "Restarts"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header_row = Row::new(header_cells).height(1);

    let rows: Vec<Row> = state
        .processes
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == state.selected {
                selected_style
            } else {
                Style::default()
            };
            let status_color = match p.status.as_str() {
                "running" => Color::Green,
                "stopped" => Color::Red,
                "errored" => Color::Red,
                "starting" => Color::Yellow,
                _ => Color::White,
            };
            Row::new(vec![
                Cell::from(p.id.to_string()),
                Cell::from(p.name.clone()),
                Cell::from(p.pid.to_string()),
                Cell::from(p.status.clone()).style(Style::default().fg(status_color)),
                Cell::from(format_bytes(p.memory)),
                Cell::from(format_uptime(p.uptime)),
                Cell::from(p.restarts.to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(15),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(9),
        ],
    )
    .header(header_row)
    .block(Block::default().borders(Borders::ALL).title("Processes"));

    f.render_widget(table, chunks[1]);

    // Memory sparkline for selected process
    let sparkline_data: Vec<u64> = if let Some(hist) = state.mem_history.get(state.selected) {
        hist.clone()
    } else {
        Vec::new()
    };
    let spark_title = if let Some(p) = state.processes.get(state.selected) {
        format!("Memory: {} ({})", p.name, format_bytes(p.memory))
    } else {
        "Memory".to_string()
    };
    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(spark_title))
        .data(&sparkline_data)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(sparkline, chunks[2]);

    // Logs
    let log_lines: Vec<Line> = state
        .logs
        .iter()
        .rev()
        .take(chunks[3].height as usize)
        .rev()
        .map(|l| {
            let color = if l.contains("[err]") {
                Color::Red
            } else {
                Color::White
            };
            Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
        })
        .collect();
    let log_title = if let Some(p) = state.processes.get(state.selected) {
        format!("Logs ({})", p.name)
    } else {
        "Logs".to_string()
    };
    let logs_widget = Paragraph::new(log_lines)
        .block(Block::default().borders(Borders::ALL).title(log_title));
    f.render_widget(logs_widget, chunks[3]);

    // Footer
    let footer = Paragraph::new(" [q]uit  [\u{2191}\u{2193}]select  [l]ogs  [r]estart  [s]top  [d]elete")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[4]);
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{:.0} KB", kb);
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{:.1} MB", mb);
    }
    let gb = mb / 1024.0;
    format!("{:.2} GB", gb)
}

fn format_uptime(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}
