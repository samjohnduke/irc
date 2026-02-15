//! BATCH message collector.
//!
//! IRCv3 BATCH groups related messages together. This module collects
//! messages with @batch= tags and emits them as a single Event::Batch
//! when the batch closes.

use irc_proto::{Command, Message};
use std::collections::HashMap;

use crate::event::Event;

/// Collects messages belonging to BATCH groups.
#[derive(Debug, Default)]
pub struct BatchCollector {
    /// Active batches indexed by reference ID.
    batches: HashMap<String, PendingBatch>,
}

/// A batch in progress.
#[derive(Debug)]
pub struct PendingBatch {
    /// Batch type (e.g., "chathistory", "netjoin").
    pub batch_type: String,

    /// Target (e.g., channel name for chathistory).
    pub target: Option<String>,

    /// Collected events.
    pub events: Vec<Event>,

    /// Additional batch parameters.
    pub params: Vec<String>,
}

/// Result of processing a message for batching.
#[derive(Debug)]
pub enum BatchResult {
    /// Message is not part of a batch, process normally.
    NotBatched(Message),

    /// Message was added to a pending batch.
    Batched,

    /// Batch completed, here are all the events.
    Complete(Event),

    /// Batch was started (message consumed).
    Started,
}

impl BatchCollector {
    /// Create a new batch collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a message, handling batch start/end and collection.
    ///
    /// Returns how to handle this message.
    pub fn process(
        &mut self,
        msg: Message,
        event_converter: impl FnOnce(Message) -> Option<Event>,
    ) -> BatchResult {
        // Check for BATCH command (start/end)
        if let Command::Batch {
            reference,
            batch_type,
            params,
        } = &msg.command
        {
            if reference.starts_with('+') {
                // Start new batch
                let ref_id = reference[1..].to_string();
                let batch_type = batch_type.clone().unwrap_or_default();
                let target = params.first().cloned();

                self.batches.insert(
                    ref_id,
                    PendingBatch {
                        batch_type,
                        target,
                        events: Vec::new(),
                        params: params.clone(),
                    },
                );

                return BatchResult::Started;
            } else if reference.starts_with('-') {
                // End batch
                let ref_id = &reference[1..];
                if let Some(batch) = self.batches.remove(ref_id) {
                    return BatchResult::Complete(Event::Batch {
                        batch_type: batch.batch_type,
                        target: batch.target,
                        messages: batch.events,
                    });
                }
                // Unknown batch end, ignore
                return BatchResult::Started;
            }
        }

        // Check if message belongs to an active batch
        if let Some(batch_ref) = msg.tags.as_ref().and_then(|t| t.batch()) {
            if let Some(batch) = self.batches.get_mut(batch_ref) {
                // Convert to event and add to batch
                if let Some(event) = event_converter(msg) {
                    batch.events.push(event);
                }
                return BatchResult::Batched;
            }
        }

        // Not part of any batch
        BatchResult::NotBatched(msg)
    }

    /// Check if there are any active batches.
    pub fn has_active_batches(&self) -> bool {
        !self.batches.is_empty()
    }

    /// Get the number of active batches.
    pub fn active_batch_count(&self) -> usize {
        self.batches.len()
    }

    /// Clear all pending batches (e.g., on disconnect).
    pub fn clear(&mut self) {
        self.batches.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use irc_proto::Tags;

    fn make_batch_start(ref_id: &str, batch_type: &str, target: &str) -> Message {
        Message::new(Command::Batch {
            reference: format!("+{}", ref_id),
            batch_type: Some(batch_type.to_string()),
            params: vec![target.to_string()],
        })
    }

    fn make_batch_end(ref_id: &str) -> Message {
        Message::new(Command::Batch {
            reference: format!("-{}", ref_id),
            batch_type: None,
            params: vec![],
        })
    }

    fn make_batched_privmsg(ref_id: &str, target: &str, msg: &str) -> Message {
        let mut tags = Tags::new();
        tags.set("batch", ref_id);
        Message::new(Command::Privmsg {
            target: target.to_string(),
            message: msg.to_string(),
        })
        .with_tags(tags)
    }

    #[test]
    fn test_batch_collection() {
        let mut collector = BatchCollector::new();

        // Start batch
        let result =
            collector.process(make_batch_start("abc123", "chathistory", "#test"), |_| None);
        assert!(matches!(result, BatchResult::Started));
        assert!(collector.has_active_batches());

        // Add batched message
        let result = collector.process(make_batched_privmsg("abc123", "#test", "Hello"), |msg| {
            Some(Event::Privmsg {
                source: "nick".to_string(),
                target: "#test".to_string(),
                message: "Hello".to_string(),
                meta: Default::default(),
            })
        });
        assert!(matches!(result, BatchResult::Batched));

        // End batch
        let result = collector.process(make_batch_end("abc123"), |_| None);
        if let BatchResult::Complete(Event::Batch {
            batch_type,
            target,
            messages,
        }) = result
        {
            assert_eq!(batch_type, "chathistory");
            assert_eq!(target, Some("#test".to_string()));
            assert_eq!(messages.len(), 1);
        } else {
            panic!("Expected BatchResult::Complete");
        }

        assert!(!collector.has_active_batches());
    }

    #[test]
    fn test_non_batched_message() {
        let mut collector = BatchCollector::new();

        let msg = Message::new(Command::Privmsg {
            target: "#test".to_string(),
            message: "Hello".to_string(),
        });

        let result = collector.process(msg.clone(), |_| None);
        assert!(matches!(result, BatchResult::NotBatched(_)));
    }
}
