//! IRCv3 message tags.
//!
//! Message tags provide metadata for IRC messages, such as timestamps,
//! message IDs, and account information.

use std::collections::HashMap;
use std::fmt;

use crate::ParseError;

/// IRCv3 message tags.
///
/// Tags are key-value pairs that appear at the start of a message,
/// prefixed with `@` and separated by `;`.
///
/// # Example
///
/// ```text
/// @time=2024-01-15T14:32:00.000Z;msgid=abc123 :nick PRIVMSG #chan :Hello
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    inner: HashMap<String, Option<String>>,
}

impl Tags {
    /// Create an empty tag set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse tags from a string (without the leading `@`).
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let mut tags = HashMap::new();

        for part in input.split(';') {
            if part.is_empty() {
                continue;
            }

            if let Some((key, value)) = part.split_once('=') {
                let unescaped = unescape_tag_value(value);
                tags.insert(key.to_string(), Some(unescaped));
            } else {
                tags.insert(part.to_string(), None);
            }
        }

        Ok(Self { inner: tags })
    }

    /// Get a tag value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.inner.get(key).and_then(|v| v.as_deref())
    }

    /// Check if a tag exists (regardless of value).
    pub fn contains(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    /// Set a tag value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.inner.insert(key.into(), Some(value.into()));
    }

    /// Set a tag without a value.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.inner.insert(key.into(), None);
    }

    /// Remove a tag.
    pub fn remove(&mut self, key: &str) -> Option<Option<String>> {
        self.inner.remove(key)
    }

    /// Check if there are no tags.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of tags.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Iterate over all tags.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v.as_deref()))
    }

    /// Get the `time` tag value.
    pub fn time(&self) -> Option<&str> {
        self.get("time")
    }

    /// Get the `msgid` tag value.
    pub fn msgid(&self) -> Option<&str> {
        self.get("msgid")
    }

    /// Get the `account` tag value.
    pub fn account(&self) -> Option<&str> {
        self.get("account")
    }

    /// Get the `batch` tag value.
    pub fn batch(&self) -> Option<&str> {
        self.get("batch")
    }

    /// Get the `label` tag value.
    pub fn label(&self) -> Option<&str> {
        self.get("label")
    }

    /// Check if this is a client-only tag (starts with `+`).
    pub fn is_client_only(key: &str) -> bool {
        key.starts_with('+')
    }
}

impl fmt::Display for Tags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (key, value) in &self.inner {
            if !first {
                write!(f, ";")?;
            }
            first = false;

            match value {
                Some(v) => write!(f, "{}={}", key, escape_tag_value(v))?,
                None => write!(f, "{}", key)?,
            }
        }
        Ok(())
    }
}

/// Unescape a tag value according to IRCv3 spec.
///
/// Escape sequences:
/// - `\:` -> `;`
/// - `\s` -> ` ` (space)
/// - `\\` -> `\`
/// - `\r` -> CR
/// - `\n` -> LF
fn unescape_tag_value(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(':') => result.push(';'),
                Some('s') => result.push(' '),
                Some('\\') => result.push('\\'),
                Some('r') => result.push('\r'),
                Some('n') => result.push('\n'),
                Some(other) => {
                    // Invalid escape, keep as-is
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Escape a tag value according to IRCv3 spec.
fn escape_tag_value(input: &str) -> String {
    let mut result = String::with_capacity(input.len());

    for c in input.chars() {
        match c {
            ';' => result.push_str("\\:"),
            ' ' => result.push_str("\\s"),
            '\\' => result.push_str("\\\\"),
            '\r' => result.push_str("\\r"),
            '\n' => result.push_str("\\n"),
            _ => result.push(c),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let tags = Tags::parse("time=2024-01-15T14:32:00.000Z;msgid=abc123").unwrap();
        assert_eq!(tags.time(), Some("2024-01-15T14:32:00.000Z"));
        assert_eq!(tags.msgid(), Some("abc123"));
    }

    #[test]
    fn test_parse_no_value() {
        let tags = Tags::parse("flag;key=value").unwrap();
        assert!(tags.contains("flag"));
        assert_eq!(tags.get("flag"), None);
        assert_eq!(tags.get("key"), Some("value"));
    }

    #[test]
    fn test_escape_roundtrip() {
        let original = "hello; world\\test\r\n";
        let escaped = escape_tag_value(original);
        let unescaped = unescape_tag_value(&escaped);
        assert_eq!(original, unescaped);
    }

    #[test]
    fn test_display() {
        let mut tags = Tags::new();
        tags.set("msgid", "abc123");
        tags.set("time", "2024-01-15T14:32:00.000Z");

        let s = tags.to_string();
        assert!(s.contains("msgid=abc123"));
        assert!(s.contains("time=2024-01-15T14:32:00.000Z"));
    }
}
