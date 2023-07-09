use anyhow::{Context, Result};
use database::{Mailbox, MessageFilter};
use serde::de::value::{Error, StrDeserializer};
use serde::de::IntoDeserializer;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    ids: Option<String>,
    mailbox: Option<Mailbox>,
    states: Option<String>,
}

impl TryFrom<Filter> for MessageFilter {
    type Error = anyhow::Error;

    fn try_from(value: Filter) -> Result<Self, Self::Error> {
        let mut message_filter = MessageFilter::new();
        if let Some(ids) = value.ids.as_ref() {
            message_filter = message_filter.with_ids(
                ids.split(',')
                    .map(|id| id.parse().context("Failed to parse ids"))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
        if let Some(mailbox) = value.mailbox {
            message_filter = message_filter.with_mailbox(mailbox);
        }
        if let Some(states) = value.states.as_ref() {
            message_filter = message_filter.with_states(
                states
                    .split(',')
                    .map(|state| {
                        Deserialize::deserialize::<StrDeserializer<'_, Error>>(
                            state.into_deserializer(),
                        )
                        .context("Failed to parse states")
                    })
                    .collect::<Result<Vec<_>>>()?,
            );
        }
        Ok(message_filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_ids() {
        assert!(TryInto::<MessageFilter>::try_into(Filter {
            ids: Some(String::from("1,2,3")),
            mailbox: None,
            states: None,
        })
        .is_ok());

        assert!(TryInto::<MessageFilter>::try_into(Filter {
            ids: Some(String::from("1,2,a")),
            mailbox: None,
            states: None,
        })
        .is_err());
    }

    #[test]
    fn test_parses_states() {
        assert!(TryInto::<MessageFilter>::try_into(Filter {
            ids: None,
            mailbox: None,
            states: Some(String::from("unread,read,archived")),
        })
        .is_ok());

        assert!(TryInto::<MessageFilter>::try_into(Filter {
            ids: None,
            mailbox: None,
            states: Some(String::from("unread,read,archived,foo")),
        })
        .is_err());
    }
}
