use std::error::Error;
use std::io;
use std::sync::Arc;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event as CtEvent, EventStream, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use langrust::Message;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::core::agent::{Agent, AgentEvent};
use crate::core::model_picker::BoxedModel;
use crate::core::{build_choice, default_choice_index, default_model_label, model_choices};

/// A single entry rendered in the conversation log.
enum LogEntry {
    /// Plain informational message (e.g. startup banner).
    Info(String),
    /// Message the user typed.
    User(String),
    /// Assistant text (accumulated from streaming deltas).
    Assistant(String),
    ToolCall {
        name: String,
        args: String,
    },
    ToolResult(String),
    ToolError(String),
    StreamError(String),
}

#[derive(PartialEq, Eq)]
enum Mode {
    Normal,
    ModelPicker,
}

struct ModelPickerState {
    list_state: ListState,
}

impl ModelPickerState {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(default_choice_index()));
        Self { list_state }
    }
}

struct App {
    log: Vec<LogEntry>,
    input: String,
    /// Current scroll offset, measured in *wrapped* visual lines from the top
    /// of the log.
    scroll: u16,
    /// When true, the view snaps to the bottom as new content arrives. This is
    /// flipped off when the user scrolls up and re-enabled once they scroll
    /// back to the bottom (or press End).
    stick_to_bottom: bool,
    /// Cached metrics from the previous render so scroll-clamping and Page
    /// keys can operate without re-measuring.
    last_total_lines: u16,
    last_viewport_height: u16,
    mode: Mode,
    picker: ModelPickerState,
    /// True while the agent is processing a user message.
    busy: bool,
    should_quit: bool,
    pending_model_switch: Option<(BoxedModel, String)>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            log: Vec::new(),
            input: String::new(),
            scroll: 0,
            stick_to_bottom: true,
            last_total_lines: 0,
            last_viewport_height: 0,
            mode: Mode::Normal,
            picker: ModelPickerState::new(),
            busy: false,
            should_quit: false,
            pending_model_switch: None,
        };
        app.log.push(LogEntry::Info(format!(
            "Using default model: {}",
            default_model_label()
        )));
        app.log.push(LogEntry::Info(
            "Type /model to switch providers/models. /exit or Ctrl+C to quit.".into(),
        ));
        app
    }

    fn push_log(&mut self, entry: LogEntry) {
        self.log.push(entry);
    }

    /// Append assistant text delta, merging with the previous entry when it
    /// is already an in-progress assistant message.
    fn append_assistant_delta(&mut self, text: &str) {
        if let Some(LogEntry::Assistant(s)) = self.log.last_mut() {
            s.push_str(text);
        } else {
            self.log.push(LogEntry::Assistant(text.to_string()));
        }
    }
}

pub async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_inner(&mut terminal).await;

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

async fn run_inner<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
) -> Result<(), Box<dyn Error + Send + Sync>>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    use crate::core::system_prompt::get_system_prompt;
    use crate::tools::get_filesystem_registry;

    let registry = get_filesystem_registry();
    let sp = get_system_prompt(&registry);
    let agent = Arc::new(Mutex::new(Agent::new(sp, registry)?));

    let mut app = App::new();

    let mut key_events = EventStream::new();

    // Channel for agent events — recreated per user turn.
    let (mut agent_tx, mut agent_rx): (UnboundedSender<AgentEvent>, UnboundedReceiver<AgentEvent>) =
        mpsc::unbounded_channel();
    // Channel used to notify the UI when an agent task finishes.
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<Result<(), String>>();

    terminal.draw(|f| ui(f, &mut app))?;

    while !app.should_quit {
        tokio::select! {
            maybe_key = key_events.next() => {
                match maybe_key {
                    Some(Ok(ev)) => handle_term_event(ev, &mut app, &agent, &mut agent_tx, &mut agent_rx, &done_tx).await,
                    Some(Err(e)) => {
                        app.push_log(LogEntry::StreamError(format!("terminal error: {}", e)));
                    }
                    None => break,
                }
            }
            Some(ev) = agent_rx.recv() => {
                apply_agent_event(&mut app, ev);
            }
            Some(done) = done_rx.recv() => {
                app.busy = false;
                if let Err(e) = done {
                    app.push_log(LogEntry::StreamError(e));
                }
            }
        }

        terminal.draw(|f| ui(f, &mut app))?;
    }

    Ok(())
}

async fn handle_term_event(
    ev: CtEvent,
    app: &mut App,
    agent: &Arc<Mutex<Agent>>,
    agent_tx: &mut UnboundedSender<AgentEvent>,
    agent_rx: &mut UnboundedReceiver<AgentEvent>,
    done_tx: &UnboundedSender<Result<(), String>>,
) {
    match ev {
        CtEvent::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return;
            }
            // Global: Ctrl+C quits.
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                app.should_quit = true;
                return;
            }
            match app.mode {
                Mode::ModelPicker => handle_picker_key(key, app),
                Mode::Normal => {
                    handle_normal_key(key, app, agent, agent_tx, agent_rx, done_tx).await
                }
            }
        }
        CtEvent::Mouse(m) => handle_mouse(m, app),
        _ => {}
    }
}

