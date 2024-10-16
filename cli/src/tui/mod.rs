mod app;
mod monotonic_counter;
mod multiselect_list;
mod navigable_list;
mod tree_list;
mod worker;

use self::app::{App, Pane};
use self::multiselect_list::SelectionMode;
use self::navigable_list::NavigableList;
use anyhow::Result;
use chrono::Utc;
use chrono_humanize::HumanTime;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use database::{Backend as DbBackend, Database, Mailbox, Message, State};
use linkify::{LinkFinder, LinkKind};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};

pub async fn run<B: DbBackend + Send + Sync + 'static>(
    db: Database<B>,
    initial_mailbox: Option<Mailbox>,
    initial_states: Vec<State>,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let tick_rate = Duration::from_millis(30);
    let app = App::new(db, initial_mailbox, initial_states).await?;
    let res = run_app(&mut terminal, app, tick_rate);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        app.handle_worker_responses()?;
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    return Ok(());
                }

                handle_global_key(&mut app, key)?;
                match app.active_pane {
                    Pane::Mailboxes => handle_mailbox_key(&mut app, key)?,
                    Pane::Messages => handle_message_key(&mut app, key)?,
                };
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

// Respond to keyboard presses for all panes
fn handle_global_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('1') => app.activate_pane(Pane::Mailboxes),
        KeyCode::Char('2') => app.activate_pane(Pane::Messages),
        KeyCode::Char('R') => {
            app.update_mailboxes()?;
            app.update_messages()?;
        }
        KeyCode::Char('u') if control => app.toggle_active_state(State::Unread)?,
        KeyCode::Char('r') if control => app.toggle_active_state(State::Read)?,
        KeyCode::Char('a') if control => app.toggle_active_state(State::Archived)?,
        _ => {}
    }

    Ok(())
}

// Respond to keyboard presses for the mailbox pane
fn handle_mailbox_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let old_active_mailbox = app
        .mailboxes
        .get_cursor_item()
        .map(|item| item.mailbox.clone());
    match key.code {
        KeyCode::Esc => {
            app.mailboxes.remove_cursor();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if control {
                app.mailboxes.next_sibling();
            } else {
                app.mailboxes.next();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if control {
                app.mailboxes.previous_sibling();
            } else {
                app.mailboxes.previous();
            }
        }
        KeyCode::Char('K') => {
            app.mailboxes.parent();
        }
        KeyCode::Char('a') => {
            if let Some(active_mailbox) = old_active_mailbox {
                app.set_mailbox_message_state(active_mailbox, State::Archived)?;
            }
            return Ok(());
        }
        KeyCode::Char('r') => {
            if let Some(active_mailbox) = old_active_mailbox {
                app.set_mailbox_message_state(active_mailbox, State::Read)?;
            }
            return Ok(());
        }
        KeyCode::Char('u') => {
            if let Some(active_mailbox) = old_active_mailbox {
                app.set_mailbox_message_state(active_mailbox, State::Unread)?;
            }
            return Ok(());
        }
        _ => return Ok(()),
    }

    let active_mailbox = app.mailboxes.get_cursor_item().map(|item| &item.mailbox);
    if active_mailbox == old_active_mailbox.as_ref() {
        return Ok(());
    }

    if let Some(active_mailbox) = active_mailbox {
        // If the new active mailbox is a descendant of the old one or if there wasn't an old active mailbox, the
        // messages list can be optimistically updated by filtering against the new active mailbox instead of needing
        // to refresh the whole list
        let local_update = old_active_mailbox.map_or(true, |old_active_mailbox| {
            old_active_mailbox.is_ancestor_of(active_mailbox)
        });

        if local_update {
            // Optimistically update the messages list
            app.filter_messages();
            return Ok(());
        }
    }

    // Update the mailboxes in case updating the messages list loads new messages that change the mailbox counts
    app.update_mailboxes()?;
    app.update_messages()?;

    Ok(())
}

// Respond to keyboard presses for the messages pane
fn handle_message_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if control => app.messages.set_selection_mode(
            if matches!(app.messages.get_selection_mode(), SelectionMode::Select) {
                SelectionMode::None
            } else {
                SelectionMode::Select
            },
        ),
        KeyCode::Char('d') if control => app.messages.set_selection_mode(
            if matches!(app.messages.get_selection_mode(), SelectionMode::Deselect) {
                SelectionMode::None
            } else {
                SelectionMode::Deselect
            },
        ),
        KeyCode::Char('g') => app.messages.set_all_selected(true),
        KeyCode::Char('G') => app.messages.set_all_selected(false),
        KeyCode::Down | KeyCode::Char('j') => {
            app.messages
                .move_cursor_relative(if control { 10 } else { 1 });
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.messages
                .move_cursor_relative(if control { -10 } else { -1 });
        }
        KeyCode::Char('J') => app.messages.last(),
        KeyCode::Char('K') => app.messages.first(),
        KeyCode::Esc => app.messages.remove_cursor(),
        KeyCode::Char(' ') => app.messages.toggle_cursor_selected(),
        KeyCode::Char('u') if !control => app.set_selected_message_states(State::Unread)?,
        KeyCode::Char('r') if !control => app.set_selected_message_states(State::Read)?,
        KeyCode::Char('a') if !control => {
            app.set_selected_message_states(State::Archived)?;
        }
        KeyCode::Char('x') if control => app.delete_selected_messages()?,
        KeyCode::Enter => {
            if let Some(message) = app.messages.get_cursor_item() {
                open_message(message);
            }
        }
        _ => {}
    }

    Ok(())
}

