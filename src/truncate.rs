use colored::{ColoredString, Colorize};
use unicode_segmentation::UnicodeSegmentation;

// Represents a line of characters with a max length that can be built up over time
pub struct TruncatedLine {
    // The number of available columns in the line
    remaining_columns: usize,

    // The current string contents of the line
    current_line: String,
}

impl TruncatedLine {
    // Create a new instance
    pub fn new(max_columns: usize) -> Self {
        TruncatedLine {
            remaining_columns: max_columns,
            current_line: String::new(),
        }
    }

    // Add more characters to the line, enforcing the maximum line length
    pub fn append(
        &mut self,
        new_chars: impl Into<String>,
        colorize: Option<fn(String) -> ColoredString>,
    ) {
        fn no_color(str: String) -> ColoredString {
            str.normal()
        }

        let truncated = truncate_string(new_chars.into(), self.remaining_columns);
        self.remaining_columns -= truncated.graphemes(true).count();
        let colorize = colorize.unwrap_or(no_color);
        self.current_line = format!("{}{}", self.current_line, colorize(truncated));
    }
}

impl ToString for TruncatedLine {
    fn to_string(&self) -> String {
        self.current_line.clone()
    }
}

pub fn truncate_string(string: String, max_length: usize) -> String {
    if max_length == 0 {
        "".into()
    } else if string.len() <= max_length {
        string
    } else {
        format!("{string:.*}…", max_length - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_length() {
        let mut line = TruncatedLine::new(0);
        line.append("hello", None);
        assert_eq!(line.to_string(), "");
    }

    #[test]
    fn test_truncate() {
        let mut line = TruncatedLine::new(4);
        line.append("hello", None);
        assert_eq!(line.to_string(), "hel…");
    }

    #[test]
    fn test_no_truncate() {
        let mut line = TruncatedLine::new(20);
        line.append("hello", None);
        assert_eq!(line.to_string(), "hello");
        assert_eq!(line.remaining_columns, 15);
    }

    #[test]
    fn test_multiple_append() {
        let mut line = TruncatedLine::new(9);
        line.append("hello ", None);
        line.append("world", None);
        assert_eq!(line.to_string(), "hello wo…");
    }

    #[test]
    fn test_colored() {
        // Temporarily use colors, even in CI
        colored::control::set_override(true);

        let mut line = TruncatedLine::new(11);
        line.append("hello ", None);
        line.append("world", Some(|str| str.red()));
        assert_eq!(line.to_string(), "hello \u{1b}[31mworld\u{1b}[0m");

        colored::control::unset_override();
    }

    #[test]
    fn test_truncate_string() {
        let message = "Hello, world!";
        assert_eq!(truncate_string(message.to_string(), 0), "");
        assert_eq!(truncate_string(message.to_string(), 6), "Hello…");
        assert_eq!(truncate_string(message.to_string(), 13), "Hello, world!");
        assert_eq!(truncate_string(message.to_string(), 20), "Hello, world!");
    }
}
