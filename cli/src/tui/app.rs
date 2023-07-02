use super::multiselect_list::MultiselectList;
use super::navigable_list::{Keyed, NavigableList};
use super::tree_list::{Depth, TreeList};
use super::worker::{spawn, Receiver, Request, Response, Sender};
use anyhow::Result;
use database::{Database, Message, MessageFilter, State};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync::Arc;

pub enum Pane {
    Mailboxes,
    Messages,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Mailbox {
    // The name of the mailbox, including its parents
    pub full_name: String,

    // The name of the mailbox, excluding its parents
    pub name: String,

    // Root mailbox = 0, child mailbox = 1, grandchild mailbox = 2
    pub depth: usize,

    // The number of messages in the mailbox and it's children
    pub message_count: usize,
}

impl Depth for Mailbox {
    fn get_depth(&self) -> usize {
        self.depth
    }
}

impl Keyed for Mailbox {
    fn get_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write(self.full_name.as_bytes());
        hasher.finish()
    }
}

impl Keyed for Message {
    fn get_key(&self) -> u64 {
        self.id as u64
    }
}

pub struct App {
    pub(crate) mailboxes: TreeList<Mailbox>,
    pub(crate) messages: MultiselectList<Message>,
    pub(crate) active_pane: Pane,
    pub(crate) active_states: HashSet<State>,
    worker_tx: Sender,
    worker_rx: Receiver,
}

impl App {
    pub async fn new(
        db: Database,
        initial_mailbox: Option<String>,
        initial_states: Vec<State>,
    ) -> Result<App> {
        let db = Arc::new(db);
        let (worker_tx, worker_rx) = spawn(db.clone());
        let mut app = App {
            active_pane: Pane::Messages,
            mailboxes: TreeList::new(),
            messages: MultiselectList::new(),
            active_states: initial_states.into_iter().collect(),
            worker_tx,
            worker_rx,
        };
        app.messages
            .replace_items(db.load_messages(app.get_display_filter()).await?);
        app.mailboxes.replace_items(Self::build_mailbox_list(
            db.load_mailboxes(app.get_display_filter()).await?,
        ));
        if let Some(mailbox_name) = initial_mailbox {
            app.mailboxes
                .set_cursor(app.mailboxes.get_items().iter().enumerate().find_map(
                    |(index, mailbox)| {
                        if mailbox.full_name == mailbox_name {
                            Some(index)
                        } else {
                            None
                        }
                    },
                ));
        }
        Ok(app)
    }

    // Change the active pane
    pub fn activate_pane(&mut self, pane: Pane) {
        self.active_pane = pane;
    }

    // Toggle whether a message state is active
    pub fn toggle_active_state(&mut self, state: State) -> Result<()> {
        if self.active_states.contains(&state) {
            self.active_states.remove(&state);
        } else {
            self.active_states.insert(state);
        }
        self.update_mailboxes()?;
        self.update_messages()?;
        Ok(())
    }

    // Generate the mailboxes list
    pub(crate) fn build_mailbox_list(mailbox_sizes: Vec<(String, usize)>) -> Vec<Mailbox> {
        let mut mailboxes = HashMap::<String, Mailbox>::new();
        for (mailbox, count) in mailbox_sizes {
            let sections = mailbox.split('/').collect::<Vec<_>>();
            for index in 0..sections.len() {
                // Children mailboxes contribute to the size of their parents
                let name = sections[0..=index].join("/");
                mailboxes
                    .entry(name.clone())
                    .and_modify(|mailbox| mailbox.message_count += count)
                    .or_insert(Mailbox {
                        full_name: name,
                        name: sections[index].to_string(),
                        depth: index,
                        message_count: count,
                    });
            }
        }
        let mut mailboxes = mailboxes.into_values().collect::<Vec<_>>();
        mailboxes.sort_by(|mailbox1, mailbox2| mailbox1.full_name.cmp(&mailbox2.full_name));
        mailboxes
    }

    // Update the mailboxes list
    pub fn update_mailboxes(&mut self) -> Result<()> {
        self.worker_tx.send(Request::LoadMailboxes(
            MessageFilter::new().with_states(self.get_active_states()),
        ))?;
        Ok(())
    }

    // Update the messages list based on the mailbox and other filters
    pub fn update_messages(&mut self) -> Result<()> {
        let filter = self.get_display_filter();
        self.worker_tx.send(Request::LoadMessages(filter))?;
        Ok(())
    }

