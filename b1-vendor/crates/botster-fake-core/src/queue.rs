//! The one ordered event queue and its classes (Core 6.2, EV-1, EV-5, EV-6).

use botster_core_contract::prelude::*;
use std::collections::VecDeque;

/// The queue class of an event (Core 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Mandatory: never dropped or replaced (EV-5).
    M,
    /// Keyed and coalesced: the latest value wins (EV-6).
    K,
    /// Droppable stream (EV-2).
    D,
}

pub fn class_of(ev: &Event) -> Class {
    match ev {
        Event::Completed { .. }
        | Event::SessionState { .. }
        | Event::RouteStalled { .. }
        | Event::RouteResumed { .. }
        | Event::RouteClosed { .. }
        | Event::ServiceState { .. }
        | Event::LaneConnected { .. }
        | Event::LaneEnded { .. } => Class::M,
        Event::Bell { .. }
        | Event::PromptMark { .. }
        | Event::Notification { .. }
        | Event::ClipboardWrite { .. } => Class::D,
        _ => Class::K,
    }
}

/// The key of a class K event: the event kind and what it is keyed by (Core 6.2).
fn key_of(ev: &Event) -> Option<String> {
    let kind = serde_json::to_value(ev).ok()?;
    let (name, body) = kind.as_object()?.iter().next()?;
    let part = |f: &str| body.get(f).map(|v| v.to_string()).unwrap_or_default();
    Some(match ev {
        Event::Activity { .. } => format!("{name}/{}/{}", part("instance"), part("source")),
        Event::ServiceWritable { .. } | Event::ServiceReadable { .. } => {
            format!("{name}/{}/{}", part("id"), part("lane"))
        }
        _ => format!("{name}/{}", part("instance")),
    })
}

#[derive(Default)]
pub struct Queue {
    items: VecDeque<Event>,
}

impl Queue {
    /// Posts an event. A class K event replaces the unpolled event of its key and takes the position of the newer observation (EV-1).
    pub fn post(&mut self, ev: Event) {
        if class_of(&ev) == Class::K {
            let key = key_of(&ev);
            self.items
                .retain(|e| class_of(e) != Class::K || key_of(e) != key);
        }
        self.items.push_back(ev);
    }

    /// `SessionState{Released}` retires every unpolled class K event of the instance (Core EV-5).
    pub fn retire_instance(&mut self, instance: &InstanceId) {
        self.items.retain(|e| {
            class_of(e) != Class::K
                || serde_json::to_value(e)
                    .ok()
                    .and_then(|v| v.as_object()?.values().next()?.get("instance").cloned())
                    != Some(serde_json::json!(instance))
        });
    }

    /// Takes at most `max` events from the front.
    pub fn take(&mut self, max: usize) -> Vec<Event> {
        let n = max.min(self.items.len());
        self.items.drain(..n).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
