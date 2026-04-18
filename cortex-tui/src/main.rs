use anyhow::Result;
use cortex_core::nats_bus::{CortexBus, TaskStatus};
use cortex_core::permissions::{Permission, PermissionPolicy};
use cortex_core::sandbox::Sandbox;
use cortex_core::tools::ToolRegistry;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use std::io::stdout;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Agents,
    Memory,
    Tools,
    Config,
}

struct App {
    active_tab: Tab,
    logs: Vec<String>,
    input: String,
    should_quit: bool,
    
    // NATS & Engine State
    nats_connected: bool,
    brain_online: bool,
    is_thinking: bool,
    thinking_step: usize,

    // Memory State
    memories: Vec<String>,
    _memory_list_state: ListState,

    // Tools State
    tool_list: Vec<String>,
}

impl App {
    fn new(registry: &ToolRegistry) -> Self {
        Self {
            active_tab: Tab::Agents,
            logs: vec!["[SYSTEM] Cortex OS TUI initialized.".into()],
            input: String::new(),
            should_quit: false,
            nats_connected: false,
            brain_online: false,
            is_thinking: false,
            thinking_step: 0,
            memories: Vec::new(),
            _memory_list_state: ListState::default(),
            tool_list: registry.list().into_iter().map(|s| s.to_string()).collect(),
        }
    }

    fn push_log(&mut self, msg: String) {
        self.logs.push(msg);
        if self.logs.len() > 1000 {
            self.logs.drain(0..100);
        }
    }

    fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Agents => Tab::Memory,
            Tab::Memory => Tab::Tools,
            Tab::Tools => Tab::Config,
            Tab::Config => Tab::Agents,
        };
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    
    // Core engine setup
    let sandbox = Sandbox::default();
    let registry = ToolRegistry::with_defaults(sandbox);
    let _perm_policy = PermissionPolicy::new(Permission::Full, ".");
    
    let mut app = App::new(&registry);
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    // NATS connection (separate thread/task for now)
    let nats_url = "nats://127.0.0.1:4222";
    let bus = CortexBus::connect(nats_url, None).await.ok();
    if let Some(ref b) = bus {
        app.nats_connected = true;
        if let Ok(health) = b.brain_health().await {
            if health.status == TaskStatus::Success {
                app.brain_online = true;
            }
        }
    }

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            let msg = app.input.clone();
                            app.push_log(format!("▸ {msg}"));
                            app.input.clear();
                            
                            // Simulate action
                            if msg.starts_with("search ") {
                               let query = msg.replace("search ", "");
                               if let Some(ref _b) = bus {
                                   app.push_log(format!("[MEMORY] Searching for: {query}"));
                               }
                            }
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
            if app.is_thinking {
                app.thinking_step = (app.thinking_step + 1) % 4;
            }
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
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status
        ])
        .split(f.area());

    // ─── Tabs ───────────────────────────
    let titles = vec!["[A]gents", "[M]emory", "[T]ools", "[C]onfig"];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(match app.active_tab {
            Tab::Agents => 0,
            Tab::Memory => 1,
            Tab::Tools => 2,
            Tab::Config => 3,
        })
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Cyan).bold());
    f.render_widget(tabs, chunks[0]);

    // ─── Content ──────────────────────────
    match app.active_tab {
        Tab::Agents => render_agents(f, chunks[1], app),
        Tab::Memory => render_memory(f, chunks[1], app),
        Tab::Tools => render_tools(f, chunks[1], app),
        Tab::Config => render_config(f, chunks[1], app),
    }

    // ─── Input ───────────────────────────
    let input = Paragraph::new(format!("  cortex > {}_", app.input))
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)));
    f.render_widget(input, chunks[2]);

    // ─── Status ──────────────────────────
    let nats_style = if app.nats_connected { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) };
    let brain_style = if app.brain_online { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) };
    
    let status = Line::from(vec![
        Span::styled(" NATS: ● ", nats_style),
        Span::styled(" Brain: ● ", brain_style),
        Span::raw(" | Tab: "),
        Span::styled(format!("{:?}", app.active_tab), Style::default().bold()),
        Span::raw(" | [TAB] Switch | [Q] Quit"),
    ]);
    f.render_widget(Paragraph::new(status), chunks[3]);
}

fn render_agents(f: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let logs = Paragraph::new(app.logs.join("\n"))
        .wrap(Wrap { trim: false })
        .block(Block::default().title(" Agent Activity ").borders(Borders::ALL));
    f.render_widget(logs, layout[0]);

    let thinking_marker = match app.thinking_step {
        0 => "⠋",
        1 => "⠙",
        2 => "⠹",
        3 => "⠸",
        _ => " ",
    };

    let status_text = if app.is_thinking {
        format!("{} Thinking...", thinking_marker)
    } else {
        "IDLE".to_string()
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().title(" Status ").borders(Borders::ALL));
    f.render_widget(status, layout[1]);
}

fn render_memory(f: &mut Frame, area: Rect, app: &App) {
    let memories: Vec<ListItem> = app.memories
        .iter()
        .map(|m| ListItem::new(m.as_str()))
        .collect();

    let list = List::new(memories)
        .block(Block::default().title(" Memory Palace Explorer ").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    
    f.render_widget(list, area);
}

fn render_tools(f: &mut Frame, area: Rect, app: &App) {
    let tools: Vec<ListItem> = app.tool_list
        .iter()
        .map(|t| ListItem::new(format!("⚙️ {}", t)))
        .collect();

    let list = List::new(tools)
        .block(Block::default().title(" Tool Catalog ").borders(Borders::ALL));
    f.render_widget(list, area);
}

fn render_config(f: &mut Frame, area: Rect, _app: &App) {
    let current_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());
        
    let config = vec![
        format!("OS Version: 0.1.0"),
        format!("Permission Level: Full"),
        format!("Workspace: {}", current_dir),
        format!("NATS URL: nats://127.0.0.1:4222"),
        format!("Default Model: dolphin-mistral:latest"),
    ];
    let text: Vec<ListItem> = config.into_iter().map(|s| ListItem::new(s)).collect();
    let list = List::new(text).block(Block::default().title(" System Configuration ").borders(Borders::ALL));
    f.render_widget(list, area);
}
