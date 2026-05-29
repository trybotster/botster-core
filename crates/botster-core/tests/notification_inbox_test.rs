//! Notification inbox contract acceptance tests.

use botster_core::boundary::BoundaryJson;
use botster_core::client::ClientId;
use botster_core::notification::{
    NotificationAction, NotificationContent, NotificationDeliveryStatus, NotificationId,
    NotificationInbox, NotificationItem, NotificationKind, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp,
};
use botster_core::session::SessionId;

fn session_target(id: &str) -> NotificationTarget {
    NotificationTarget::Session(SessionId(id.to_string()))
}

fn client_target(id: &str) -> NotificationTarget {
    NotificationTarget::Client(ClientId(id.to_string()))
}

fn notification_id(id: &str) -> NotificationId {
    NotificationId(id.to_string())
}

fn source() -> NotificationSource {
    NotificationSource {
        label: "core-test".to_string(),
        plugin_key: None,
    }
}

fn message(id: &str, target: NotificationTarget) -> NotificationItem {
    NotificationItem::message(
        notification_id(id),
        target,
        NotificationSeverity::Info,
        source(),
        NotificationContent {
            title: "Build finished".to_string(),
            body: Some("The requested build completed.".to_string()),
            extension: None,
        },
        NotificationTimestamp(10),
    )
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize notification contract");
    serde_json::from_str(&json).expect("deserialize notification contract")
}

#[test]
fn post_message_queues_structured_session_message() {
    let mut inbox = NotificationInbox::new();
    let item = message("notification-1", session_target("session-1")).with_actions(vec![
        NotificationAction {
            id: "open".to_string(),
            label: "Open".to_string(),
            hint: Some("primary".to_string()),
            extension: None,
        },
    ]);

    let queued_id = inbox.post(round_trip(&item));

    assert_eq!(queued_id, notification_id("notification-1"));
    assert_eq!(
        inbox.status(&queued_id),
        Some(NotificationDeliveryStatus::Queued)
    );
    assert_eq!(round_trip(&item).content.title, "Build finished");
    assert_eq!(round_trip(&item).actions[0].id, "open");
}

#[test]
fn notify_only_records_attention_without_generic_message_body() {
    let item = NotificationItem::notify_only(
        notification_id("attention-1"),
        client_target("client-1"),
        NotificationSeverity::Attention,
        source(),
        "Needs review",
        NotificationTimestamp(11),
    );

    assert_eq!(item.kind, NotificationKind::NotifyOnly);
    assert_eq!(item.target, client_target("client-1"));
    assert_eq!(item.content.title, "Needs review");
    assert!(item.content.body.is_none());
    assert!(item.actions.is_empty());
}

#[test]
fn expired_items_are_not_drained_as_deliverable() {
    let mut inbox = NotificationInbox::new();
    let expired_id = notification_id("expired-1");
    let boundary_id = notification_id("boundary-1");
    let live_id = notification_id("live-1");

    inbox.post(
        message("expired-1", session_target("session-1")).with_expiry(NotificationTimestamp(20)),
    );
    inbox.post(
        message("boundary-1", session_target("session-1")).with_expiry(NotificationTimestamp(30)),
    );
    inbox.post(
        message("live-1", session_target("session-1")).with_expiry(NotificationTimestamp(40)),
    );

    let drained = inbox.drain(&session_target("session-1"), NotificationTimestamp(30));

    assert_eq!(
        drained.iter().map(|item| &item.id).collect::<Vec<_>>(),
        vec![&live_id]
    );
    assert_eq!(
        inbox.status(&expired_id),
        Some(NotificationDeliveryStatus::Expired)
    );
    assert_eq!(
        inbox.status(&boundary_id),
        Some(NotificationDeliveryStatus::Expired)
    );
    assert_eq!(
        inbox.status(&live_id),
        Some(NotificationDeliveryStatus::Delivered)
    );
}

#[test]
fn receive_drains_target_inbox_once() {
    let mut inbox = NotificationInbox::new();
    inbox.post(message("first", session_target("session-1")));
    inbox.post(message("second", session_target("session-1")));

    let first_drain = inbox.drain(&session_target("session-1"), NotificationTimestamp(12));
    let second_drain = inbox.drain(&session_target("session-1"), NotificationTimestamp(12));

    assert_eq!(
        first_drain
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert!(second_drain.is_empty());
}

#[test]
fn delivery_status_tracks_post_deliver_expire_and_drop() {
    let mut inbox = NotificationInbox::new();
    let delivered_id = inbox.post(message("delivered", session_target("session-1")));
    let expired_id = inbox.post(
        message("expired", session_target("session-2")).with_expiry(NotificationTimestamp(15)),
    );
    let dropped_id = inbox.post(message("dropped", client_target("client-1")));

    assert_eq!(
        inbox.status(&delivered_id),
        Some(NotificationDeliveryStatus::Queued)
    );

    let drained = inbox.drain(&session_target("session-1"), NotificationTimestamp(12));
    let expired = inbox.expire(NotificationTimestamp(20));
    let dropped = inbox.drop_item(&dropped_id);
    let acknowledged = inbox.acknowledge(&delivered_id);

    assert_eq!(drained[0].id, delivered_id);
    assert_eq!(expired, vec![expired_id.clone()]);
    assert_eq!(dropped, Some(NotificationDeliveryStatus::Dropped));
    assert_eq!(acknowledged, Some(NotificationDeliveryStatus::Acknowledged));
    assert_eq!(
        inbox.status(&delivered_id),
        Some(NotificationDeliveryStatus::Acknowledged)
    );
    assert_eq!(
        inbox.status(&expired_id),
        Some(NotificationDeliveryStatus::Expired)
    );
}

#[test]
fn session_and_client_scopes_are_isolated() {
    let mut inbox = NotificationInbox::new();
    inbox.post(message("session-message", session_target("session-1")));
    inbox.post(message("client-message", client_target("client-1")));

    let session_items = inbox.drain(&session_target("session-1"), NotificationTimestamp(12));
    let client_items = inbox.drain(&client_target("client-1"), NotificationTimestamp(12));

    assert_eq!(session_items.len(), 1);
    assert_eq!(session_items[0].target, session_target("session-1"));
    assert_eq!(client_items.len(), 1);
    assert_eq!(client_items[0].target, client_target("client-1"));
}

#[test]
fn boundary_json_is_limited_to_extension_payloads() {
    let item =
        message("extended", session_target("session-1")).with_actions(vec![NotificationAction {
            id: "plugin-action".to_string(),
            label: "Plugin Action".to_string(),
            hint: None,
            extension: Some(BoundaryJson(serde_json::json!({ "plugin_owned": true }))),
        }]);

    assert_eq!(item.kind, NotificationKind::Message);
    assert_eq!(item.severity, NotificationSeverity::Info);
    assert!(item.actions[0].extension.is_some());
    assert!(item.content.extension.is_none());
    assert!(!format!("{:?}", item.kind).contains("BoundaryJson"));
    assert!(!format!("{:?}", item.target).contains("BoundaryJson"));
    assert!(!format!("{:?}", item.severity).contains("BoundaryJson"));
}
