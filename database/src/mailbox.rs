use anyhow::bail;
use sea_query::Value;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Mailbox(String);

impl Mailbox {
    // Iterate over the mailbox's ancestor mailboxes, including itself
    // Mailbox "a/b/c" with produce "a", "a/b", "a/b/c"
    pub fn iter_ancestors(&self) -> impl Iterator<Item = Mailbox> + '_ {
        let sections = self.0.split('/').collect::<Vec<_>>();
        (0..sections.len()).map(move |index| Mailbox(sections[0..=index].join("/")))
    }

    // Return the name of the mailbox without its ancestors
    #[must_use]
    pub fn get_leaf_name(&self) -> &str {
        self.0.split('/').last().unwrap_or_default()
    }
}

impl AsRef<str> for Mailbox {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for Mailbox {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Mailbox {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        s.try_into()
    }
}

impl TryFrom<&str> for Mailbox {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<Self> {
        String::from(value).try_into()
    }
}

impl TryFrom<String> for Mailbox {
    type Error = anyhow::Error;

    fn try_from(value: String) -> anyhow::Result<Self> {
        if value.is_empty() {
            bail!("mailbox must not be empty");
        }
        if value.starts_with('/') {
            bail!("mailbox must not start with /");
        }
        if value.ends_with('/') {
            bail!("mailbox must not end with /");
        }
        if value.contains("//") {
            bail!("mailbox must not contain //");
        }
        if value.contains('%') {
            bail!("mailbox must not contain %");
        }

        Ok(Self(value))
    }
}

impl From<Mailbox> for String {
    fn from(value: Mailbox) -> Self {
        value.0
    }
}

impl From<Mailbox> for Value {
    fn from(value: Mailbox) -> Value {
        Value::String(Some(Box::new(value.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_ancestors() {
        let mailbox: Mailbox = "a/b/c".try_into().unwrap();
        assert_eq!(
            mailbox.iter_ancestors().collect::<Vec<_>>(),
            vec![
                "a".try_into().unwrap(),
                "a/b".try_into().unwrap(),
                "a/b/c".try_into().unwrap()
            ]
        );
    }

    #[test]
    fn test_get_leaf_name() {
        let mailbox: Mailbox = "a/b/c".try_into().unwrap();
        assert_eq!(mailbox.get_leaf_name(), "c");
    }

    #[test]
    fn test_try_into() {
        assert!(TryInto::<Mailbox>::try_into("parent/child").is_ok());
        assert!(TryInto::<Mailbox>::try_into("").is_err());
        assert!(TryInto::<Mailbox>::try_into("mailbox/").is_err());
        assert!(TryInto::<Mailbox>::try_into("/mailbox").is_err());
        assert!(TryInto::<Mailbox>::try_into("parent//child").is_err());
        assert!(TryInto::<Mailbox>::try_into("parent/%").is_err());
    }
}