fn handle_mouse(m: MouseEvent, app: &mut App) {
    match m.kind {
        MouseEventKind::ScrollUp => scroll_by(app, -3),
        MouseEventKind::ScrollDown => scroll_by(app, 3),
        _ => {}
    }
}

/// Adjust the scroll offset by `delta` wrapped lines, updating the
/// stick-to-bottom flag based on whether the user ends up at the bottom.
fn scroll_by(app: &mut App, delta: i32) {
    let max_scroll = app
        .last_total_lines
        .saturating_sub(app.last_viewport_height);
    let current = app.scroll as i32;
    let new = (current + delta).clamp(0, max_scroll as i32) as u16;
    app.scroll = new;
    app.stick_to_bottom = new >= max_scroll;
}

fn handle_picker_key(key: KeyEvent, app: &mut App) {
    let choices = model_choices();
    let len = choices.len();
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Up => {
            let i = app.picker.list_state.selected().unwrap_or(0);
            let new = if i == 0 { len - 1 } else { i - 1 };
            app.picker.list_state.select(Some(new));
        }
        KeyCode::Down => {
            let i = app.picker.list_state.selected().unwrap_or(0);
            let new = (i + 1) % len;
            app.picker.list_state.select(Some(new));
        }
        KeyCode::Enter => {
            let i = app
                .picker
                .list_state
                .selected()
                .unwrap_or(default_choice_index());
            match build_choice(i) {
                Ok(model) => {
                    // We need to set the model on the agent. This is tricky
                    // because we only have a reference to the App here. We
                    // pend this state change by storing it and handling it
                    // in the caller — but simpler: the caller has the Arc.
                    // So we defer via a side channel. Instead, we use a
                    // dedicated app field.
                    app.pending_model_switch = Some((model, choices[i].label.to_string()));
                    app.mode = Mode::Normal;
                }
                Err(e) => {
                    app.log.push(LogEntry::StreamError(format!(
                        "Failed to build model: {}",
                        e
                    )));
                    app.mode = Mode::Normal;
                }
            }
        }
        _ => {}
    }
}

async fn handle_normal_key(
    key: KeyEvent,
    app: &mut App,
    agent: &Arc<Mutex<Agent>>,
    agent_tx: &mut UnboundedSender<AgentEvent>,
    agent_rx: &mut UnboundedReceiver<AgentEvent>,
    done_tx: &UnboundedSender<Result<(), String>>,
) {
    // Apply any pending model switch first.
    if let Some((model, label)) = app.pending_model_switch.take() {
        agent.lock().await.set_model(model);
        app.push_log(LogEntry::Info(format!("Switched model: {}", label)));
    }

    match key.code {
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.busy {
                app.input.push(c);
            }
        }
        KeyCode::Backspace => {
            if !app.busy {
                app.input.pop();
            }
        }
        KeyCode::Up => scroll_by(app, -1),
        KeyCode::Down => scroll_by(app, 1),
        KeyCode::PageUp => scroll_by(app, -(app.last_viewport_height.max(1) as i32)),
        KeyCode::PageDown => scroll_by(app, app.last_viewport_height.max(1) as i32),
        KeyCode::Home => {
            app.scroll = 0;
            app.stick_to_bottom = false;
        }
        KeyCode::End => {
            app.stick_to_bottom = true;
        }
        KeyCode::Enter => {
            if app.busy {
                return;
            }
            let trimmed = app.input.trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            let input = std::mem::take(&mut app.input);

            if trimmed == "/exit" {
                app.should_quit = true;
                return;
            }
            if trimmed == "/model" {
                app.mode = Mode::ModelPicker;
                app.picker.list_state.select(Some(default_choice_index()));
                return;
            }

            app.push_log(LogEntry::User(input.clone()));
            app.stick_to_bottom = true;

            // Fresh channel for this turn.
            let (tx, rx) = mpsc::unbounded_channel::<AgentEvent>();
            *agent_tx = tx.clone();
            *agent_rx = rx;

            let agent_clone = agent.clone();
            let done_tx = done_tx.clone();
            app.busy = true;
            tokio::spawn(async move {
                let mut guard = agent_clone.lock().await;
                let result = guard
                    .send_user_message(Message::user(input), tx)
                    .await
                    .map_err(|e| e.to_string());
                let _ = done_tx.send(result);
            });
        }
        _ => {}
    }
}

