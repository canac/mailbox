use crate::message::{MessageIden, MessageState};
use sea_query::{Cond, Condition, Expr};

#[derive(Clone)]
pub struct MessageFilter {
    ids: Option<Vec<i32>>,
    mailbox: Option<String>,
    states: Option<Vec<MessageState>>,
}

// MessageFilter is a consistent interface for filtering messages in Database methods.
// It utilizes the builder pattern.
impl MessageFilter {
    // Create a new message filter
    pub fn new() -> Self {
        MessageFilter {
            mailbox: None,
            states: None,
            ids: None,
        }
    }

    // Add a mailbox filter
    pub fn with_mailbox(mut self, mailbox: impl Into<String>) -> Self {
        self.mailbox = Some(mailbox.into());
        self
    }

    // Add a mailbox filter if the option is Some
    pub fn with_mailbox_option(self, mailbox: Option<impl Into<String>>) -> Self {
        match mailbox {
            Some(mailbox) => self.with_mailbox(mailbox),
            None => self,
        }
    }

    // Add a states filter
    pub fn with_states(mut self, states: Vec<MessageState>) -> Self {
        self.states = Some(states);
        self
    }

    // Add IDs to a filter
    pub fn with_ids(mut self, ids: impl Iterator<Item = i32>) -> Self {
        self.ids = Some(ids.collect());
        self
    }

    // Generate a sea-query where expression message filter
    pub fn get_where(self) -> Condition {
        Cond::all()
            .add_option(self.ids.map(|ids| Expr::col(MessageIden::Id).is_in(ids)))
            .add_option(self.mailbox.map(|mailbox| {
                Cond::any()
                    .add(Expr::col(MessageIden::Mailbox).eq(mailbox.clone()))
                    .add(Expr::cust_with_values(
                        "mailbox GLOB ?",
                        vec![format!("{mailbox}/*")],
                    ))
            }))
            .add_option(self.states.map(|states| {
                Expr::col(MessageIden::State).is_in(
                    states
                        .iter()
                        .map(|state| (*state).into())
                        .collect::<Vec<i64>>(),
                )
            }))
    }
}