    // Handle any pending worker responses without blocking
    pub fn handle_worker_responses(&mut self) -> Result<()> {
        while let Ok(res) = self.worker_rx.try_recv() {
            match res {
                Response::LoadMessages(messages) => self.messages.replace_items(messages),
                Response::LoadMailboxes(mailboxes) => self
                    .mailboxes
                    .replace_items(Self::build_mailbox_list(mailboxes)),
                Response::ChangeMessageStates | Response::DeleteMessages => {
                    self.update_mailboxes()?;
                    self.update_messages()?;
                }
            };
        }
        Ok(())
    }

    // Return a vector of the active states
    fn get_active_states(&self) -> Vec<State> {
        self.active_states.iter().copied().collect()
    }

    // Get the filter representing which messages should be displayed
    pub fn get_display_filter(&self) -> MessageFilter {
        MessageFilter::new()
            .with_mailbox_option(
                self.mailboxes
                    .get_cursor_item()
                    .map(|mailbox| mailbox.full_name.clone()),
            )
            .with_states(self.get_active_states())
    }

    // // Get the filter representing which messages are selected and should be acted upon
    fn get_action_filter(&self) -> MessageFilter {
        let selected_items = self
            .messages
            .get_selected_items()
            .map(|message| message.id)
            .collect::<Vec<_>>();
        let active_ids = if selected_items.is_empty() {
            // If no items are selected, then act on the active item
            self.messages
                .get_cursor_item()
                .map(|message| message.id)
                .into_iter()
                .collect()
        } else {
            selected_items
        };
        MessageFilter::new().with_ids(active_ids)
    }

    // Change the state of all selected messages
    pub fn set_selected_message_states(&mut self, new_state: State) -> Result<()> {
        let action_filter = self.get_action_filter();
        self.worker_tx.send(Request::ChangeMessageStates {
            filter: action_filter.clone(),
            new_state,
        })?;

        // Optimistically update the messages list
        let display_filter = self.get_display_filter();
        self.messages.replace_items(
            self.messages
                .get_items()
                .iter()
                .cloned()
                .filter_map(|message| {
                    if !action_filter.matches_message(&message) {
                        // This message is not being changed, so keep it
                        return Some(message);
                    }

                    let new_message = Message {
                        state: new_state,
                        ..message
                    };
                    // Filter out the message if it no longer matches the display filter
                    if display_filter.matches_message(&new_message) {
                        Some(new_message)
                    } else {
                        None
                    }
                })
                .collect(),
        );

        Ok(())
    }

    // Delete all selected messages
    pub fn delete_selected_messages(&mut self) -> Result<()> {
        let filter = self.get_action_filter();
        self.worker_tx
            .send(Request::DeleteMessages(filter.clone()))?;

        // Optimistically update the message list
        self.messages.replace_items(
            self.messages
                .get_items()
                .iter()
                .filter(|message| !filter.matches_message(message))
                .cloned()
                .collect(),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_mailbox_list() {
        let mailboxes = vec![
            (String::from("a"), 1),
            (String::from("a/b"), 1),
            (String::from("c"), 1),
            (String::from("b"), 1),
            (String::from("b/d/e"), 1),
            (String::from("b/c"), 1),
            (String::from("b/d"), 1),
        ];
        assert_eq!(
            App::build_mailbox_list(mailboxes),
            vec![
                Mailbox {
                    full_name: String::from("a"),
                    name: String::from("a"),
                    depth: 0,
                    message_count: 2,
                },
                Mailbox {
                    full_name: String::from("a/b"),
                    name: String::from("b"),
                    depth: 1,
                    message_count: 1,
                },
                Mailbox {
                    full_name: String::from("b"),
                    name: String::from("b"),
                    depth: 0,
                    message_count: 4,
                },
                Mailbox {
                    full_name: String::from("b/c"),
                    name: String::from("c"),
                    depth: 1,
                    message_count: 1,
                },
                Mailbox {
                    full_name: String::from("b/d"),
                    name: String::from("d"),
                    depth: 1,
                    message_count: 2,
                },
                Mailbox {
                    full_name: String::from("b/d/e"),
                    name: String::from("e"),
                    depth: 2,
                    message_count: 1,
                },
                Mailbox {
                    full_name: String::from("c"),
                    name: String::from("c"),
                    depth: 0,
                    message_count: 1,
                }
            ]
        );
    }
}
