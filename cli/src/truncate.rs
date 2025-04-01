use colored::{ColoredString, Colorize};
use std::fmt::{self, Display, Formatter};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

type ApplyColor = fn(&str) -> ColoredString;

fn no_color(str: &str) -> ColoredString {
    str.normal()
}

// Represents a line of characters with a max length that can be built up over time
pub struct TruncatedLine {
    // The number of available columns in the line
    available_columns: usize,

    // The sections that make up the truncated line
    sections: Vec<(String, ApplyColor)>,
}

impl TruncatedLine {
    // Create a new instance
    pub fn new(max_columns: usize) -> Self {
        Self {
            available_columns: max_columns,
            sections: Vec::default(),
        }
    }

    // Add more characters to the line, enforcing the maximum line length
    pub fn append(&mut self, new_chars: impl Into<String>, colorize: Option<ApplyColor>) {
        let new_chars: String = new_chars.into();
        if !new_chars.is_empty() {
            self.sections
                .push((new_chars, colorize.unwrap_or(no_color)));
        }
    }

    // Generate the string representation of this line
    fn generate(&self) -> String {
        let mut line = String::new();

        let mut remaining_columns = self.available_columns;
        for (index, (new_chars, colorize)) in self.sections.iter().enumerate() {
            // If this section exactly fits in the remaining columns, but there
            // are still other sections remaining, then force truncation
            let force_truncate =
                remaining_columns == new_chars.width() && index != self.sections.len() - 1;
            // Force truncation if necessary by adding an extra character that
            // will be truncated off
            let (truncated, width) = truncate_string(
                &format!("{new_chars}{}", if force_truncate { " " } else { "" }),
                remaining_columns,
            );
            remaining_columns -= width;
            line = format!(
                "{}{}",
                line,
                if truncated.is_empty() {
                    // Don't add color codes to empty strings
                    no_color("")
                } else {
                    colorize(&truncated)
                }
            );

            // remaining_columns might be non-zero if new_chars contained
            // double-width characters, so abort since we don't want to add
            // anything else
            if force_truncate {
                return line;
            }
        }

        line
    }
}

impl Display for TruncatedLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.generate())
    }
}

// Truncate the input string to fit within a given width, taking
// non-single-width Unicode characters into account
// Returns the truncated string and its width
// Uses the algorithm in https://github.com/Aetf/unicode-truncate with added
// support for adding ellipses when the string is truncated
pub fn truncate_string(input: &str, width: usize) -> (String, usize) {
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
            input.get(..byte_index).unwrap_or_default(),
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
    }

    #[test]
    fn test_multiple_append() {
        let mut line = TruncatedLine::new(9);
        line.append("hello ", None);
        line.append("world", None);
        assert_eq!(line.to_string(), "hello wo…");
    }

    #[test]
    fn test_multiple_append_trailing_empty() {
        let mut line = TruncatedLine::new(11);
        line.append("hello ", None);
        line.append("world", None);
        line.append("", None);
        assert_eq!(line.to_string(), "hello world");
    }

    #[test]
    fn test_multiple_append_first_fills() {
        let mut line = TruncatedLine::new(6);
        line.append("hello ", None);
        line.append("world", None);
        assert_eq!(line.to_string(), "hello…");
    }

    #[test]
    fn test_multiple_append_first_fills_double_width() {
        let mut line = TruncatedLine::new(6);
        line.append("⭐⭐⭐", None);
        line.append("world", None);
        assert_eq!(line.to_string(), "⭐⭐…");
    }

    #[test]
    fn test_colored() {
        let mut line = TruncatedLine::new(11);
        line.append("hello ", None);
        line.append("world", Some(|str| str.red()));
        assert_eq!(line.to_string(), "hello \u{1b}[31mworld\u{1b}[0m");
    }

    #[test]
    fn test_colored_empty_string() {
        let mut line = TruncatedLine::new(5);
        line.append("hello ", None);
        line.append("world", Some(|str| str.red()));
        assert_eq!(line.to_string(), "hell…");
    }

    #[test]
    fn test_truncate_string() {
        let message = "Hello, world!";
        assert_eq!(truncate_string(message, 0), (String::new(), 0));
        assert_eq!(truncate_string(message, 6), (String::from("Hello…"), 6));
        assert_eq!(
            truncate_string(message, 13),
            (String::from("Hello, world!"), 13)
        );
        assert_eq!(
            truncate_string(message, 20),
            (String::from("Hello, world!"), 13)
        );
    }

    #[test]
    fn test_truncate_string_unicode() {
        let message = "⭐a⭐b⭐c⭐";
        assert_eq!(truncate_string(message, 0), (String::new(), 0));
        assert_eq!(truncate_string(message, 5), (String::from("⭐a…"), 4));
        assert_eq!(truncate_string(message, 6), (String::from("⭐a⭐…"), 6));
        assert_eq!(
            truncate_string(message, 11),
            (String::from("⭐a⭐b⭐c⭐"), 11)
        );
        assert_eq!(
            truncate_string(message, 20),
            (String::from("⭐a⭐b⭐c⭐"), 11)
        );
    }
}
