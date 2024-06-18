use crate::mailbox::Mailbox;
use crate::message::{Id, Message, MessageIden, State};
use sea_query::{Cond, Condition, Expr};
use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::string::ToString;

// Serialize Option<Vec<T>> into a comma-separated string so that serde_urlencoded can handle it
fn serialize_vec_to_csv<S, T>(vec: &Option<Vec<T>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: ToString,
{
    match vec {
        Some(item) => {
            let csv = item
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join(",");
            serializer.serialize_some(&csv)
        }
        None => serializer.serialize_none(),
    }
}

// Deserialize Option<Vec<T>> from a comma-separated string so that serde_urlencoded can handle it
fn deserialize_vec_from_csv<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let csv = Option::<String>::deserialize(deserializer)?;
    csv.map(|csv| {
        if csv.is_empty() {
            return Ok(vec![]);
        }

        csv.split(',')
            .map(|item| item.parse().map_err(de::Error::custom))
            .collect::<Result<Vec<T>, _>>()
    })
    .transpose()
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[must_use]
pub struct MessageFilter {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_vec_to_csv",
        deserialize_with = "deserialize_vec_from_csv",
        default
    )]
    ids: Option<Vec<Id>>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    mailbox: Option<Mailbox>,

    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_vec_to_csv",
        deserialize_with = "deserialize_vec_from_csv",
        default
    )]
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

    #[test]
    fn test_serialize_ids() {
        let filter = MessageFilter::new().with_ids(vec![1]);
        assert_eq!(serde_urlencoded::to_string(filter).unwrap(), "ids=1");

        let filter = MessageFilter::new().with_ids(vec![1, 2, 3]);
        assert_eq!(
            serde_urlencoded::to_string(filter).unwrap(),
            "ids=1%2C2%2C3"
        );
    }

    #[test]
    fn test_serialize_mailbox() {
        let filter = MessageFilter::new().with_mailbox("foo".try_into().unwrap());
        assert_eq!(serde_urlencoded::to_string(filter).unwrap(), "mailbox=foo");
    }

    #[test]
    fn test_serialize_states() {
        let filter = MessageFilter::new().with_states(vec![State::Unread]);
        assert_eq!(
            serde_urlencoded::to_string(filter).unwrap(),
            "states=unread"
        );

        let filter = MessageFilter::new().with_states(vec![State::Read, State::Archived]);
        assert_eq!(
            serde_urlencoded::to_string(filter).unwrap(),
            "states=read%2Carchived"
        );
    }

    #[test]
    fn test_serialize_multiple() {
        let filter = MessageFilter::new()
            .with_ids(vec![1, 2, 3])
            .with_mailbox("foo".try_into().unwrap())
            .with_states(vec![State::Unread, State::Read]);
        assert_eq!(
            serde_urlencoded::to_string(filter).unwrap(),
            "ids=1%2C2%2C3&mailbox=foo&states=unread%2Cread"
        );
    }

    #[test]
    fn test_deserialize_ids() {
        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("ids=1").unwrap(),
            MessageFilter::new().with_ids(vec![1])
        );

        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("ids=1,2,3").unwrap(),
            MessageFilter::new().with_ids(vec![1, 2, 3])
        );

        assert!(serde_urlencoded::from_str::<MessageFilter>("ids=1,2,a").is_err());
    }

    #[test]
    fn test_deserialize_mailbox() {
        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("mailbox=foo").unwrap(),
            MessageFilter::new().with_mailbox("foo".try_into().unwrap())
        );
    }

    #[test]
    fn test_deserialize_states() {
        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("states=unread").unwrap(),
            MessageFilter::new().with_states(vec![State::Unread])
        );

        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("states=read,archived").unwrap(),
            MessageFilter::new().with_states(vec![State::Read, State::Archived])
        );

        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("states=").unwrap(),
            MessageFilter::new().with_states(vec![])
        );

        assert!(serde_urlencoded::from_str::<MessageFilter>("states=unread,foo").is_err());
    }

    #[test]
    fn test_deserialize_multiple() {
        assert_eq!(
            serde_urlencoded::from_str::<MessageFilter>("ids=1,2,3&mailbox=foo&states=unread,read")
                .unwrap(),
            MessageFilter::new()
                .with_ids(vec![1, 2, 3])
                .with_mailbox("foo".try_into().unwrap())
                .with_states(vec![State::Unread, State::Read])
        );
    }
}
