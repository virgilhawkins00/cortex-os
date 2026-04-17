use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::io::stdout;
use std::time::{Duration, Instant};

struct App {
    logs: Vec<String>,
    input: String,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            logs: vec![
                "╔══════════════════════════════════════════╗".into(),
                "║       CORTEX OS v0.1.0 — ONLINE         ║".into(),
                "╚══════════════════════════════════════════╝".into(),
                String::new(),
                "System initialized. Waiting for tasks...".into(),
                "Press Ctrl+C to exit.".into(),
            ],
            input: String::new(),
            should_quit: false,
        }
    }

    fn push_log(&mut self, msg: String) {
        self.logs.push(msg);
        // Keep last 500 lines
        if self.logs.len() > 500 {
            self.logs.drain(0..100);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            let cmd = app.input.clone();
                            app.push_log(format!("▸ {cmd}"));
                            app.push_log("  [queued for execution]".into());
                            app.input.clear();
                        }
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),   // Logs
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // ─── Header ──────────────────────────
    let header = Paragraph::new("  🧠 CORTEX OS  —  Autonomous AI Runtime")
        .style(Style::default().fg(Color::Cyan).bold())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(header, chunks[0]);

    // ─── Logs ────────────────────────────
    let log_text = app.logs.join("\n");
    let logs = Paragraph::new(log_text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(" Activity Log ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(logs, chunks[1]);

    // ─── Input ───────────────────────────
    let input = Paragraph::new(format!("  cortex > {}_", app.input))
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .title(" Command ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(input, chunks[2]);

    // ─── Status bar ──────────────────────
    let status = Paragraph::new(" NATS: ● | Ollama: ○ | Memory: ○ | Tools: 3 loaded")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[3]);
}
