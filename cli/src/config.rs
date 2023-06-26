use anyhow::{Context, Result};
use database::{MessageState, NewMessage};
use serde::Deserialize;
use std::{collections::HashMap, io::ErrorKind, path::PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Override {
    Unread,
    Read,
    Archived,
    Ignored,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "lowercase", tag = "provider")]
pub enum DatabaseProvider {
    #[default]
    Sqlite,
    Postgres {
        url: String,
    },
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    overrides: HashMap<String, Override>,

    #[serde(default)]
    pub database: DatabaseProvider,
}

impl Config {
    // Load the configuration file from the provided path
    pub fn load(path: &PathBuf) -> Result<Option<Self>> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Ok(Some(toml::from_str(&contents).with_context(|| {
                format!("Failed to parse config file {}", path.to_string_lossy())
            })?)),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).context("Failed to read config file"),
        }
    }

    // Return the configured override for the given mailbox if there is one
    pub fn get_override(&self, mailbox: &str) -> Option<Override> {
        let sections = mailbox.split('/').collect::<Vec<_>>();
        (0..sections.len())
            .rev()
            .find_map(|index| self.overrides.get(&sections[0..=index].join("/")))
            .copied()
    }

    // Take an iterator of new messages and apply the overrides defined in
    // this config, returning the new iterator
    pub fn apply_override(&self, message: NewMessage) -> Option<NewMessage> {
        let overridden_state = self.get_override(&message.mailbox);
        let state = match overridden_state {
            Some(Override::Unread) => Some(MessageState::Unread),
            Some(Override::Read) => Some(MessageState::Read),
            Some(Override::Archived) => Some(MessageState::Archived),
            // Skip this message entirely
            Some(Override::Ignored) => return None,
            None => message.state,
        };
        Some(NewMessage { state, ..message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_config(toml: &str) -> Result<Config> {
        Ok(toml::from_str(toml)?)
    }

    fn apply_override(config: &Config, mailbox: &str) -> Option<NewMessage> {
        config.apply_override(NewMessage {
            mailbox: mailbox.to_string(),
            content: "Content".to_string(),
            state: Some(MessageState::Unread),
        })
    }

    #[test]
    fn test_empty() {
        assert!(load_config("").is_ok());
    }

    #[test]
    fn test_load_provider() {
        assert_eq!(
            load_config("[database]\nprovider = 'sqlite'\n")
                .unwrap()
                .database,
            DatabaseProvider::Sqlite
        );
        assert!(load_config("[database]\nprovider = 'postgres'\n").is_err());
        assert_eq!(
            load_config("[database]\nprovider = 'postgres'\nurl = 'postgres://'\n")
                .unwrap()
                .database,
            DatabaseProvider::Postgres {
                url: String::from("postgres://")
            }
        );
        assert!(load_config("[database]\nprovider = 'foo'\n").is_err());
    }

    #[test]
    fn test_load_overrides() {
        assert!(load_config("[overrides]\nfoo = 'unread'\n").is_ok());
        assert!(load_config("[overrides]\nfoo = 'bar'\n").is_err());
    }

    #[test]
    fn test_get_overrides() -> Result<()> {
        let config = load_config("[overrides]\n'a/b/c' = 'ignored'\n'a' = 'read'")?;
        assert_eq!(config.get_override("a/b/c/d"), Some(Override::Ignored));
        assert_eq!(config.get_override("a/b/c"), Some(Override::Ignored));
        assert_eq!(config.get_override("a/b"), Some(Override::Read));
        assert_eq!(config.get_override("a"), Some(Override::Read));
        assert_eq!(config.get_override("b"), None);
        Ok(())
    }

    #[test]
    fn test_apply_override() {
        let config = Config {
            overrides: HashMap::from([
                ("a/b/c".to_string(), Override::Ignored),
                ("a".to_string(), Override::Read),
            ]),
            database: Default::default(),
        };

        assert!(apply_override(&config, "a/b/c/d").is_none());
        assert!(apply_override(&config, "a/b/c").is_none());
        assert_eq!(
            apply_override(&config, "a/b").unwrap().state,
            Some(MessageState::Read)
        );
        assert_eq!(
            apply_override(&config, "a").unwrap().state,
            Some(MessageState::Read)
        );
        assert_eq!(
            apply_override(&config, "b").unwrap().state,
            Some(MessageState::Unread)
        );
    }
}
