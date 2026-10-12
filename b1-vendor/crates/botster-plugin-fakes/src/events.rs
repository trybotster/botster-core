//! The event queues of one plugin (HP section 10): classes, bounded queues, shedding and gap markers.
//!
//! This is the part of FakeHub that the open orders of HP section 19 need: the delivery order across emitters (HP EV-4, EV-10) and which copies are shed under
//! pressure (HP EV-5, EV-12, EV-13). An event copy is a queued request of class `high`, `normal` or `low`. Not modeled here: the bounded loss ledger and its
//! aggregation (HP EV-12, EV-13), and the retry of a marker that a handler failed (HP EV-15). The `hp-suite` features add them with their transcripts.

use crate::hub::{FakeHub, Req, ReqState};
use crate::link::Choice;
use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The class of a queued request (HP EV-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Request,
    High,
    Normal,
    Low,
}

/// What an event copy or a gap marker knows about itself.
#[derive(Debug, Clone)]
pub(crate) struct Meta {
    pub owner: String,
    pub name: String,
    pub key: Option<String>,
    /// A gap marker: its payload is built from the ledger when it is sent (HP EV-13, EV-15).
    pub gap: bool,
}

#[derive(Default)]
pub(crate) struct Queues {
    /// Consecutive deliveries from a class above the lowest waiting class (HP EV-11).
    pub streak: u64,
    /// The copies lost per `(owner, name)` since the last marker (HP EV-13).
    pub lost: BTreeMap<(String, String), u64>,
    /// The names whose next marker already waits in a queue.
    pub pending: BTreeSet<(String, String)>,
    pub gap_seq: u64,
}

fn class_of(name: &str) -> Class {
    match name {
        "high" => Class::High,
        "low" => Class::Low,
        _ => Class::Normal,
    }
}

fn cap(h: &FakeHub, c: Class) -> u64 {
    match c {
        Class::High => h.limits.event_queue_high,
        Class::Normal => h.limits.event_queue_normal,
        Class::Low => h.limits.event_queue_low,
        Class::Request => h.limits.invoke_queue,
    }
}

/// The next queued request to send. HP EV-11: high events, then requests, then normal events, then low events; after `event_starvation_bound` consecutive
/// deliveries from a higher class, one copy from the next lower non-empty class. Within a class the order is the admission order (HP IV-4).
pub(crate) fn pick_next(h: &mut FakeHub, key: &str) -> Option<usize> {
    let order = [Class::High, Class::Request, Class::Normal, Class::Low];
    let first = |h: &FakeHub, c: Class| {
        h.plugins[key]
            .reqs
            .iter()
            .position(|r| r.state == ReqState::Queued && r.class == c)
    };
    let live: Vec<(Class, usize)> = order
        .iter()
        .filter_map(|c| first(h, *c).map(|i| (*c, i)))
        .collect();
    let bound = h.limits.event_starvation_bound;
    let streak = h.plugins[key].events.streak;
    let pick = match live.as_slice() {
        [] => return None,
        [only] => (*only, false),
        [top, next, ..] if streak >= bound => (*next, true),
        [top, ..] => (*top, false),
    };
    let ((_, idx), served_lower) = pick;
    let q = &mut h.plugins.get_mut(key).expect("plugin").events;
    q.streak = if served_lower || live.len() == 1 {
        0
    } else {
        q.streak + 1
    };
    Some(idx)
}

/// Builds the payload of a queued request when it is sent: a gap marker reads its count from the ledger (HP EV-13, EV-15).
pub(crate) fn finalize(h: &mut FakeHub, key: &str, idx: usize) {
    let now = h.now;
    let p = h.plugins.get_mut(key).expect("plugin");
    let Some(meta) = p.reqs[idx].meta.clone() else {
        return;
    };
    if meta.gap {
        let id = (meta.owner.clone(), meta.name.clone());
        let count = p.events.lost.remove(&id).unwrap_or(0);
        p.events.pending.remove(&id);
        p.events.gap_seq += 1;
        p.reqs[idx].payload = json!({"owner": meta.owner, "name": meta.name, "count": count, "gap_id": p.events.gap_seq, "recovery": "unavailable"});
    } else {
        p.reqs[idx].payload = json!({"owner": meta.owner, "name": meta.name, "priority": match p.reqs[idx].class { Class::High => "high", Class::Low => "low", _ => "normal" }, "source": p.reqs[idx].source, "payload": p.reqs[idx].payload, "at": now});
    }
}

