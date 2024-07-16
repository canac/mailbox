use super::multiselect_list::MultiselectList;
use super::navigable_list::{Keyed, NavigableList};
use super::tree_list::{Depth, TreeList};
use super::worker::{spawn, Receiver, Request, Response, Sender};
use anyhow::Result;
use database::{Backend, Database, Filter, MailboxInfo, Message, State};
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
    // The name of the mailbox
    #[allow(clippy::struct_field_names)]
    pub mailbox: database::Mailbox,

    // Root mailbox = 0, child mailbox = 1, grandchild mailbox = 2
    pub depth: usize,

    // The number of messages in the mailbox and its children
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
        hasher.write(self.mailbox.as_ref().as_bytes());
        hasher.finish()
    }
}

impl Keyed for Message {
    fn get_key(&self) -> u64 {
        u64::from(self.id)
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
    pub async fn new<B: Backend + Send + Sync + 'static>(
        db: Database<B>,
        initial_mailbox: Option<database::Mailbox>,
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
        app.mailboxes.replace_items(Self::build_mailbox_list(
            db.load_mailboxes(app.get_display_filter()).await?,
        ));
        if let Some(initial_mailbox) = initial_mailbox {
            app.mailboxes.set_cursor(
                app.mailboxes
                    .get_items()
                    .iter()
                    .position(|mailbox| mailbox.mailbox == initial_mailbox),
            );
        }
        // Load the messages with the initial mailbox filter applied
        app.messages
            .replace_items(db.load_messages(app.get_display_filter()).await?);
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
    pub(crate) fn build_mailbox_list(mailbox_sizes: Vec<MailboxInfo>) -> Vec<Mailbox> {
        let mut mailboxes = HashMap::<database::Mailbox, Mailbox>::new();
        for MailboxInfo {
            name: mailbox,
            message_count: count,
        } in mailbox_sizes
        {
            for (index, full_name) in mailbox.iter_ancestors().enumerate() {
                // Children mailboxes contribute to the size of their parents
                mailboxes
                    .entry(full_name.clone())
                    .or_insert(Mailbox {
                        mailbox: full_name,
                        depth: index,
                        message_count: 0,
                    })
                    .message_count += count;
            }
        }
        let mut mailboxes = mailboxes.into_values().collect::<Vec<_>>();
        mailboxes.sort_by(|mailbox1, mailbox2| mailbox1.mailbox.cmp(&mailbox2.mailbox));
        mailboxes
    }

    // Update the messages list based on the mailbox and other filters
    pub fn filter_messages(&mut self) {
        let filter = self.get_display_filter();
        self.messages.replace_items(
            self.messages
                .get_items()
                .iter()
                .filter(|&item| filter.matches_message(item))
                .cloned()
                .collect(),
        );
    }

    // Update the mailboxes list
    pub fn update_mailboxes(&mut self) -> Result<()> {
        self.worker_tx.send(Request::LoadMailboxes(
            Filter::new().with_states(self.get_active_states()),
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
                Response::LoadMailboxes(mailboxes) => {
                    let old_display_filter = self.get_display_filter();
                    self.mailboxes
                        .replace_items(Self::build_mailbox_list(mailboxes));
                    if old_display_filter != self.get_display_filter() {
                        // If changing the mailbox list changed the active mailbox, refresh the message list
                        self.update_messages()?;
                    }
                }
                Response::Refresh => {
                    // A change or delete messages mutation has completed that changed the active mailbox, so now
                    // refresh the mailbox and message lists. We have to wait for the mutation to complete first to
                    // avoid loading the unchanged messages.
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
    pub fn get_display_filter(&self) -> Filter {
        Filter::new()
            .with_mailbox_option(
                self.mailboxes
                    .get_cursor_item()
                    .map(|mailbox| mailbox.mailbox.clone()),
            )
            .with_states(self.get_active_states())
    }

    // // Get the filter representing which messages are selected and should be acted upon
    fn get_action_filter(&self) -> Filter {
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
        Filter::new().with_ids(active_ids)
    }

    // Change the state of all selected messages
    pub fn set_selected_message_states(&mut self, new_state: State) -> Result<()> {
        let action_filter = self.get_action_filter();

        // Optimistically update the messages list
        let display_filter = self.get_display_filter();
        let (remaining, removed) = self
            .messages
            .get_items()
            .iter()
            .cloned()
            .map(|message| {
                if action_filter.matches_message(&message) {
                    Message {
                        state: new_state,
                        ..message
                    }
                } else {
                    // This message is not being changed, so keep it
                    message
                }
            })
            .partition(|message| display_filter.matches_message(message));
        self.messages.replace_items(remaining);

        // Optimistically update the mailbox list
        let old_display_filter = self.get_display_filter();
        self.remove_messages_from_mailboxes(removed);

        // Apply the mutation
        self.worker_tx.send(Request::ChangeMessageStates {
            filter: action_filter,
            new_state,
            // If changing the mailbox list changed the active mailbox, the message list needs to be refreshed
            // The actual refreshing is done when handle_worker_response receives the refresh response
            response: if old_display_filter == self.get_display_filter() {
                None
            } else {
                Some(Response::Refresh)
            },
        })?;

        Ok(())
    }

    // Delete all selected messages
    pub fn delete_selected_messages(&mut self) -> Result<()> {
        let filter = self.get_action_filter();

        // Optimistically update the message list
        let (deleted, remaining) = self
            .messages
            .get_items()
            .iter()
            .cloned()
            .partition(|message| filter.matches_message(message));
        self.messages.replace_items(remaining);

        // Optimistically update the mailbox list
        let old_display_filter = self.get_display_filter();
        self.remove_messages_from_mailboxes(deleted);

        // Apply the mutation
        self.worker_tx.send(Request::DeleteMessages {
            filter,
            // If changing the mailbox list changed the active mailbox, the message list needs to be refreshed
            // The actual refreshing is done when handle_worker_response receives the refresh response
            response: if old_display_filter == self.get_display_filter() {
                None
            } else {
                Some(Response::Refresh)
            },
        })?;

        Ok(())
    }

    // Update the mailbox list to reflect the removal of the messages from the message list
    fn remove_messages_from_mailboxes(&mut self, messages: Vec<Message>) {
        // First determine which mailboxes are losing messages and how many
        let mut count_decreases = HashMap::<database::Mailbox, usize>::new();
        for message in messages {
            for mailbox in message.mailbox.iter_ancestors() {
                *count_decreases.entry(mailbox).or_default() += 1;
            }
        }
        // Then decrease the message count for those messages, removing empty mailboxes
        self.mailboxes.replace_items(
            self.mailboxes
                .get_items()
                .iter()
                .filter_map(|mailbox| {
                    let message_count = mailbox.message_count.saturating_sub(
                        count_decreases
                            .get(&mailbox.mailbox)
                            .copied()
                            .unwrap_or_default(),
                    );
                    if message_count == 0 {
                        None
                    } else {
                        Some(Mailbox {
                            mailbox: mailbox.mailbox.clone(),
                            depth: mailbox.depth,
                            message_count,
                        })
                    }
                })
                .collect(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_mailbox_list() -> Result<()> {
        let mailboxes = vec![
            MailboxInfo {
                name: "a".try_into()?,
                message_count: 1,
            },
            MailboxInfo {
                name: "a/b".try_into()?,
                message_count: 1,
            },
            MailboxInfo {
                name: "c".try_into()?,
                message_count: 1,
            },
            MailboxInfo {
                name: "b".try_into()?,
                message_count: 1,
            },
            MailboxInfo {
                name: "b/d/e".try_into()?,
                message_count: 1,
            },
            MailboxInfo {
                name: "b/c".try_into()?,
                message_count: 1,
            },
            MailboxInfo {
                name: "b/d".try_into()?,
                message_count: 1,
            },
        ];
        assert_eq!(
            App::build_mailbox_list(mailboxes),
            vec![
                Mailbox {
                    mailbox: "a".try_into()?,
                    depth: 0,
                    message_count: 2,
                },
                Mailbox {
                    mailbox: "a/b".try_into()?,
                    depth: 1,
                    message_count: 1,
                },
                Mailbox {
                    mailbox: "b".try_into()?,
                    depth: 0,
                    message_count: 4,
                },
                Mailbox {
                    mailbox: "b/c".try_into()?,
                    depth: 1,
                    message_count: 1,
                },
                Mailbox {
                    mailbox: "b/d".try_into()?,
                    depth: 1,
                    message_count: 2,
                },
                Mailbox {
                    mailbox: "b/d/e".try_into()?,
                    depth: 2,
                    message_count: 1,
                },
                Mailbox {
                    mailbox: "c".try_into()?,
                    depth: 0,
                    message_count: 1,
                }
            ]
        );
        Ok(())
    }
}
