use colored::{ColoredString, Colorize};
use unicode_width::UnicodeWidthChar;

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

        let (truncated, width) = truncate_string(new_chars.into(), self.remaining_columns);
        self.remaining_columns -= width;
        let colorize = colorize.unwrap_or(no_color);
        self.current_line = format!("{}{}", self.current_line, colorize(truncated));
    }
}

impl ToString for TruncatedLine {
    fn to_string(&self) -> String {
        self.current_line.clone()
    }
}

// Truncate the input string to fit within a given width, taking
// non-single-width Unicode characters into account
// Returns the truncated string and its width
// Uses the algorithm in https://github.com/Aetf/unicode-truncate with added
// support for adding ellipses when the string is truncated
pub fn truncate_string(input: String, width: usize) -> (String, usize) {
    let (add_ellipsis, byte_index, new_width) = input
        .char_indices()
        // Map to byte index and the width of the substring starting at the index
        .map(|(byte_index, char)| (true, byte_index, char.width().unwrap_or(0)))
        // Append a final element representing the position past the last char
        .chain(std::iter::once((false, input.len(), 0)))
        // Calculate the total substring width for each element
        .scan(0, |substring_width, (last, byte_index, char_width)| {
            let current_width = *substring_width;
            *substring_width += char_width;
            Some((last, byte_index, current_width))
        })
        // Ignore substrings that exceed the desired width
        .take_while(|&(add_ellipsis, _, substring_width)| {
            if add_ellipsis {
                // Reserve an extra character for the ellipse
                substring_width < width
            } else {
                substring_width <= width
            }
        })
        // Take the longest possible substring
        .last()
        .unwrap_or((false, 0, 0));
    (
        format!(
            "{}{}",
            input.get(..byte_index).unwrap(),
            if add_ellipsis { "…" } else { "" }
        ),
        if add_ellipsis {
            new_width + 1
        } else {
            new_width
        },
    )
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
    fn test_truncate_double_width() {
        let mut line = TruncatedLine::new(4);
        line.append("⭐⭐⭐", None);
        assert_eq!(line.to_string(), "⭐…");
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
        assert_eq!(truncate_string(message.to_string(), 0), ("".to_string(), 0));
        assert_eq!(
            truncate_string(message.to_string(), 6),
            ("Hello…".to_string(), 6)
        );
        assert_eq!(
            truncate_string(message.to_string(), 13),
            ("Hello, world!".to_string(), 13)
        );
        assert_eq!(
            truncate_string(message.to_string(), 20),
            ("Hello, world!".to_string(), 13)
        );
    }

    #[test]
    fn test_truncate_string_unicode() {
        let message = "⭐a⭐b⭐c⭐";
        assert_eq!(truncate_string(message.to_string(), 0), ("".to_string(), 0));
        assert_eq!(
            truncate_string(message.to_string(), 5),
            ("⭐a…".to_string(), 4)
        );
        assert_eq!(
            truncate_string(message.to_string(), 6),
            ("⭐a⭐…".to_string(), 6)
        );
        assert_eq!(
            truncate_string(message.to_string(), 11),
            ("⭐a⭐b⭐c⭐".to_string(), 11)
        );
        assert_eq!(
            truncate_string(message.to_string(), 20),
            ("⭐a⭐b⭐c⭐".to_string(), 11)
        );
    }
}
