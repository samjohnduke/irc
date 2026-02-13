//! Input line state and widget.

use crate::completion::CompletionState;

/// Input line state.
#[derive(Debug, Clone)]
pub struct InputState {
    /// Current input text.
    pub text: String,

    /// Cursor position (byte offset).
    pub cursor: usize,

    /// Input history.
    pub history: Vec<String>,

    /// Current history index (None = not browsing).
    pub history_index: Option<usize>,

    /// Saved current input when browsing history.
    pub saved_input: Option<String>,

    /// Tab completion state.
    pub completion: CompletionState,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: None,
            completion: CompletionState::new(),
        }
    }

    /// Insert a character at the cursor.
    pub fn insert(&mut self, c: char) {
        self.completion.reset(); // Reset completion on any input
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Apply a completion to the current input.
    pub fn apply_completion(&mut self, completion: &str, start_pos: usize) {
        // Replace from start_pos to cursor with the completion
        self.text.replace_range(start_pos..self.cursor, completion);
        self.cursor = start_pos + completion.len();
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        self.completion.reset();
        if self.cursor > 0 {
            // Find the previous character boundary
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    /// Delete the character at the cursor.
    pub fn delete(&mut self) {
        self.completion.reset();
        if self.cursor < self.text.len() {
            let next = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            self.text.drain(self.cursor..next);
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
        }
    }

    /// Move cursor to start.
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn cursor_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Delete word before cursor.
    pub fn delete_word(&mut self) {
        if self.cursor > 0 {
            // Skip trailing spaces
            let mut pos = self.cursor;
            while pos > 0 {
                let prev_char = self.text[..pos].chars().last();
                if prev_char.map(|c| c.is_whitespace()).unwrap_or(false) {
                    pos = self.text[..pos]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                } else {
                    break;
                }
            }

            // Delete word characters
            while pos > 0 {
                let prev_char = self.text[..pos].chars().last();
                if prev_char.map(|c| !c.is_whitespace()).unwrap_or(false) {
                    pos = self.text[..pos]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                } else {
                    break;
                }
            }

            self.text.drain(pos..self.cursor);
            self.cursor = pos;
        }
    }

    /// Clear the input line.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_input = None;
    }

    /// Take the current input and add to history.
    pub fn submit(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.history_index = None;
        self.saved_input = None;

        // Add to history if non-empty and different from last
        if !text.is_empty() {
            if self.history.last() != Some(&text) {
                self.history.push(text.clone());
            }
        }

        text
    }

    /// Browse history up.
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Start browsing from the end
                self.saved_input = Some(self.text.clone());
                self.history_index = Some(self.history.len() - 1);
                self.text = self.history.last().cloned().unwrap_or_default();
            }
            Some(i) if i > 0 => {
                self.history_index = Some(i - 1);
                self.text = self.history[i - 1].clone();
            }
            _ => {}
        }

        self.cursor = self.text.len();
    }

    /// Browse history down.
    pub fn history_down(&mut self) {
        match self.history_index {
            Some(i) if i + 1 < self.history.len() => {
                self.history_index = Some(i + 1);
                self.text = self.history[i + 1].clone();
            }
            Some(_) => {
                // Back to current input
                self.history_index = None;
                self.text = self.saved_input.take().unwrap_or_default();
            }
            None => {}
        }

        self.cursor = self.text.len();
    }

    /// Get cursor position in characters (for display).
    #[allow(dead_code)]
    pub fn cursor_char_pos(&self) -> usize {
        self.text[..self.cursor].chars().count()
    }
}

// Note: InputWidget has been removed - rendering is now handled directly
// in layout.rs for better control over the input area styling.
