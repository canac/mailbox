use super::multiselect_list::MultiselectList;
use super::navigable_list::{Keyed, NavigableList};
use super::tree_list::{Depth, TreeList};
use crate::database::Database;
use crate::message::{Message, MessageState};
use crate::message_filter::MessageFilter;
use anyhow::Result;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;

pub enum Pane {
    Mailboxes,
    Messages,
}

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
    pub(crate) active_states: HashSet<MessageState>,
    db: Database,
}

impl App {
    pub fn new(
        db: Database,
        initial_mailbox: Option<String>,
        initial_states: Vec<MessageState>,
    ) -> Result<App> {
        let mut app = App {
            active_pane: Pane::Messages,
            mailboxes: TreeList::new(),
            messages: MultiselectList::new(),
            active_states: initial_states.into_iter().collect(),
            db,
        };
        app.update_mailboxes()?;
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
        app.update_messages()?;
        Ok(app)
    }

    // Change the active pane
    pub fn activate_pane(&mut self, pane: Pane) {
        self.active_pane = pane;
    }

    // Toggle whether a message state is active
    pub fn toggle_active_state(&mut self, state: MessageState) -> Result<()> {
        if self.active_states.contains(&state) {
            self.active_states.remove(&state);
        } else {
            self.active_states.insert(state);
        }
        self.update_mailboxes()?;
        self.update_messages()?;
        Ok(())
    }

    // Update the mailboxes list
    pub fn update_mailboxes(&mut self) -> Result<()> {
        let mut mailboxes = HashMap::<String, Mailbox>::new();
        for (mailbox, count) in self
            .db
            .load_mailboxes(MessageFilter::new().with_states(self.get_active_states()))?
        {
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
        let mut mailboxes = mailboxes.into_values().into_iter().collect::<Vec<_>>();
        mailboxes.sort_by(|mailbox1, mailbox2| mailbox1.full_name.cmp(&mailbox2.full_name));
        self.mailboxes.replace_items(mailboxes);
        Ok(())
    }

    // Update the messages list based on the mailbox and other filters
    pub fn update_messages(&mut self) -> Result<()> {
        let filter = self.get_display_filter();
        self.messages.replace_items(
            self.db
                .load_messages(filter)?
                .into_iter()
                .map(|message| Message {
                    id: message.id,
                    timestamp: message.timestamp,
                    mailbox: message.mailbox,
                    content: message.content,
                    state: message.state,
                })
                .collect(),
        );
        Ok(())
    }

    // Return a vector of the active states
    fn get_active_states(&self) -> Vec<MessageState> {
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
        self.get_display_filter().with_ids(active_ids.into_iter())
    }

    // Change the state of all selected messages
    pub fn set_selected_message_states(&mut self, new_state: MessageState) -> Result<()> {
        self.db.change_state(self.get_action_filter(), new_state)?;
        self.update_mailboxes()?;
        self.update_messages()?;
        Ok(())
    }

    // Delete all selected messages
    pub fn delete_selected_messages(&mut self) -> Result<()> {
        self.db.delete_messages(self.get_action_filter())?;
        self.update_mailboxes()?;
        self.update_messages()?;
        Ok(())
    }
}
