//! Tab completion support.

/// Available commands for completion.
pub const COMMANDS: &[&str] = &[
    "away",
    "clear",
    "close",
    "disconnect",
    "help",
    "history",
    "invite",
    "join",
    "kick",
    "me",
    "msg",
    "nick",
    "part",
    "query",
    "quit",
    "raw",
    "reconnect",
    "topic",
];

/// Completion state for cycling through candidates.
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    /// The original prefix being completed.
    prefix: String,

    /// Start position in the input text.
    start_pos: usize,

    /// End position in the input text (before completion).
    end_pos: usize,

    /// List of candidates.
    candidates: Vec<String>,

    /// Current index in candidates.
    index: usize,

    /// Whether we're actively completing.
    active: bool,
}

impl CompletionState {
    /// Create a new completion state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new completion with the given candidates.
    pub fn start(
        &mut self,
        prefix: String,
        start_pos: usize,
        end_pos: usize,
        candidates: Vec<String>,
    ) {
        if candidates.is_empty() {
            self.reset();
            return;
        }

        self.prefix = prefix;
        self.start_pos = start_pos;
        self.end_pos = end_pos;
        self.candidates = candidates;
        self.index = 0;
        self.active = true;
    }

    /// Get the current completion candidate.
    pub fn current(&self) -> Option<&str> {
        if self.active {
            self.candidates.get(self.index).map(String::as_str)
        } else {
            None
        }
    }

    /// Cycle to the next candidate.
    pub fn next(&mut self) -> Option<&str> {
        if !self.active || self.candidates.is_empty() {
            return None;
        }

        self.index = (self.index + 1) % self.candidates.len();
        self.current()
    }

    /// Cycle to the previous candidate.
    pub fn prev(&mut self) -> Option<&str> {
        if !self.active || self.candidates.is_empty() {
            return None;
        }

        if self.index == 0 {
            self.index = self.candidates.len() - 1;
        } else {
            self.index -= 1;
        }
        self.current()
    }

    /// Check if completion is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the start position.
    pub fn start_pos(&self) -> usize {
        self.start_pos
    }

    /// Reset completion state.
    pub fn reset(&mut self) {
        self.prefix.clear();
        self.start_pos = 0;
        self.end_pos = 0;
        self.candidates.clear();
        self.index = 0;
        self.active = false;
    }
}

/// Context needed for completion.
pub struct CompletionContext<'a> {
    /// Channel members (nicks) for the current buffer.
    pub members: &'a [String],

    /// All buffer names.
    pub buffers: &'a [String],
}

/// Find the word being completed and its position.
pub fn find_completion_word(text: &str, cursor: usize) -> (usize, &str) {
    // Find the start of the current word
    let before_cursor = &text[..cursor];
    let word_start = before_cursor
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);

    (word_start, &text[word_start..cursor])
}

/// Generate completion candidates based on the prefix and context.
pub fn get_candidates(prefix: &str, context: &CompletionContext) -> Vec<String> {
    if prefix.is_empty() {
        return Vec::new();
    }

    // Command completion (starts with /)
    if prefix.starts_with('/') {
        let cmd_prefix = &prefix[1..].to_lowercase();
        return COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(cmd_prefix))
            .map(|cmd| format!("/{}", cmd))
            .collect();
    }

    // Channel completion (starts with # or &)
    if prefix.starts_with('#') || prefix.starts_with('&') {
        let prefix_lower = prefix.to_lowercase();
        return context
            .buffers
            .iter()
            .filter(|buf| buf.to_lowercase().starts_with(&prefix_lower))
            .cloned()
            .collect();
    }

    // Nick completion
    let prefix_lower = prefix.to_lowercase();
    let mut candidates: Vec<String> = context
        .members
        .iter()
        .filter(|nick| nick.to_lowercase().starts_with(&prefix_lower))
        .cloned()
        .collect();

    // Sort by length (shorter first) then alphabetically
    candidates.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));

    candidates
}

/// Format a nick completion (add colon if at start of line).
pub fn format_nick_completion(nick: &str, at_start: bool) -> String {
    if at_start {
        format!("{}: ", nick)
    } else {
        format!("{} ", nick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_completion_word() {
        let (start, word) = find_completion_word("hello wor", 9);
        assert_eq!(start, 6);
        assert_eq!(word, "wor");

        let (start, word) = find_completion_word("/joi", 4);
        assert_eq!(start, 0);
        assert_eq!(word, "/joi");
    }

    #[test]
    fn test_command_completion() {
        let ctx = CompletionContext {
            members: &[],
            buffers: &[],
        };

        let candidates = get_candidates("/jo", &ctx);
        assert!(candidates.contains(&"/join".to_string()));
    }

    #[test]
    fn test_nick_completion() {
        let ctx = CompletionContext {
            members: &["alice".to_string(), "bob".to_string(), "albert".to_string()],
            buffers: &[],
        };

        // Sorted by length (shorter first), then alphabetically
        let candidates = get_candidates("al", &ctx);
        assert_eq!(candidates, vec!["alice".to_string(), "albert".to_string()]);
    }
}
