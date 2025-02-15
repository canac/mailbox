use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// Represents a input event and contains all the information needed to handle that event. It is usually constructed from
// a KeyEvent but can also be constructed synthetically.
pub struct InputEvent {
    pub key: KeyCode,
    pub control: bool,
}

impl InputEvent {
    // Create an input event representing a specific key code
    pub fn from_key(key: KeyCode) -> Self {
        Self {
            key,
            control: false,
        }
    }

    // Create an input event from a crossterm KeyEvent
    pub fn from_event(event: KeyEvent) -> Self {
        Self {
            key: event.code,
            control: event.modifiers.contains(KeyModifiers::CONTROL),
        }
    }
}
