//! Event broadcast module for Server-Sent Events (SSE) support.
//!
//! This module provides a broadcast channel for real-time event distribution to
//! connected SSE clients. Events are ephemeral and not persisted to the database.

use crate::models::EventSeverity;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// A broadcast event that can be sent to SSE clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastEvent {
    /// The workflow ID this event belongs to.
    pub(crate) workflow_id: i64,
    /// Timestamp in milliseconds since Unix epoch.
    pub(crate) timestamp: i64,
    /// The type of event (e.g., "job_started", "job_completed", "job_failed").
    pub(crate) event_type: String,
    /// The severity level of the event.
    pub(crate) severity: EventSeverity,
    /// Event-specific data as JSON.
    pub(crate) data: serde_json::Value,
}

/// Event broadcaster that manages a broadcast channel for SSE events.
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: Arc<broadcast::Sender<BroadcastEvent>>,
}

impl EventBroadcaster {
    /// Create a new event broadcaster with the specified channel capacity.
    ///
    /// The capacity determines how many events can be buffered before slow
    /// receivers start missing events (lagging).
    pub(crate) fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
        }
    }

    /// Broadcast an event to all subscribed receivers.
    ///
    /// If there are no subscribers, the event is silently dropped.
    /// This is intentional - events are ephemeral and not persisted.
    pub(crate) fn broadcast(&self, event: BroadcastEvent) {
        // Ignore the result - if there are no receivers, the event is dropped
        let _ = self.sender.send(event);
    }

    /// Subscribe to the broadcast channel.
    ///
    /// Returns a receiver that will receive all future events broadcast
    /// after this subscription is created.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<BroadcastEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(512)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_broadcast_event() {
        let broadcaster = EventBroadcaster::new(16);
        let mut receiver = broadcaster.subscribe();

        let event = BroadcastEvent {
            workflow_id: 1,
            timestamp: 1234567890,
            event_type: "job_started".to_string(),
            severity: EventSeverity::Info,
            data: serde_json::json!({"job_id": 42}),
        };

        broadcaster.broadcast(event.clone());

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.workflow_id, 1);
        assert_eq!(received.event_type, "job_started");
        assert_eq!(received.severity, EventSeverity::Info);
    }

    #[tokio::test]
    async fn test_broadcast_no_subscribers() {
        let broadcaster = EventBroadcaster::new(16);

        // Broadcasting with no subscribers should not panic
        let event = BroadcastEvent {
            workflow_id: 1,
            timestamp: 1234567890,
            event_type: "test".to_string(),
            severity: EventSeverity::Debug,
            data: serde_json::json!({}),
        };

        broadcaster.broadcast(event);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let broadcaster = EventBroadcaster::new(16);
        let mut receiver1 = broadcaster.subscribe();
        let mut receiver2 = broadcaster.subscribe();

        let event = BroadcastEvent {
            workflow_id: 1,
            timestamp: 1234567890,
            event_type: "test".to_string(),
            severity: EventSeverity::Warning,
            data: serde_json::json!({"value": 123}),
        };

        broadcaster.broadcast(event);

        let received1 = receiver1.recv().await.unwrap();
        let received2 = receiver2.recv().await.unwrap();

        assert_eq!(received1.workflow_id, received2.workflow_id);
        assert_eq!(received1.event_type, received2.event_type);
        assert_eq!(received1.severity, received2.severity);
    }
}
