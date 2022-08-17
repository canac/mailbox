use crate::message::{Message, MessageState};
use crate::message_components::MessageComponents;
use crate::truncate::TruncatedLine;
use chrono::{Local, TimeZone, Utc};
use chrono_humanize::HumanTime;
use std::collections::HashMap;

pub enum TimestampFormat {
    Relative,
    Local,
    #[allow(dead_code)] // only used in tests
    Utc,
}

enum Word {
    Message,
    Mailbox,
}

struct Mailbox<'messages> {
    // The name of the mailbox
    name: &'messages String,

    // The messages in the mailbox, sorted in the order they will be displayed
    // Will never be empty
    messages: Vec<&'messages Message>,

    // The timestamp of the mailbox's most recent message
    most_recent_timestamp: i64,

    // The number lines that this mailbox is allocated to display its contents
    allocated_lines: usize,
}

impl<'messages> Mailbox<'messages> {
    // Create a new mailbox containing the provided messages
    // Will panic if messages is an empty vector
    fn new(name: &'messages String, messages: Vec<&'messages Message>) -> Self {
        let mut messages = messages;

        // Sort the messages with newest ones first, the alphabetically by mailbox name
        messages.sort_by_key(|message| (-message.timestamp.timestamp(), &message.mailbox));
        let timestamp = messages
            .first()
            .expect("messages must not be empty")
            .timestamp
            .timestamp();

        Mailbox {
            name,
            messages,
            most_recent_timestamp: timestamp,
            allocated_lines: 0,
        }
    }
}

pub struct MessageFormatter {
    color: bool,
    timestamp_format: TimestampFormat,
    max_columns: Option<usize>,
    max_lines: Option<usize>,
}

// MessageFormatter is responsible for formatting individual messages as well
// as lists of messages. The output can be colorized or not, the timestamp
// format can be adjusted, and the maximum number of lines to output can also
// be configured.
impl MessageFormatter {
    pub fn new() -> Self {
        Self {
            color: true,
            timestamp_format: TimestampFormat::Relative,
            max_columns: None,
            max_lines: None,
        }
    }

    // Configure whether the output is colored
    pub fn with_color(self, color: bool) -> Self {
        Self { color, ..self }
    }

    // Configure the output timestamp format
    pub fn with_timestamp_format(self, timestamp_format: TimestampFormat) -> Self {
        Self {
            timestamp_format,
            ..self
        }
    }

    // Configure the maximum number of output columns, None is no limit
    pub fn with_max_columns(self, max_columns: Option<usize>) -> Self {
        Self {
            max_columns,
            ..self
        }
    }

    // Configure the maximum number of output lines, None is no limit
    pub fn with_max_lines(self, max_lines: Option<usize>) -> Self {
        Self { max_lines, ..self }
    }

    // Format a single message into a string. There will not be a newline at the end.
    pub fn format_message(&self, message: &Message, appendix: Option<String>) -> String {
        use colored::*;

        // Display the time differently based on the requested format
        let time = match self.timestamp_format {
            TimestampFormat::Relative => HumanTime::from(
                message
                    .timestamp
                    .signed_duration_since(Utc::now().naive_utc()),
            )
            .to_string(),
            TimestampFormat::Local => Local
                .timestamp(message.timestamp.timestamp(), 0)
                .to_string(),
            TimestampFormat::Utc => Utc.timestamp(message.timestamp.timestamp(), 0).to_string(),
        };

        let max_columns = self.max_columns.unwrap_or(usize::MAX);
        let components = MessageComponents {
            state: message.state,
            content: message.content.clone(),
            mailbox: message.mailbox.clone(),
            time,
            appendix: appendix.unwrap_or_default(),
        }
        .truncate(max_columns);

        let mut line = TruncatedLine::new(max_columns);
        line.append(
            components.state.to_string(),
            if matches!(message.state, MessageState::Unread) && self.color {
                Some(|str: String| str.red().bold())
            } else {
                None
            },
        );
        line.append(format!(" {} [", components.content), None);
        line.append(
            components.mailbox,
            if self.color {
                Some(|str: String| str.green().bold())
            } else {
                None
            },
        );
        line.append("] @ ", None);
        line.append(
            components.time,
            if self.color {
                Some(|str: String| str.yellow())
            } else {
                None
            },
        );
        line.append(components.appendix, None);
        line.to_string()
    }

