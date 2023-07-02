use crate::cli::ImportMessageFormat;
use crate::config::Config;
use anyhow::{Context, Result};
use csv::ReaderBuilder;
use database::{Database, Message, NewMessage};
use futures::future::try_join_all;

// Import messages from stdin lines
pub fn read_messages_stdin<Stdin>(stdin: Stdin, format: ImportMessageFormat) -> Vec<NewMessage>
where
    Stdin: std::io::BufRead,
{
    let lines = stdin.lines().filter_map(|result| match result {
        Ok(line) if !line.is_empty() => Some(line),
        _ => None,
    });

    match format {
        ImportMessageFormat::Json => lines
            .map(|line| {
                serde_json::from_str(&line)
                    .with_context(|| format!("Failed to parse line as JSON:\n{line}"))
            })
            .collect::<Vec<Result<NewMessage>>>(),
        ImportMessageFormat::Tsv => {
            // ReaderBuilder needs a header row for the state column to be optional
            let lines = lines.collect::<Vec<_>>();
            let tsv = format!("mailbox\tcontent\tstate\n{}", lines.join("\n"));
            ReaderBuilder::new()
                .has_headers(true)
                .flexible(true)
                .delimiter(b'\t')
                .from_reader(tsv.as_bytes())
                .deserialize()
                .enumerate()
                .map(|(index, result)| {
                    result.with_context(|| {
                        format!(
                            "Failed to parse line as TSV:\n{}",
                            lines.get(index).unwrap_or(&String::new())
                        )
                    })
                })
                .collect::<Vec<Result<NewMessage>>>()
        }
    }
    .into_iter()
    .filter_map(|result| match result {
        Ok(message) => Some(message),
        Err(err) => {
            // Print an error if there was an error, keeping the other valid lines
            eprintln!("{err:?}");
            None
        }
    })
    .collect()
}

// Add multiple messages to the database
#[allow(clippy::module_name_repetitions)]
pub async fn import_messages(
    db: &mut Database,
    config: &Option<Config>,
    new_messages: Vec<NewMessage>,
) -> Result<Vec<Message>> {
    let futures = new_messages
        .into_iter()
        .filter_map(|message| match config.as_ref() {
            Some(config) => config.apply_override(message),
            None => Some(message),
        })
        .map(|message| db.add_message(message))
        .collect::<Vec<_>>();
    try_join_all(futures).await
}

#[cfg(test)]
mod tests {
    use database::State;

    use super::*;

    #[test]
    fn test_empty() {
        let stdin = "";
        assert!(read_messages_stdin(stdin.as_bytes(), ImportMessageFormat::Tsv).is_empty());
        assert!(read_messages_stdin(stdin.as_bytes(), ImportMessageFormat::Json).is_empty());
    }

    #[test]
    fn test_tsv() {
        let stdin = "1\na\tb\nfoo\tbar\tread\nA\tB\tC\tD";
        assert_eq!(
            read_messages_stdin(stdin.as_bytes(), ImportMessageFormat::Tsv),
            vec![
                NewMessage {
                    mailbox: String::from("a"),
                    content: String::from("b"),
                    state: None
                },
                NewMessage {
                    mailbox: String::from("foo"),
                    content: String::from("bar"),
                    state: Some(State::Read)
                }
            ]
        );
    }

    #[test]
    fn test_json() {
        let stdin = r#"{"mailbox":"1"}
{"mailbox":"a","content":"b"}
{"mailbox":"foo","content":"bar","state":"read"}
{"mailbox":"A","content":"B","unknown":"C"}"#;
        assert_eq!(
            read_messages_stdin(stdin.as_bytes(), ImportMessageFormat::Json),
            vec![
                NewMessage {
                    mailbox: String::from("a"),
                    content: String::from("b"),
                    state: None
                },
                NewMessage {
                    mailbox: String::from("foo"),
                    content: String::from("bar"),
                    state: Some(State::Read)
                }
            ]
        );
    }
}
