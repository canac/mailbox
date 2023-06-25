use crate::message::{Message, MessageIden, MessageState};
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
                    .add(Expr::col(MessageIden::Mailbox).like(format!("{mailbox}/%")))
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

    // Determine whether a message matches the filter
    pub fn matches_message(&self, message: &Message) -> bool {
        if let Some(ids) = self.ids.as_ref() {
            if !ids.contains(&message.id) {
                return false;
            }
        }
        if let Some(mailbox) = self.mailbox.as_ref() {
            if !(mailbox == &message.mailbox
                || message.mailbox.starts_with(format!("{mailbox}/").as_str()))
            {
                return false;
            }
        }
        if let Some(states) = self.states.as_ref() {
            if !states.contains(&message.state) {
                return false;
            }
        }
        return true;
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
            mailbox: String::from("parent/child"),
            content: String::from("Content"),
            state: MessageState::Unread,
        }
    }

    #[test]
    fn test_matches_message_empty_filter() {
        let message = get_message();
        assert_eq!(MessageFilter::new().matches_message(&message), true);
    }

    #[test]
    fn test_matches_message_id_filter() {
        let message = get_message();
        assert_eq!(
            MessageFilter::new()
                .with_ids([1].into_iter())
                .matches_message(&message),
            true
        );
        assert_eq!(
            MessageFilter::new()
                .with_ids([1, 2].into_iter())
                .matches_message(&message),
            true
        );
        assert_eq!(
            MessageFilter::new()
                .with_ids([2].into_iter())
                .matches_message(&message),
            false
        );
    }

    #[test]
    fn test_matches_message_mailbox_filter() {
        let message = get_message();
        assert_eq!(
            MessageFilter::new()
                .with_mailbox("parent")
                .matches_message(&message),
            true
        );
        assert_eq!(
            MessageFilter::new()
                .with_mailbox("parent/child")
                .matches_message(&message),
            true
        );
        assert_eq!(
            MessageFilter::new()
                .with_mailbox("parent/child2")
                .matches_message(&message),
            false
        );
        assert_eq!(
            MessageFilter::new()
                .with_mailbox("parent/child/grandchild")
                .matches_message(&message),
            false
        );
    }

    #[test]
    fn test_matches_message_state_filter() {
        let message = get_message();
        assert_eq!(
            MessageFilter::new()
                .with_states(vec![MessageState::Unread])
                .matches_message(&message),
            true
        );
        assert_eq!(
            MessageFilter::new()
                .with_states(vec![MessageState::Unread, MessageState::Read])
                .matches_message(&message),
            true
        );
        assert_eq!(
            MessageFilter::new()
                .with_states(vec![MessageState::Read])
                .matches_message(&message),
            false
        );
    }
}
