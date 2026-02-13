//! Keyboard input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::input::InputState;

/// Result of handling a key event.
#[derive(Debug)]
pub enum KeyAction {
    /// Continue running, no action needed.
    None,
    /// Submit the current input.
    Submit(String),
    /// Quit the application.
    Quit,
    /// Switch to next buffer.
    NextBuffer,
    /// Switch to previous buffer.
    PrevBuffer,
    /// Scroll up.
    ScrollUp(usize),
    /// Scroll down.
    ScrollDown(usize),
    /// Scroll to bottom.
    ScrollBottom,
    /// Tab completion requested.
    TabComplete,
    /// Reverse tab completion (Shift+Tab).
    TabCompleteReverse,
    /// Toggle help overlay.
    ToggleHelp,
    /// Close help overlay (Esc key).
    CloseHelp,
}

/// Handle a key event, updating input state and returning any action.
pub fn handle_key_event(event: KeyEvent, input: &mut InputState) -> KeyAction {
    match (event.code, event.modifiers) {
        // Help
        (KeyCode::F(1), _) => KeyAction::ToggleHelp,
        (KeyCode::Char('?'), KeyModifiers::NONE) if input.text.is_empty() => KeyAction::ToggleHelp,
        (KeyCode::Esc, _) => KeyAction::CloseHelp, // Close help with Esc (only if open)

        // Submit
        (KeyCode::Enter, _) => {
            let text = input.submit();
            if text.is_empty() {
                KeyAction::None
            } else {
                KeyAction::Submit(text)
            }
        }

        // Quit
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => KeyAction::Quit,
        (KeyCode::Char('d'), KeyModifiers::CONTROL) if input.text.is_empty() => KeyAction::Quit,

        // Tab completion
        (KeyCode::Tab, KeyModifiers::NONE) => KeyAction::TabComplete,
        (KeyCode::BackTab, _) => KeyAction::TabCompleteReverse,

        // Buffer switching
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => KeyAction::NextBuffer,
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => KeyAction::PrevBuffer,
        (KeyCode::Down, KeyModifiers::ALT) => KeyAction::NextBuffer,
        (KeyCode::Up, KeyModifiers::ALT) => KeyAction::PrevBuffer,
        (KeyCode::Right, KeyModifiers::ALT) => KeyAction::NextBuffer,
        (KeyCode::Left, KeyModifiers::ALT) => KeyAction::PrevBuffer,

        // Scrolling
        (KeyCode::PageUp, _) => KeyAction::ScrollUp(10),
        (KeyCode::PageDown, _) => KeyAction::ScrollDown(10),
        (KeyCode::Home, KeyModifiers::CONTROL) => KeyAction::ScrollUp(usize::MAX),
        (KeyCode::End, KeyModifiers::CONTROL) => KeyAction::ScrollBottom,

        // Editing
        (KeyCode::Backspace, _) => {
            input.backspace();
            KeyAction::None
        }
        (KeyCode::Delete, _) => {
            input.delete();
            KeyAction::None
        }
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            input.delete_word();
            KeyAction::None
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            input.clear();
            KeyAction::None
        }

        // Cursor movement
        (KeyCode::Left, KeyModifiers::NONE) => {
            input.cursor_left();
            KeyAction::None
        }
        (KeyCode::Right, KeyModifiers::NONE) => {
            input.cursor_right();
            KeyAction::None
        }
        (KeyCode::Home, KeyModifiers::NONE) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
            input.cursor_home();
            KeyAction::None
        }
        (KeyCode::End, KeyModifiers::NONE) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
            input.cursor_end();
            KeyAction::None
        }

        // History
        (KeyCode::Up, KeyModifiers::NONE) => {
            input.history_up();
            KeyAction::None
        }
        (KeyCode::Down, KeyModifiers::NONE) => {
            input.history_down();
            KeyAction::None
        }

        // Character input
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            input.insert(c);
            KeyAction::None
        }

        _ => KeyAction::None,
    }
}
