use crate::mailbox::Mailbox;
use crate::message::{Id, Message, MessageIden, State};
use sea_query::{Cond, Condition, Expr};
use serde::Deserialize;

#[derive(Clone, Default, Deserialize, Eq, PartialEq)]
#[must_use]
pub struct MessageFilter {
    ids: Option<Vec<Id>>,
    mailbox: Option<Mailbox>,
    states: Option<Vec<State>>,
}

// MessageFilter is a consistent interface for filtering messages in Database methods.
// It utilizes the builder pattern.
impl MessageFilter {
    // Create a new message filter
    pub fn new() -> Self {
        MessageFilter::default()
    }

    // Add a mailbox filter
    pub fn with_mailbox(mut self, mailbox: Mailbox) -> Self {
        self.mailbox = Some(mailbox);
        self
    }

    // Add a mailbox filter if the option is Some
    pub fn with_mailbox_option(self, mailbox: Option<Mailbox>) -> Self {
        match mailbox {
            Some(mailbox) => self.with_mailbox(mailbox),
            None => self,
        }
    }

    // Add a states filter
    pub fn with_states(mut self, states: Vec<State>) -> Self {
        self.states = Some(states);
        self
    }

    // Add IDs to a filter
    pub fn with_ids(mut self, ids: Vec<Id>) -> Self {
        self.ids = Some(ids);
        self
    }

    // Generate a sea-query where expression message filter
    #[must_use]
    pub fn get_where(self) -> Condition {
        Cond::all()
            .add_option(self.ids.map(|ids| Expr::col(MessageIden::Id).is_in(ids)))
            .add_option(self.mailbox.map(|mailbox| {
                Cond::any()
                    .add(Expr::col(MessageIden::Mailbox).like(format!("{mailbox}/%")))
                    .add(Expr::col(MessageIden::Mailbox).eq(mailbox))
            }))
            .add_option(self.states.map(|states| {
                Expr::col(MessageIden::State).is_in(
                    states
                        .iter()
                        .map(|state| (*state).into())
                        .collect::<Vec<i32>>(),
                )
            }))
    }

    // Determine whether a message filter is unrestricted and matches all messages
    #[must_use]
    pub fn matches_all(&self) -> bool {
        self.ids.is_none() && self.mailbox.is_none() && self.states.is_none()
    }

    // Determine whether a message matches the filter
    #[must_use]
    pub fn matches_message(&self, message: &Message) -> bool {
        if let Some(ids) = self.ids.as_ref() {
            if !ids.contains(&message.id) {
                return false;
            }
        }
        if let Some(mailbox) = self.mailbox.as_ref() {
            if !(mailbox == &message.mailbox
                || message
                    .mailbox
                    .as_ref()
                    .starts_with(format!("{mailbox}/").as_str()))
            {
                return false;
            }
        }
        if let Some(states) = self.states.as_ref() {
            if !states.contains(&message.state) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use super::*;

    fn get_message() -> Message {
        Message {
            id: 1,
            timestamp: NaiveDateTime::MIN,
            mailbox: "parent/child".try_into().unwrap(),
            content: String::from("Content"),
            state: State::Unread,
        }
    }

    #[test]
    fn test_matches_all() {
        assert!(MessageFilter::new().matches_all());
        assert!(!MessageFilter::new().with_ids(vec![1]).matches_all());
        assert!(!MessageFilter::new()
            .with_mailbox("foo".try_into().unwrap())
            .matches_all());
        assert!(!MessageFilter::new()
            .with_states(vec![State::Unread])
            .matches_all());
    }

    #[test]
    fn test_matches_message_empty_filter() {
        let message = get_message();
        assert!(MessageFilter::new().matches_message(&message));
    }

    #[test]
    fn test_matches_message_id_filter() {
        let message = get_message();
        assert!(MessageFilter::new()
            .with_ids(vec![1])
            .matches_message(&message));
        assert!(MessageFilter::new()
            .with_ids(vec![1, 2])
            .matches_message(&message));
        assert!(!MessageFilter::new()
            .with_ids(vec![2])
            .matches_message(&message));
    }

    #[test]
    fn test_matches_message_mailbox_filter() {
        let message = get_message();
        assert!(MessageFilter::new()
            .with_mailbox("parent".try_into().unwrap())
            .matches_message(&message));
        assert!(MessageFilter::new()
            .with_mailbox("parent/child".try_into().unwrap())
            .matches_message(&message));
        assert!(!MessageFilter::new()
            .with_mailbox("parent/child2".try_into().unwrap())
            .matches_message(&message));
        assert!(!MessageFilter::new()
            .with_mailbox("parent/child/grandchild".try_into().unwrap())
            .matches_message(&message));
    }

    #[test]
    fn test_matches_message_state_filter() {
        let message = get_message();
        assert!(MessageFilter::new()
            .with_states(vec![State::Unread])
            .matches_message(&message));
        assert!(MessageFilter::new()
            .with_states(vec![State::Unread, State::Read])
            .matches_message(&message));
        assert!(!MessageFilter::new()
            .with_states(vec![State::Read])
            .matches_message(&message));
    }
}