    // Format multiple messages into a string. There will be a newline at the end.
    pub fn format_messages(&self, messages: &[Message]) -> String {
        // Group the messages by mailbox
        let mut mailboxes: HashMap<&String, Vec<&Message>> = HashMap::new();
        for message in messages {
            let key = &message.mailbox;
            if let Some(value) = mailboxes.get_mut(key) {
                value.push(message);
            } else {
                mailboxes.insert(key, vec![message]);
            }
        }

        // Sort the mailboxes with ones containing the newest messages first
        let mut mailboxes = mailboxes
            .into_iter()
            .map(|(name, messages)| Mailbox::new(name, messages))
            .collect::<Vec<_>>();
        mailboxes.sort_by_key(|mailbox| (-mailbox.most_recent_timestamp, mailbox.name));

        let max_lines = std::cmp::min(
            mailboxes
                .iter()
                .map(|mailbox| mailbox.messages.len())
                .sum::<usize>(),
            self.max_lines.unwrap_or(std::usize::MAX),
        );

        // Distribute the available lines to the mailboxes as evenly as possible
        let mut line = 0;
        while line < max_lines {
            for mailbox in mailboxes.iter_mut() {
                if mailbox.allocated_lines < mailbox.messages.len() {
                    mailbox.allocated_lines += 1;
                    line += 1;
                }

                if line >= max_lines {
                    // We allocated the last line, so abort
                    break;
                }
            }
        }

        // If there aren't enough lines to show each mailbox on its own line,
        // reserve one line for the hidden mailboxes message
        let displayed_mailbox_count = if mailboxes.len() > max_lines {
            max_lines - 1
        } else {
            mailboxes.len()
        };
        let hidden_mailboxes = mailboxes
            .iter()
            .skip(displayed_mailbox_count)
            .collect::<Vec<_>>();

        // Calculate the single-line message to be displayed if mailboxes were hidden
        let hidden_mailboxes_message = if hidden_mailboxes.is_empty() {
            None
        } else {
            let hidden_message_count = hidden_mailboxes
                .iter()
                .map(|mailbox| mailbox.messages.len())
                .sum::<usize>();
            Some(format!(
                "(+{} older messages in {})\n",
                hidden_message_count,
                Self::summarize_hidden_mailboxes(hidden_mailboxes),
            ))
        };

        // For each mailbox, display the allocated number of messages
        mailboxes
            .iter()
            .take(displayed_mailbox_count)
            .flat_map(|mailbox| {
                let hidden_message_count = mailbox.messages.len() - mailbox.allocated_lines;
                mailbox
                    .messages
                    .iter()
                    .take(mailbox.allocated_lines)
                    .enumerate()
                    .map(move |(index, message)| {
                        // At the end of the final displayed message in the
                        // mailbox, signify that messages were hidden
                        let hidden_messages_hint =
                            if hidden_message_count > 0 && index == mailbox.allocated_lines - 1 {
                                Some(format!(
                                    " (+{} older {})",
                                    hidden_message_count,
                                    Self::pluralize_word(Word::Message, hidden_message_count)
                                ))
                            } else {
                                None
                            };
                        self.format_message(message, hidden_messages_hint) + "\n"
                    })
            })
            .collect::<Vec<_>>()
            .join("")
            + &hidden_mailboxes_message.unwrap_or_else(|| "".to_string())
    }