fn ui<B: Backend>(frame: &mut Frame<B>, app: &mut App) {
    // Create the content and footer chunks
    let frame_size = frame.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(frame_size.height.saturating_sub(1)),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(frame_size);

    // Create the mailbox and message chunks
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)].as_ref())
        .split(chunks[0]);

    render_footer(frame, app, chunks[1]);
    render_mailboxes(frame, app, content_chunks[0]);
    render_messages(frame, app, content_chunks[1]);
}

// Render the footer section of the UI
fn render_footer<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    const ACTIVE_STYLE: Style = Style::new().fg(Color::Black).bg(Color::Green);
    const INACTIVE_STYLE: Style = Style::new();
    const SELECTING_STYLE: Style = Style::new().fg(Color::LightBlue);
    let footer = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            " unread ",
            if app.active_states.contains(&State::Unread) {
                ACTIVE_STYLE
            } else {
                INACTIVE_STYLE
            },
        ),
        Span::raw(" "),
        Span::styled(
            " read ",
            if app.active_states.contains(&State::Read) {
                ACTIVE_STYLE
            } else {
                INACTIVE_STYLE
            },
        ),
        Span::raw(" "),
        Span::styled(
            " archived ",
            if app.active_states.contains(&State::Archived) {
                ACTIVE_STYLE
            } else {
                INACTIVE_STYLE
            },
        ),
        Span::raw("   "),
        Span::styled(
            match app.messages.get_selection_mode() {
                SelectionMode::None => "",
                SelectionMode::Select => "selecting",
                SelectionMode::Deselect => "deselecting",
            },
            SELECTING_STYLE,
        ),
    ]));
    frame.render_widget(footer, area);
}

// Render the mailboxes section of the UI
fn render_mailboxes<B: Backend>(frame: &mut Frame<B>, app: &mut App, area: Rect) {
    const MAILBOX_STYLE: Style = Style::new();
    const MAILBOX_BORDER_STYLE: Style = Style::new().fg(Color::LightBlue);
    const MESSAGE_BORDER_STYLE: Style = Style::new();
    const MAILBOX_HIGHLIGHT_STYLE: Style = Style::new()
        .bg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    let mailboxes = app
        .mailboxes
        .get_items()
        .iter()
        .map(|mailbox| {
            ListItem::new(Span::styled(
                format!(
                    "{}{} ({})",
                    " ".repeat(mailbox.depth),
                    mailbox.mailbox.get_leaf_name(),
                    mailbox.message_count
                ),
                MAILBOX_STYLE,
            ))
        })
        .collect::<Vec<_>>();
    let border_style = match app.active_pane {
        Pane::Mailboxes => MAILBOX_BORDER_STYLE,
        Pane::Messages => MESSAGE_BORDER_STYLE,
    };
    let mailboxes_list = List::new(mailboxes)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(
                    "Mailboxes ({}{})",
                    app.mailboxes
                        .get_cursor()
                        .map_or_else(String::new, |index| format!("{}/", index + 1)),
                    app.mailboxes.get_items().len()
                )),
        )
        .highlight_style(MAILBOX_HIGHLIGHT_STYLE);
    frame.render_stateful_widget(mailboxes_list, area, app.mailboxes.get_list_state());
}

// Render the messages section of the UI
fn render_messages<B: Backend>(frame: &mut Frame<B>, app: &mut App, area: Rect) {
    const BULLET_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
    const UNREAD_STYLE: Style = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
    const TIMESTAMP_STYLE: Style = Style::new().fg(Color::Yellow);
    const MESSAGE_BORDER_STYLE: Style = Style::new().fg(Color::LightBlue);
    const MAILBOX_BORDER_STYLE: Style = Style::new();
    const HIGHLIGHT_STYLE: Style = Style::new()
        .bg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    let messages = app
        .messages
        .iter_items_with_selected()
        .map(|(message, selected)| {
            let active_marker = if selected {
                Span::styled("• ", BULLET_STYLE)
            } else {
                Span::raw("  ")
            };
            let state_marker = match message.state {
                State::Unread => Span::styled("* ", UNREAD_STYLE),
                State::Read => Span::raw("  "),
                State::Archived => Span::raw("- "),
            };
            let timestamp = HumanTime::from(
                message
                    .timestamp
                    .signed_duration_since(Utc::now().naive_utc()),
            )
            .to_string();
            ListItem::new(Line::from(vec![
                active_marker,
                state_marker,
                Span::raw(message.content.clone()),
                Span::styled(format!(" @ {timestamp}"), TIMESTAMP_STYLE),
            ]))
        })
        .collect::<Vec<_>>();
    let border_style = match app.active_pane {
        Pane::Messages => MESSAGE_BORDER_STYLE,
        Pane::Mailboxes => MAILBOX_BORDER_STYLE,
    };
    let messages_list = List::new(messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(
                    "Messages ({}{})",
                    app.messages
                        .get_cursor()
                        .map_or_else(String::new, |index| format!("{}/", index + 1)),
                    app.messages.get_items().len()
                )),
        )
        .highlight_style(HIGHLIGHT_STYLE);
    frame.render_stateful_widget(messages_list, area, app.messages.get_list_state());
}

// If the message contains a URL, open it in a web browser
fn open_message(message: &Message) {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);

    if let Some(link) = finder.links(&message.content).next() {
        // Silently ignore errors if the URL couldn't be opened
        let _ = webbrowser::open(link.as_str());
    }
}
