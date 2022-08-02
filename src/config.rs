use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Override {
    Unread,
    Read,
    Archived,
    Ignored,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    overrides: HashMap<String, Override>,
}

impl Config {
    // Load the configuration file from the provided path
    pub fn load(path: PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(path).context("Error reading config file")?;
        toml::from_str(&contents).context("Error parsing config file")
    }

    // Return the configured override for the given mailbox if there is one
    pub fn get_override(&self, mailbox: &str) -> Option<Override> {
        let sections = mailbox.split('/').collect::<Vec<_>>();
        (0..sections.len())
            .rev()
            .find_map(|index| self.overrides.get(&sections[0..=index].join("/")))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_config(toml: &str) -> Result<Config> {
        Ok(toml::from_str(toml)?)
    }

    #[test]
    fn test_empty() {
        assert!(load_config("").is_ok());
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
}