    // Pluralize a word if count is not 1
    fn pluralize_word(word: Word, count: usize) -> &'static str {
        match (word, count) {
            (Word::Mailbox, 1) => "mailbox",
            (Word::Mailbox, _) => "mailboxes",
            (Word::Message, 1) => "message",
            (Word::Message, _) => "messages",
        }
    }

    // Create a human-readable single-line summary of the mailboxes that were hidden
    fn summarize_hidden_mailboxes(hidden_mailboxes: Vec<&Mailbox>) -> String {
        let count = hidden_mailboxes.len();
        if count == 0 {
            "".to_string()
        } else if count == 1 {
            hidden_mailboxes[0].name.to_string()
        } else if count == 2 {
            format!(
                "{} and {}",
                hidden_mailboxes[0].name, hidden_mailboxes[1].name
            )
        } else if count == 3 {
            format!(
                "{}, {}, and {}",
                hidden_mailboxes[0].name, hidden_mailboxes[1].name, hidden_mailboxes[2].name,
            )
        } else {
            format!(
                "{}, {}, and {} other {}",
                hidden_mailboxes[0].name,
                hidden_mailboxes[1].name,
                hidden_mailboxes.len() - 2,
                Self::pluralize_word(Word::Mailbox, hidden_mailboxes.len() - 2)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::NaiveDateTime;

    // Helper for creating a new message
    fn make_message(mailbox: &str, content: &str, timestamp_offset: i64) -> Message {
        Message {
            id: 1,
            timestamp: NaiveDateTime::from_timestamp(1640995200 + timestamp_offset, 0),
            mailbox: mailbox.into(),
            content: content.into(),
            state: MessageState::Unread,
        }
    }

    // Create a generic message formatter
    fn make_formatter() -> MessageFormatter {
        MessageFormatter::new()
            .with_color(false)
            .with_timestamp_format(TimestampFormat::Utc)
    }

    #[test]
    fn test_format() {
        let messages = vec![make_message("a", "foo", 0)];
        let formatter = make_formatter();
        assert_eq!(
            formatter.format_messages(&messages).as_str(),
            "* foo [a] @ 2022-01-01 00:00:00 UTC\n"
        );
    }

    #[test]
    fn test_empty() {
        let messages = vec![];
        let formatter = make_formatter();
        assert_eq!(formatter.format_messages(&messages).as_str(), "");
    }

    #[test]
    fn test_ordering() {
        let messages = vec![
            make_message("foo", "a", 0),
            make_message("foo", "b", 2),
            make_message("foo", "c", 0),
            make_message("foo", "d", 0),
            make_message("foo", "e", 1),
            make_message("foo", "f", 0),
            make_message("foo", "g", 1),
        ];
        let formatter = make_formatter();
        assert_eq!(
            formatter.format_messages(&messages).as_str(),
            "* b [foo] @ 2022-01-01 00:00:02 UTC
* e [foo] @ 2022-01-01 00:00:01 UTC
* g [foo] @ 2022-01-01 00:00:01 UTC
* a [foo] @ 2022-01-01 00:00:00 UTC
* c [foo] @ 2022-01-01 00:00:00 UTC
* d [foo] @ 2022-01-01 00:00:00 UTC
* f [foo] @ 2022-01-01 00:00:00 UTC\n"
        );
    }

    #[test]
    fn test_truncate_content() {
        let formatter = make_formatter().with_max_columns(Some(60));
        assert_eq!(
            formatter
                .format_message(
                    &make_message(
                        "foo",
                        "Lorem ipsum dolor sit amet, consectetur adipiscing elit",
                        0
                    ),
                    Some(" appendix".to_string())
                )
                .as_str(),
            "* Lorem ipsum dolo… [foo] @ 2022-01-01 00:00:00 UTC appendix"
        );
    }

    #[test]
    fn test_truncate_mailbox() {
        let formatter = make_formatter().with_max_columns(Some(60));
        assert_eq!(
            formatter
                .format_message(
                    &make_message(
                        "really-really-really-really-really-really-really-really-long",
                        "a",
                        0
                    ),
                    Some(" appendix".to_string())
                )
                .as_str(),
            "* a [really-really-real…] @ 2022-01-01 00:00:00 UTC appendix"
        );
    }

    #[test]
    fn test_truncate_mailbox_and_content() {
        let formatter = make_formatter().with_max_columns(Some(60));
        assert_eq!(
            formatter
                .format_message(
                    &make_message(
                        "really-really-really-really-really-really-really-really-long",
                        "Lorem ipsum dolor sit amet, consectetur adipiscing elit",
                        0
                    ),
                    Some(" appendix".to_string())
                )
                .as_str(),
            "* Lorem ips… [really-re…] @ 2022-01-01 00:00:00 UTC appendix"
        );
    }

    #[test]
    fn test_truncate_all() {
        let formatter = make_formatter().with_max_columns(Some(20));
        assert_eq!(
            formatter
                .format_message(&make_message(
                    "really-really-really-really-really-really-really-really-long",
                    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
                    0
                ), Some("appendix".to_string()))
                .as_str(),
            "* Lor… [rea…] @ 202…"
        );
    }

    #[test]
    fn test_summarize_many_mailboxes() {
        let messages = vec![
            make_message("a", "foo", 0),
            make_message("b", "foo", 2),
            make_message("c", "foo", 0),
            make_message("d", "foo", 0),
            make_message("e", "foo", 1),
            make_message("f", "foo", 0),
            make_message("g", "foo", 1),
        ];
        let formatter = make_formatter().with_max_lines(Some(4));
        assert_eq!(
            formatter.format_messages(&messages).as_str(),
            "* foo [b] @ 2022-01-01 00:00:02 UTC
* foo [e] @ 2022-01-01 00:00:01 UTC
* foo [g] @ 2022-01-01 00:00:01 UTC
(+4 older messages in a, c, and 2 other mailboxes)\n"
        );
    }

    #[test]
    fn test_summarize_large_mailbox() {
        let messages = vec![
            make_message("foo", "a", 0),
            make_message("foo", "b", 2),
            make_message("foo", "c", 0),
            make_message("foo", "d", 0),
            make_message("foo", "e", 1),
            make_message("foo", "f", 0),
            make_message("foo", "g", 1),
        ];
        let formatter = make_formatter().with_max_lines(Some(4));
        assert_eq!(
            formatter.format_messages(&messages).as_str(),
            "* b [foo] @ 2022-01-01 00:00:02 UTC
* e [foo] @ 2022-01-01 00:00:01 UTC
* g [foo] @ 2022-01-01 00:00:01 UTC
* a [foo] @ 2022-01-01 00:00:00 UTC (+3 older messages)\n"
        );
    }

    #[test]
    fn test_summarize_large_and_many_mailboxes() {
        let messages = vec![
            make_message("foo", "a", 0),
            make_message("foo", "b", 2),
            make_message("foo", "c", 0),
            make_message("foo", "d", 0),
            make_message("foo", "e", 1),
            make_message("foo", "f", 0),
            make_message("foo", "g", 1),
            make_message("bar1", "a", 0),
            make_message("bar2", "b", 0),
            make_message("bar3", "c", 0),
            make_message("bar4", "d", 0),
            make_message("bar5", "e", 0),
            make_message("bar6", "f", 0),
            make_message("bar7", "g", 0),
        ];
        let formatter = make_formatter().with_max_lines(Some(4));
        assert_eq!(
            formatter.format_messages(&messages).as_str(),
            "* b [foo] @ 2022-01-01 00:00:02 UTC (+6 older messages)
* a [bar1] @ 2022-01-01 00:00:00 UTC
* b [bar2] @ 2022-01-01 00:00:00 UTC
(+5 older messages in bar3, bar4, and 3 other mailboxes)\n"
        );
    }

    #[test]
    fn test_summarize_three_large_mailboxes() {
        let messages = vec![
            make_message("foo", "a", 0),
            make_message("foo", "b", 2),
            make_message("foo", "c", 0),
            make_message("foo", "d", 0),
            make_message("bar", "e", 1),
            make_message("bar", "f", 0),
            make_message("bar", "g", 1),
        ];
        let formatter = make_formatter().with_max_lines(Some(4));
        assert_eq!(
            formatter.format_messages(&messages).as_str(),
            "* b [foo] @ 2022-01-01 00:00:02 UTC
* a [foo] @ 2022-01-01 00:00:00 UTC (+2 older messages)
* e [bar] @ 2022-01-01 00:00:01 UTC
* g [bar] @ 2022-01-01 00:00:01 UTC (+1 older message)\n"
        );
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(
            MessageFormatter::pluralize_word(Word::Mailbox, 1),
            "mailbox"
        );
        assert_eq!(
            MessageFormatter::pluralize_word(Word::Mailbox, 0),
            "mailboxes"
        );
        assert_eq!(
            MessageFormatter::pluralize_word(Word::Mailbox, 3),
            "mailboxes"
        );

        assert_eq!(
            MessageFormatter::pluralize_word(Word::Message, 1),
            "message"
        );
        assert_eq!(
            MessageFormatter::pluralize_word(Word::Message, 0),
            "messages"
        );
        assert_eq!(
            MessageFormatter::pluralize_word(Word::Message, 3),
            "messages"
        );
    }
}