fn apply_agent_event(app: &mut App, ev: AgentEvent) {
    match ev {
        AgentEvent::TextDelta(t) => app.append_assistant_delta(&t),
        AgentEvent::TextDone(()) => {
            // End the current assistant block by starting a new one next time.
            // We do this implicitly: append_assistant_delta merges only when
            // the last entry is Assistant. Pushing a marker would double the
            // work; instead, insert a zero-width separator by pushing Info("")
            // only if needed. Simpler: push an empty Assistant entry? Leave
            // as-is; consecutive text blocks are rare between tool calls.
            app.log.push(LogEntry::Info(String::new()));
        }
        AgentEvent::ToolCall { name, args } => {
            let args_str = serde_json::to_string(&args).unwrap_or_default();
            app.push_log(LogEntry::ToolCall {
                name,
                args: args_str,
            });
        }
        AgentEvent::ToolResult { result } => {
            let result_str =
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
            let truncated = if result_str.len() > 200 {
                format!("{}...", &result_str[..200])
            } else {
                result_str
            };
            app.push_log(LogEntry::ToolResult(truncated));
        }
        AgentEvent::ToolError { error } => {
            app.push_log(LogEntry::ToolError(error));
        }
        AgentEvent::StreamError(e) => {
            app.push_log(LogEntry::StreamError(e));
        }
    }
}

// ----- rendering -----

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(f.area());

    render_log(f, chunks[0], app);
    render_input(f, chunks[1], app);

    if app.mode == Mode::ModelPicker {
        render_model_picker(f, f.area(), app);
    }
}

fn render_log(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let lines = log_lines(&app.log);

    let title = if app.busy {
        " fabrica — thinking... "
    } else {
        " fabrica "
    };
    let mut title = title.to_string();
    if !app.stick_to_bottom {
        title.push_str("[scrolled — End to jump to bottom] ");
    }

    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    let viewport_height = inner.height;

    // Measure with full inner width. If content overflows we reserve one
    // column for the scrollbar and remeasure, since that re-wraps text.
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let mut total = paragraph.line_count(inner.width) as u16;
    let needs_scrollbar = total > viewport_height;

    let text_area = if needs_scrollbar && inner.width > 1 {
        let ta = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width - 1,
            height: inner.height,
        };
        total = paragraph.line_count(ta.width) as u16;
        ta
    } else {
        inner
    };

    let max_scroll = total.saturating_sub(viewport_height);
    if app.stick_to_bottom || app.scroll > max_scroll {
        app.scroll = max_scroll;
    }
    app.last_total_lines = total;
    app.last_viewport_height = viewport_height;

    f.render_widget(block, area);
    f.render_widget(paragraph.scroll((app.scroll, 0)), text_area);

    if needs_scrollbar {
        let scrollbar_area = Rect {
            x: inner.x + inner.width - 1,
            y: inner.y,
            width: 1,
            height: inner.height,
        };
        let mut sb_state = ScrollbarState::new(max_scroll as usize).position(app.scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        f.render_stateful_widget(scrollbar, scrollbar_area, &mut sb_state);
    }
}

fn log_lines(log: &[LogEntry]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for entry in log {
        match entry {
            LogEntry::Info(s) => {
                if s.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(Span::styled(
                        s.clone(),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            LogEntry::User(s) => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "> ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(s.clone()),
                ]));
            }
            LogEntry::Assistant(s) => {
                // Split on newlines so wrapping works per line.
                for line in s.split('\n') {
                    lines.push(Line::from(Span::raw(line.to_string())));
                }
            }
            LogEntry::ToolCall { name, args } => {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("[tool: {}]", name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("[args: {}]", args),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
            LogEntry::ToolResult(s) => {
                lines.push(Line::from(Span::styled(
                    format!("[result: {}]", s),
                    Style::default().fg(Color::Blue),
                )));
            }
            LogEntry::ToolError(s) => {
                lines.push(Line::from(Span::styled(
                    format!("[error: {}]", s),
                    Style::default().fg(Color::Red),
                )));
            }
            LogEntry::StreamError(s) => {
                lines.push(Line::from(Span::styled(
                    format!("Stream error: {}", s),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }
        }
    }
    lines
}

fn render_input(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let (title, style) = if app.busy {
        (
            " (working — input disabled) ",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (" input (Enter to send) ", Style::default())
    };
    let input = Paragraph::new(app.input.as_str())
        .style(style)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(input, area);

    if !app.busy {
        // Place cursor at end of input text.
        let x = area.x + 1 + app.input.chars().count() as u16;
        let y = area.y + 1;
        f.set_cursor_position((x.min(area.x + area.width.saturating_sub(2)), y));
    }
}

fn render_model_picker(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let choices = model_choices();
    // Center a box.
    let width = 60.min(area.width.saturating_sub(4));
    let height = (choices.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);

    f.render_widget(Clear, rect);

    let default_idx = default_choice_index();
    let items: Vec<ListItem> = choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let marker = if i == default_idx { " (default)" } else { "" };
            ListItem::new(format!("{}{}", c.label, marker))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" select model (↑/↓, Enter to choose, Esc to cancel) "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, rect, &mut app.picker.list_state);
}
