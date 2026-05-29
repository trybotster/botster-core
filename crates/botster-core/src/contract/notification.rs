//! Transport-neutral notification and inbox primitives.

use serde::{Deserialize, Serialize};

use crate::boundary::BoundaryJson;
use crate::client::ClientId;
use crate::session::SessionId;

/// Stable identifier for a notification inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationId(pub String);

/// Deterministic timestamp used by notification expiry checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NotificationTimestamp(pub u64);

/// Session or client route that owns an inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum NotificationTarget {
    /// Notification is scoped to a session.
    Session(SessionId),
    /// Notification is scoped to a client.
    Client(ClientId),
}

/// Notification delivery intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Structured message content should be available to the receiver.
    Message,
    /// Attention-only notification with no generic message body.
    NotifyOnly,
}

/// Stable severity vocabulary owned by core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    /// Informational notice.
    Info,
    /// Successful completion notice.
    Success,
    /// Warning notice.
    Warning,
    /// Error notice.
    Error,
    /// Attention request that may not be an error.
    Attention,
}

/// Source metadata for a notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSource {
    /// Stable source label shown to hosts and clients.
    pub label: String,
    /// Optional plugin owner key when the source is plugin-owned.
    pub plugin_key: Option<String>,
}

/// Stable action attached to a notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationAction {
    /// Stable action identifier within the notification source.
    pub id: String,
    /// Human-readable action label.
    pub label: String,
    /// Optional host-facing hint for choosing a presentation affordance.
    pub hint: Option<String>,
    /// Plugin-owned action payload whose schema is opaque to core.
    pub extension: Option<BoundaryJson>,
}

/// Typed notification content with an optional extension payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationContent {
    /// Short notification title.
    pub title: String,
    /// Optional generic message body.
    pub body: Option<String>,
    /// Plugin-owned structured content whose schema is opaque to core.
    pub extension: Option<BoundaryJson>,
}

/// In-memory delivery status for an inbox item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationDeliveryStatus {
    /// Item is queued for its target.
    Queued,
    /// Item was drained by its target.
    Delivered,
    /// Item expired before delivery.
    Expired,
    /// Item was dropped by host policy.
    Dropped,
    /// Item was acknowledged by a host or client.
    Acknowledged,
}

/// Notification envelope stored in a core inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationItem {
    /// Stable notification id.
    pub id: NotificationId,
    /// Session or client route that owns this item.
    pub target: NotificationTarget,
    /// Delivery intent.
    pub kind: NotificationKind,
    /// Stable severity.
    pub severity: NotificationSeverity,
    /// Source metadata.
    pub source: NotificationSource,
    /// Typed content.
    pub content: NotificationContent,
    /// Optional actions.
    pub actions: Vec<NotificationAction>,
    /// Deterministic creation timestamp.
    pub created_at: NotificationTimestamp,
    /// Optional deterministic expiry timestamp.
    pub expires_at: Option<NotificationTimestamp>,
    /// Current in-memory status.
    pub status: NotificationDeliveryStatus,
}

impl NotificationItem {
    /// Build a structured message inbox item.
    #[must_use]
    pub fn message(
        id: NotificationId,
        target: NotificationTarget,
        severity: NotificationSeverity,
        source: NotificationSource,
        content: NotificationContent,
        created_at: NotificationTimestamp,
    ) -> Self {
        Self {
            id,
            target,
            kind: NotificationKind::Message,
            severity,
            source,
            content,
            actions: Vec::new(),
            created_at,
            expires_at: None,
            status: NotificationDeliveryStatus::Queued,
        }
    }

    /// Build an attention-only inbox item.
    #[must_use]
    pub fn notify_only(
        id: NotificationId,
        target: NotificationTarget,
        severity: NotificationSeverity,
        source: NotificationSource,
        title: impl Into<String>,
        created_at: NotificationTimestamp,
    ) -> Self {
        Self {
            id,
            target,
            kind: NotificationKind::NotifyOnly,
            severity,
            source,
            content: NotificationContent {
                title: title.into(),
                body: None,
                extension: None,
            },
            actions: Vec::new(),
            created_at,
            expires_at: None,
            status: NotificationDeliveryStatus::Queued,
        }
    }

    /// Set actions on an inbox item.
    #[must_use]
    pub fn with_actions(mut self, actions: Vec<NotificationAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Set an expiry timestamp on an inbox item.
    #[must_use]
    pub const fn with_expiry(mut self, expires_at: NotificationTimestamp) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Whether this item is expired at the provided timestamp.
    #[must_use]
    pub fn is_expired_at(&self, now: NotificationTimestamp) -> bool {
        self.expires_at.is_some_and(|expires_at| expires_at <= now)
    }
}

/// In-memory inbox model keyed by notification target.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationInbox {
    items: Vec<NotificationItem>,
}

impl NotificationInbox {
    /// Build an empty in-memory inbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue an item and return its id.
    pub fn post(&mut self, mut item: NotificationItem) -> NotificationId {
        item.status = NotificationDeliveryStatus::Queued;
        let id = item.id.clone();
        self.items.push(item);
        id
    }

    /// Mark queued expired items and return their ids.
    pub fn expire(&mut self, now: NotificationTimestamp) -> Vec<NotificationId> {
        let mut expired = Vec::new();

        for item in &mut self.items {
            if item.status == NotificationDeliveryStatus::Queued && item.is_expired_at(now) {
                item.status = NotificationDeliveryStatus::Expired;
                expired.push(item.id.clone());
            }
        }

        expired
    }

    /// Drain deliverable items for one target exactly once.
    pub fn drain(
        &mut self,
        target: &NotificationTarget,
        now: NotificationTimestamp,
    ) -> Vec<NotificationItem> {
        let mut drained = Vec::new();

        for item in &mut self.items {
            if &item.target != target || item.status != NotificationDeliveryStatus::Queued {
                continue;
            }

            if item.is_expired_at(now) {
                item.status = NotificationDeliveryStatus::Expired;
                continue;
            }

            item.status = NotificationDeliveryStatus::Delivered;
            drained.push(item.clone());
        }

        drained
    }

    /// Return the current status for an item id.
    #[must_use]
    pub fn status(&self, id: &NotificationId) -> Option<NotificationDeliveryStatus> {
        self.items
            .iter()
            .find(|item| &item.id == id)
            .map(|item| item.status)
    }

    /// Mark an item as dropped.
    pub fn drop_item(&mut self, id: &NotificationId) -> Option<NotificationDeliveryStatus> {
        self.set_status(id, NotificationDeliveryStatus::Dropped)
    }

    /// Mark an item as acknowledged.
    pub fn acknowledge(&mut self, id: &NotificationId) -> Option<NotificationDeliveryStatus> {
        self.set_status(id, NotificationDeliveryStatus::Acknowledged)
    }

    fn set_status(
        &mut self,
        id: &NotificationId,
        status: NotificationDeliveryStatus,
    ) -> Option<NotificationDeliveryStatus> {
        let item = self.items.iter_mut().find(|item| &item.id == id)?;
        item.status = status;
        Some(item.status)
    }
}