fn loss(h: &mut FakeHub, key: &str, class: Class, at: Option<usize>, owner: &str, name: &str) {
    let id = (owner.to_string(), name.to_string());
    let p = h.plugins.get_mut(key).expect("plugin");
    *p.events.lost.entry(id.clone()).or_default() += 1;
    if p.events.pending.insert(id) {
        let handler = p.published["gap_handler"]
            .as_str()
            .unwrap_or("gap")
            .to_string();
        let marker = Req {
            ticket: 0,
            request_id: 0,
            handler,
            kind: HandlerKind::EventGap,
            origin: "event_gap".into(),
            payload: json!({}),
            caller: Caller::System,
            source: None,
            descriptor_id: None,
            long: false,
            idempotency_key: false,
            state: ReqState::Queued,
            deadline_at: 0,
            conn: String::new(),
            class,
            meta: Some(Meta {
                owner: owner.into(),
                name: name.into(),
                key: None,
                gap: true,
            }),
        };
        // HP EV-13: the marker is delivered at the position of the first copy lost since that name's last marker. A high marker needs no queue room (HP EV-12).
        match at {
            Some(i) => p.reqs.insert(i, marker),
            None => p.reqs.push(marker),
        }
    }
}

/// Enqueues one event copy for the plugin, or sheds one under the rule of its class (HP EV-5, EV-12, EV-13).
fn enqueue(
    h: &mut FakeHub,
    key: &str,
    class: Class,
    meta: Meta,
    source: Option<EventSourceRef>,
    handler: &str,
    payload: Value,
) {
    let queued: Vec<usize> = (0..h.plugins[key].reqs.len())
        .filter(|i| {
            let r = &h.plugins[key].reqs[*i];
            r.state == ReqState::Queued
                && r.class == class
                && r.meta.as_ref().is_some_and(|m| !m.gap)
        })
        .collect();
    let full = queued.len() as u64 >= cap(h, class);
    if full {
        match class {
            // HP EV-12: a full high queue takes no further copy; the loss goes to the ledger and a marker follows.
            Class::High => return loss(h, key, class, None, &meta.owner, &meta.name),
            // HP EV-13: a keyed normal copy coalesces with the older one of its key; an unkeyed one drops the oldest; low drops the oldest.
            _ => {
                let same = meta.key.as_ref().and_then(|k| {
                    queued.iter().copied().find(|i| {
                        h.plugins[key].reqs[*i].meta.as_ref().is_some_and(|m| {
                            m.owner == meta.owner
                                && m.name == meta.name
                                && m.key.as_ref() == Some(k)
                        })
                    })
                });
                let victim = if class == Class::Normal {
                    same.unwrap_or(queued[0])
                } else {
                    queued[0]
                };
                let m = h.plugins[key].reqs[victim]
                    .meta
                    .clone()
                    .expect("event meta");
                h.plugins.get_mut(key).expect("plugin").reqs.remove(victim);
                loss(h, key, class, Some(victim), &m.owner, &m.name);
            }
        }
    }
    let r = Req {
        ticket: 0,
        request_id: 0,
        handler: handler.to_string(),
        kind: HandlerKind::Event,
        origin: "event".into(),
        payload,
        caller: Caller::System,
        source,
        descriptor_id: None,
        long: false,
        idempotency_key: false,
        state: ReqState::Queued,
        deadline_at: 0,
        conn: String::new(),
        class,
        meta: Some(meta),
    };
    // HP EV-13: the latest copy goes to the end, never in place of the older one.
    h.plugins.get_mut(key).expect("plugin").reqs.push(r);
}

/// The control `emit_events`: each emitter emits `count` copies of one event, in order. The order across emitters is a seeded choice (HP EV-4: no order across emitters).
pub(crate) fn emit(h: &mut FakeHub, key: &str, op: &Value) -> Value {
    let emitters = op["emitters"].as_array().cloned().unwrap_or_default();
    let mut left: Vec<u64> = emitters
        .iter()
        .map(|e| e["count"].as_u64().unwrap_or(1))
        .collect();
    let mut sent: Vec<u64> = vec![0; emitters.len()];
    let first_handler = h.plugins[key].published["subscriptions"][0]
        .as_str()
        .unwrap_or("event")
        .to_string();
    while left.iter().any(|n| *n > 0) {
        let live: Vec<usize> = (0..left.len()).filter(|i| left[*i] > 0).collect();
        let i = live[h.ch.pick(Choice::EmitterOrder, live.len())];
        left[i] -= 1;
        sent[i] += 1;
        let e = &emitters[i];
        let meta = Meta {
            owner: e["owner"].as_str().unwrap_or("hub").into(),
            name: e["name"].as_str().unwrap_or("e").into(),
            key: e.get("key").and_then(Value::as_str).map(String::from),
            gap: false,
        };
        let source = e
            .get("source")
            .and_then(|s| serde_json::from_value(s.clone()).ok());
        let handler = e["handler"].as_str().unwrap_or(&first_handler).to_string();
        enqueue(
            h,
            key,
            class_of(e["priority"].as_str().unwrap_or("normal")),
            meta,
            source,
            &handler,
            json!({"seq": sent[i], "emitter": i}),
        );
    }
    h.plugins.get_mut(key).expect("plugin").events.streak = 0;
    json!({})
}
