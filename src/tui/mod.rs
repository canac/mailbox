mod app;
mod multiselect_list;
mod navigable_list;
mod tree_list;

use self::app::{App, Pane};
use self::multiselect_list::SelectionMode;
use self::navigable_list::NavigableList;
use crate::{database::Database, message::MessageState};
use anyhow::Result;
use chrono::Utc;
use chrono_humanize::HumanTime;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::time::{Duration, Instant};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

pub fn run(
    db: Database,
    initial_mailbox: Option<String>,
    initial_states: Vec<MessageState>,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let tick_rate = Duration::from_millis(250);
    let app = App::new(db, initial_mailbox, initial_states)?;
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
        KeyCode::Char('u') if control => app.toggle_active_state(MessageState::Unread)?,
        KeyCode::Char('r') if control => app.toggle_active_state(MessageState::Read)?,
        KeyCode::Char('a') if control => app.toggle_active_state(MessageState::Archived)?,
        _ => {}
    }

    Ok(())
}

// Respond to keyboard presses for the mailbox pane
fn handle_mailbox_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => {
            app.mailboxes.remove_cursor();
            app.update_messages()?;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if control {
                app.mailboxes.next_sibling();
            } else {
                app.mailboxes.next();
            }
            app.update_messages()?;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if control {
                app.mailboxes.previous_sibling();
            } else {
                app.mailboxes.previous();
            }
            app.update_messages()?;
        }
        _ => {}
    }

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
                .move_cursor_relative(if control { 10 } else { 1 })
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.messages
                .move_cursor_relative(if control { -10 } else { -1 })
        }
        KeyCode::Char('J') => app.messages.last(),
        KeyCode::Char('K') => app.messages.first(),
        KeyCode::Esc => app.messages.remove_cursor(),
        KeyCode::Char(' ') => app.messages.toggle_cursor_selected(),
        KeyCode::Char('u') if !control => app.set_selected_message_states(MessageState::Unread)?,
        KeyCode::Char('r') if !control => app.set_selected_message_states(MessageState::Read)?,
        KeyCode::Char('a') if !control => {
            app.set_selected_message_states(MessageState::Archived)?
        }
        KeyCode::Char('x') if control => app.delete_selected_messages()?,
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

    let active_style = Style::default().fg(Color::Black).bg(Color::Green);
    let inactive_style = Style::default();
    let footer = Paragraph::new(Spans::from(vec![
        Span::raw(" "),
        Span::styled(
            " unread ",
            if app.active_states.contains(&MessageState::Unread) {
                active_style
            } else {
                inactive_style
            },
        ),
        Span::raw(" "),
        Span::styled(
            " read ",
            if app.active_states.contains(&MessageState::Read) {
                active_style
            } else {
                inactive_style
            },
        ),
        Span::raw(" "),
        Span::styled(
            " archived ",
            if app.active_states.contains(&MessageState::Archived) {
                active_style
            } else {
                inactive_style
            },
        ),
        Span::raw("   "),
        Span::styled(
            match app.messages.get_selection_mode() {
                SelectionMode::None => "",
                SelectionMode::Select => "selecting",
                SelectionMode::Deselect => "deselecting",
            },
            Style::default().fg(Color::LightBlue),
        ),
    ]));
    frame.render_widget(footer, chunks[1]);

    let mailboxes = app
        .mailboxes
        .get_items()
        .iter()
        .map(|mailbox| {
            ListItem::new(Span::styled(
                format!(
                    "{}{} ({})",
                    " ".repeat(mailbox.depth),
                    mailbox.name,
                    mailbox.message_count
                ),
                Style::default(),
            ))
        })
        .collect::<Vec<_>>();
    let border_style = match app.active_pane {
        Pane::Mailboxes => Style::default().fg(Color::LightBlue),
        _ => Style::default(),
    };
    let mailboxes_list = List::new(mailboxes)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(
                    "Mailboxes ({}{})",
                    match app.mailboxes.get_cursor() {
                        None => "".to_string(),
                        Some(index) => format!("{}/", index + 1),
                    },
                    app.mailboxes.get_items().len()
                )),
        )
        .highlight_style(
            Style::default()
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        mailboxes_list,
        content_chunks[0],
        app.mailboxes.get_list_state(),
    );

    let messages = app
        .messages
        .iter_items_with_selected()
        .map(|(message, selected)| {
            let active_marker = if selected {
                Span::styled("• ", Style::default().add_modifier(Modifier::BOLD))
            } else {
                Span::raw("  ")
            };
            let state_marker = match message.state {
                MessageState::Unread => Span::styled(
                    "* ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                MessageState::Read => Span::raw("  "),
                MessageState::Archived => Span::raw("- "),
            };
            let timestamp = HumanTime::from(
                message
                    .timestamp
                    .signed_duration_since(Utc::now().naive_utc()),
            )
            .to_string();
            ListItem::new(Spans::from(vec![
                active_marker,
                state_marker,
                Span::raw(message.content.clone()),
                Span::styled(
                    format!(" @ {timestamp}"),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let border_style = match app.active_pane {
        Pane::Messages => Style::default().fg(Color::LightBlue),
        _ => Style::default(),
    };
    let messages_list = List::new(messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(
                    "Messages ({}{})",
                    match app.messages.get_cursor() {
                        None => "".to_string(),
                        Some(index) => format!("{}/", index + 1),
                    },
                    app.messages.get_items().len()
                )),
        )
        .highlight_style(
            Style::default()
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(
        messages_list,
        content_chunks[1],
        app.messages.get_list_state(),
    );
}
