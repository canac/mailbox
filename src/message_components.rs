use crate::message::MessageState;
use crate::truncate::truncate_string;
use std::cmp::max;

// Represents the individual components of a message that will be displayed to
// the user. This struct exists to allow message components to be independently
// truncated when the message gets too long.
pub struct MessageComponents {
    pub state: MessageState,
    pub content: String,
    pub mailbox: String,
    pub time: String,
    pub appendix: String,
}

impl MessageComponents {
    // Attempt to truncate the combined length of the message components down
    // to max_length. If this isn't possible, the message components will be
    // truncated as much as possible.
    pub fn truncate(self, max_length: usize) -> MessageComponents {
        let total_length =
            8 + self.content.len() + self.mailbox.len() + self.time.len() + self.appendix.len();
        if total_length <= max_length {
            // The message doesn't need truncation
            return self;
        }

        // First try to truncate the mailbox
        let others_length = total_length - self.mailbox.len();
        if others_length + 4 <= max_length {
            let mailbox = truncate_string(self.mailbox, max_length - others_length);
            return MessageComponents { mailbox, ..self };
        }

        // Next try to truncate the content
        let others_length = total_length - self.content.len();
        if others_length + 4 <= max_length {
            let content = truncate_string(self.content, max_length - others_length);
            return MessageComponents { content, ..self };
        }

        // Lastly, truncate the content and the mailbox
        let others_length = total_length - self.content.len() - self.mailbox.len();
        let mailbox_and_content_length = max(max_length.saturating_sub(others_length) / 2, 4);
        let mailbox = truncate_string(self.mailbox, mailbox_and_content_length);
        let content = truncate_string(self.content, mailbox_and_content_length);
        MessageComponents {
            mailbox,
            content,
            ..self
        }
    }
}
