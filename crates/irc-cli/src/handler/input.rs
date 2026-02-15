//! Keyboard input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::input::InputState;

/// Input mode for vim-style navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Normal text input mode.
    #[default]
    Insert,
    /// Vim-style navigation mode (message scrolling).
    Normal,
    /// Search mode (/ in vim).
    Search,
}

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
    /// Jump to buffer by index (1-9).
    JumpToBuffer(usize),
    /// Scroll up.
    ScrollUp(usize),
    /// Scroll down.
    ScrollDown(usize),
    /// Scroll to top.
    ScrollTop,
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
    /// Close current buffer.
    CloseBuffer,
    /// Enter search mode.
    EnterSearch,
    /// Exit search mode.
    ExitSearch,
    /// Search next match.
    SearchNext,
    /// Search previous match.
    SearchPrev,
    /// Toggle sidebar visibility.
    ToggleSidebar,
    /// Toggle user filter mode in sidebar.
    ToggleUserFilter,
    /// Open command palette.
    OpenCommandPalette,
}

/// Tracks 'g' key for gg command in vim mode.
#[derive(Debug, Default)]
pub struct VimState {
    /// Pending 'g' for gg command.
    pub pending_g: bool,
}

/// Handle a key event, updating input state and returning any action.
#[allow(dead_code)]
pub fn handle_key_event(event: KeyEvent, input: &mut InputState) -> KeyAction {
    handle_key_event_with_mode(event, input, InputMode::Insert, &mut VimState::default())
}

/// Handle a key event with vim mode support.
pub fn handle_key_event_with_mode(
    event: KeyEvent,
    input: &mut InputState,
    mode: InputMode,
    vim_state: &mut VimState,
) -> KeyAction {
    // Handle vim normal mode
    if mode == InputMode::Normal {
        return handle_vim_normal_mode(event, vim_state);
    }

    // Handle vim search mode
    if mode == InputMode::Search {
        return handle_vim_search_mode(event, input);
    }

    // Insert mode (normal text input)
    match (event.code, event.modifiers) {
        // Help
        (KeyCode::F(1), _) => KeyAction::ToggleHelp,
        (KeyCode::Char('?'), KeyModifiers::NONE) if input.text.is_empty() => KeyAction::ToggleHelp,

        // Esc with empty input in vim-enabled mode can switch to normal mode
        // This is handled at the app level since we need to know vim_mode setting
        (KeyCode::Esc, _) => KeyAction::CloseHelp,

        // Submit
        (KeyCode::Enter, _) => {
            let text = input.submit();
            if text.is_empty() {
                // Empty enter can be used to scroll to bottom
                KeyAction::ScrollBottom
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

        // Alt+1-9 for buffer jumping
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::ALT) => {
            KeyAction::JumpToBuffer((c as usize) - ('1' as usize))
        }

        // Search mode (Ctrl+F)
        (KeyCode::Char('f'), KeyModifiers::CONTROL) => KeyAction::EnterSearch,

        // Toggle sidebar (Alt+S)
        (KeyCode::Char('s'), KeyModifiers::ALT) => KeyAction::ToggleSidebar,

        // Toggle user filter (Alt+F)
        (KeyCode::Char('f'), KeyModifiers::ALT) => KeyAction::ToggleUserFilter,

        // Command palette (Ctrl+K)
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => KeyAction::OpenCommandPalette,

        // Scrolling
        (KeyCode::PageUp, _) => KeyAction::ScrollUp(10),
        (KeyCode::PageDown, _) => KeyAction::ScrollDown(10),
        (KeyCode::Home, KeyModifiers::CONTROL) => KeyAction::ScrollTop,
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

/// Handle keys in vim normal mode.
fn handle_vim_normal_mode(event: KeyEvent, vim_state: &mut VimState) -> KeyAction {
    // Check for pending 'g' for gg command
    if vim_state.pending_g {
        vim_state.pending_g = false;
        if event.code == KeyCode::Char('g') {
            return KeyAction::ScrollTop;
        }
        // Not gg, ignore the pending g
    }

    match (event.code, event.modifiers) {
        // Return to insert mode
        (KeyCode::Char('i'), KeyModifiers::NONE) => {
            // This signals to switch back to insert mode
            // The app handles the actual mode switch
            KeyAction::None
        }

        // Scrolling
        (KeyCode::Char('j'), KeyModifiers::NONE) => KeyAction::ScrollDown(1),
        (KeyCode::Char('k'), KeyModifiers::NONE) => KeyAction::ScrollUp(1),
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => KeyAction::ScrollDown(10), // Half page
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => KeyAction::ScrollUp(10),   // Half page
        (KeyCode::Char('G'), KeyModifiers::SHIFT) => KeyAction::ScrollBottom,
        (KeyCode::Char('g'), KeyModifiers::NONE) => {
            vim_state.pending_g = true;
            KeyAction::None
        }

        // Search
        (KeyCode::Char('/'), KeyModifiers::NONE) => KeyAction::EnterSearch,
        (KeyCode::Char('n'), KeyModifiers::NONE) => KeyAction::SearchNext,
        (KeyCode::Char('N'), KeyModifiers::SHIFT) => KeyAction::SearchPrev,

        // Buffer operations
        (KeyCode::Char('q'), KeyModifiers::NONE) => KeyAction::CloseBuffer,

        // Escape goes back to insert mode
        (KeyCode::Esc, _) => KeyAction::None, // App handles mode switch

        // Buffer switching with Alt
        (KeyCode::Char('j'), KeyModifiers::ALT) => KeyAction::NextBuffer,
        (KeyCode::Char('k'), KeyModifiers::ALT) => KeyAction::PrevBuffer,

        // Alt+1-9 for buffer jumping
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::ALT) => {
            KeyAction::JumpToBuffer((c as usize) - ('1' as usize))
        }

        // Standard quit
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => KeyAction::Quit,

        _ => KeyAction::None,
    }
}

/// Handle keys in vim search mode.
fn handle_vim_search_mode(event: KeyEvent, input: &mut InputState) -> KeyAction {
    match (event.code, event.modifiers) {
        // Exit search
        (KeyCode::Esc, _) => KeyAction::ExitSearch,

        // Execute search / next match
        (KeyCode::Enter, KeyModifiers::SHIFT) => KeyAction::SearchPrev,
        (KeyCode::Enter, _) => KeyAction::SearchNext,

        // Navigate matches
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => KeyAction::SearchNext,
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => KeyAction::SearchPrev,

        // Editing search query
        (KeyCode::Backspace, _) => {
            input.backspace();
            KeyAction::None
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            input.clear();
            KeyAction::None
        }

        // Character input for search
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            input.insert(c);
            KeyAction::None
        }

        _ => KeyAction::None,
    }
}
