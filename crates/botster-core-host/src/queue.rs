//! The one event queue (Core 6.2, EV-2, EV-5, EV-6, EV-9): four classes, each with its own bound and loss rule.
//!
//! - **M** mandatory: never dropped and never replaced. `Completed` has a slot that the engine reserved at `begin`, so it
//!   needs no room here; every other M event counts against `mandatory_events`, and a producer that finds no room waits.
//! - **K** keyed, coalesced: at most one unpolled event per key; a newer event replaces it, carries the latest value and
//!   takes the newer position (EV-1).
//! - **D** droppable: at most `event_queue` of them; the oldest is dropped first and leaves a loss marker (EV-2).
//! - **L** loss marker: one per session instance, in the position of the first event of that instance that was dropped.
//!
//! The queue is pure data: it reads no clock and holds no randomness, and it orders events by a per-queue sequence (OR-2).

use botster_core_contract::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// The class of an event (Core 6.2).
///
/// Clause: Core EV-2, Core EV-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Completed`: its slot is reserved, so it never needs room.
    Completed,
    /// Every other mandatory event.
    Mandatory,
    Keyed,
    Droppable,
    Lost,
}

/// The class of `event`. `RouteAdopted` has no class in 6.2; it is mandatory, because it must never be lost (DP-8).
pub fn class_of(event: &Event) -> Class {
    match event {
        Event::Completed { .. } => Class::Completed,
        Event::SessionState { .. }
        | Event::RouteStalled { .. }
        | Event::RouteResumed { .. }
        | Event::RouteClosed { .. }
        | Event::RouteAdopted { .. }
        | Event::ServiceState { .. }
        | Event::LaneConnected { .. }
        | Event::LaneEnded { .. } => Class::Mandatory,
        Event::ModesChanged { .. }
        | Event::SizeChanged { .. }
        | Event::FocusChanged { .. }
        | Event::Activity { .. }
        | Event::Silent { .. }
        | Event::TitleChanged { .. }
        | Event::CwdChanged { .. }
        | Event::MetadataChanged { .. }
        | Event::SessionWritable { .. }
        | Event::ServiceWritable { .. }
        | Event::ServiceReadable { .. } => Class::Keyed,
        Event::Bell { .. }
        | Event::PromptMark { .. }
        | Event::Notification { .. }
        | Event::ClipboardWrite { .. } => Class::Droppable,
        Event::EventsLost { .. } => Class::Lost,
        // The enum is non-exhaustive: an event that a later contract adds is mandatory until a clause gives it a class.
        _ => Class::Mandatory,
    }
}

/// What a class K event is keyed by (Core 6.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Modes(InstanceId),
    Size(InstanceId),
    Focus(InstanceId),
    /// Keyed by instance, source and route (IN-4).
    Activity(InstanceId, ActivitySourceKey),
    Silent(InstanceId),
    Title(InstanceId),
    Cwd(InstanceId),
    Metadata(InstanceId),
    Writable(InstanceId),
    ServiceWritable(ServiceId, u8),
    ServiceReadable(ServiceId, u8),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ActivitySourceKey {
    Output,
    Client(RouteId),
}

fn key_of(event: &Event) -> Option<Key> {
    Some(match event {
        Event::ModesChanged { instance, .. } => Key::Modes(instance.clone()),
        Event::SizeChanged { instance, .. } => Key::Size(instance.clone()),
        Event::FocusChanged { instance, .. } => Key::Focus(instance.clone()),
        Event::Activity {
            instance, source, ..
        } => Key::Activity(
            instance.clone(),
            match source {
                ActivitySource::Output => ActivitySourceKey::Output,
                ActivitySource::Client(route) => ActivitySourceKey::Client(*route),
                // A source that a later contract adds is keyed with the output, the coarsest key.
                _ => ActivitySourceKey::Output,
            },
        ),
        Event::Silent { instance, .. } => Key::Silent(instance.clone()),
        Event::TitleChanged { instance, .. } => Key::Title(instance.clone()),
        Event::CwdChanged { instance, .. } => Key::Cwd(instance.clone()),
        Event::MetadataChanged { instance, .. } => Key::Metadata(instance.clone()),
        Event::SessionWritable { instance, .. } => Key::Writable(instance.clone()),
        Event::ServiceWritable { id, lane } => Key::ServiceWritable(*id, *lane),
        Event::ServiceReadable { id, lane } => Key::ServiceReadable(*id, *lane),
        _ => return None,
    })
}

/// The instance that a session-scoped event belongs to.
fn instance_of(event: &Event) -> Option<&InstanceId> {
    match event {
        Event::SessionState { instance, .. }
        | Event::SessionWritable { instance, .. }
        | Event::ModesChanged { instance, .. }
        | Event::FocusChanged { instance, .. }
        | Event::SizeChanged { instance, .. }
        | Event::Activity { instance, .. }
        | Event::Silent { instance, .. }
        | Event::TitleChanged { instance, .. }
        | Event::CwdChanged { instance, .. }
        | Event::Bell { instance, .. }
        | Event::PromptMark { instance, .. }
        | Event::Notification { instance, .. }
        | Event::ClipboardWrite { instance, .. }
        | Event::EventsLost { instance, .. }
        | Event::MetadataChanged { instance, .. }
        | Event::RouteAdopted { instance, .. } => Some(instance),
        _ => None,
    }
}

/// The service that a service-scoped event belongs to.
fn service_of(event: &Event) -> Option<ServiceId> {
    match event {
        Event::ServiceWritable { id, .. } | Event::ServiceReadable { id, .. } => Some(*id),
        _ => None,
    }
}

/// An event of class K that is keyed by a route (a client `Activity`).
fn route_of(event: &Event) -> Option<RouteId> {
    match event {
        Event::Activity {
            source: ActivitySource::Client(route),
            ..
        } => Some(*route),
        _ => None,
    }
}

/// The loss marker of one session instance. It is an `EventsLost` event in the position of the first dropped event.
#[derive(Debug, Clone)]
struct Marker {
    id: SessionId,
    kinds: BTreeSet<LostKind>,
    tap_dropped_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
enum Entry {
    Event(Event),
    Marker {
        instance: InstanceId,
        marker: Marker,
    },
}

/// The bounds of the queue (Core 9B).
#[derive(Debug, Clone, Copy)]
pub struct QueueBounds {
    /// `CoreLimits.event_queue`: class D events.
    pub droppable: usize,
    /// `CoreLimits.mandatory_events`: class M events other than `Completed`.
    pub mandatory: usize,
}

/// What a poll returned.
#[derive(Debug)]
pub struct Polled {
    pub events: Vec<Event>,
    /// How many mandatory events (not `Completed`) the poll removed: room that parked work may use (EV-5d).
    pub freed_mandatory: usize,
}

/// The one ordered event queue.
#[derive(Debug)]
pub struct EventQueue {
    bounds: QueueBounds,
    next_seq: u64,
    entries: BTreeMap<u64, Entry>,
    keyed: BTreeMap<Key, u64>,
    markers: BTreeMap<InstanceId, u64>,
    droppable: BTreeSet<u64>,
    mandatory: usize,
    /// How many events were posted since the last `take_posted` (the `events_posted` of `PumpReport`).
    posted: u32,
    /// Every event ever posted: a step compares it with its start to learn whether it posted already.
    total: u64,
}

impl EventQueue {
    pub fn new(bounds: QueueBounds) -> EventQueue {
        EventQueue {
            bounds,
            next_seq: 0,
            entries: BTreeMap::new(),
            keyed: BTreeMap::new(),
            markers: BTreeMap::new(),
            droppable: BTreeSet::new(),
            mandatory: 0,
            posted: 0,
            total: 0,
        }
    }

    /// The count of every event posted so far.
    pub fn total_posted(&self) -> u64 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when one more mandatory event (not `Completed`) fits (EV-5b).
    pub fn has_mandatory_room(&self) -> bool {
        self.mandatory < self.bounds.mandatory
    }

    /// The count of events posted since the last call, and a reset.
    pub fn take_posted(&mut self) -> u32 {
        std::mem::take(&mut self.posted)
    }

    fn push(&mut self, entry: Entry) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.insert(seq, entry);
        self.posted += 1;
        self.total += 1;
        seq
    }

    /// Posts `Completed`. Its slot was reserved at `begin` (EV-5a), so it never needs room.
    pub fn post_completed(&mut self, op: OpId, result: OpResult) {
        self.push(Entry::Event(Event::Completed { op, result }));
    }

    /// Posts a mandatory event, or refuses when the queue has no room (EV-5b): the producer then waits.
    ///
    /// # Errors
    /// `Err(event)` hands the event back when the queue is full.
    pub fn post_mandatory(&mut self, event: Event) -> Result<(), Box<Event>> {
        debug_assert_eq!(class_of(&event), Class::Mandatory);
        if !self.has_mandatory_room() {
            return Err(Box::new(event));
        }
        self.mandatory += 1;
        self.push(Entry::Event(event));
        Ok(())
    }

    /// Posts a class K event. An unpolled event of the same key is replaced, and the new event takes the later position
    /// (EV-1, EV-6).
    pub fn post_keyed(&mut self, event: Event) {
        let key = key_of(&event).expect("a class K event has a key");
        if let Some(old) = self.keyed.remove(&key) {
            self.entries.remove(&old);
        }
        let seq = self.push(Entry::Event(event));
        self.keyed.insert(key, seq);
    }

    /// Posts a class D event. When `event_queue` of them are unpolled, the oldest is dropped first (EV-2) and the loss is
    /// recorded in its session's marker.
    pub fn post_droppable(&mut self, event: Event) {
        let kind = lost_kind(&event).expect("a class D event has a kind");
        if self.droppable.len() >= self.bounds.droppable {
            self.drop_oldest();
        }
        let seq = self.push(Entry::Event(event));
        let _ = kind;
        self.droppable.insert(seq);
    }

    /// Records a loss that the worker reported (a tap overflow, a query that was too long): the marker gains the kind.
    ///
    /// Clause: Core EV-2, Core TP-1, Core EV-8.
    pub fn post_lost(
        &mut self,
        id: &SessionId,
        instance: &InstanceId,
        kind: LostKind,
        tap_dropped_bytes: u64,
    ) {
        match self.markers.get(instance).copied() {
            Some(seq) => {
                if let Some(Entry::Marker { marker, .. }) = self.entries.get_mut(&seq) {
                    marker.kinds.insert(kind);
                    if tap_dropped_bytes > 0 {
                        *marker.tap_dropped_bytes.get_or_insert(0) += tap_dropped_bytes;
                    }
                }
            }
            None => {
                let marker = Marker {
                    id: id.clone(),
                    kinds: BTreeSet::from([kind]),
                    tap_dropped_bytes: (tap_dropped_bytes > 0).then_some(tap_dropped_bytes),
                };
                let seq = self.push(Entry::Marker {
                    instance: instance.clone(),
                    marker,
                });
                self.markers.insert(instance.clone(), seq);
            }
        }
    }

    /// Drops the oldest class D event. Its session's marker takes its position, or gains the kind (EV-2).
    fn drop_oldest(&mut self) {
        let Some(seq) = self.droppable.pop_first() else {
            return;
        };
        let Some(Entry::Event(event)) = self.entries.remove(&seq) else {
            return;
        };
        let (Some(kind), Some(instance), Some(id)) = (
            lost_kind(&event),
            instance_of(&event).cloned(),
            session_id_of(&event).cloned(),
        ) else {
            return;
        };
        if let Some(existing) = self.markers.get(&instance).copied() {
            if let Some(Entry::Marker { marker, .. }) = self.entries.get_mut(&existing) {
                marker.kinds.insert(kind);
            }
            return;
        }
        let marker = Marker {
            id,
            kinds: BTreeSet::from([kind]),
            tap_dropped_bytes: None,
        };
        self.entries.insert(
            seq,
            Entry::Marker {
                instance: instance.clone(),
                marker,
            },
        );
        self.markers.insert(instance, seq);
    }

    /// Retires every unpolled K, L and D event of `instance`, with no loss marker (6.2 retirement, EV-5).
    pub fn retire_instance(&mut self, instance: &InstanceId) {
        let doomed: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| match entry {
                Entry::Marker { instance: i, .. } => i == instance,
                Entry::Event(event) => {
                    matches!(
                        class_of(event),
                        Class::Keyed | Class::Droppable | Class::Lost
                    ) && instance_of(event) == Some(instance)
                }
            })
            .map(|(seq, _)| *seq)
            .collect();
        for seq in doomed {
            self.remove(seq);
        }
    }

    /// Retires the unpolled class K events that are keyed by `route` (a route's `Activity`, EV-5).
    pub fn retire_route(&mut self, route: RouteId) {
        let doomed: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry, Entry::Event(e) if route_of(e) == Some(route)))
            .map(|(seq, _)| *seq)
            .collect();
        for seq in doomed {
            self.remove(seq);
        }
    }

    /// Retires the unpolled class K events of a service that was released (EV-5).
    pub fn retire_service(&mut self, service: ServiceId) {
        let doomed: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry, Entry::Event(e) if service_of(e) == Some(service)))
            .map(|(seq, _)| *seq)
            .collect();
        for seq in doomed {
            self.remove(seq);
        }
    }

    fn remove(&mut self, seq: u64) {
        let Some(entry) = self.entries.remove(&seq) else {
            return;
        };
        match entry {
            Entry::Marker { instance, .. } => {
                self.markers.remove(&instance);
            }
            Entry::Event(event) => match class_of(&event) {
                Class::Keyed => {
                    if let Some(key) = key_of(&event) {
                        self.keyed.remove(&key);
                    }
                }
                Class::Droppable => {
                    self.droppable.remove(&seq);
                }
                Class::Mandatory => self.mandatory -= 1,
                Class::Completed | Class::Lost => {}
            },
        }
    }

    /// Takes at most `max` events from the front, in order (OR-2). It makes no progress.
    pub fn poll(&mut self, max: usize) -> Polled {
        let mut events = Vec::new();
        let mut freed_mandatory = 0;
        while events.len() < max {
            let Some((&seq, _)) = self.entries.first_key_value() else {
                break;
            };
            let entry = self.entries.remove(&seq).expect("the key was just read");
            match entry {
                Entry::Marker { instance, marker } => {
                    self.markers.remove(&instance);
                    events.push(Event::EventsLost {
                        id: marker.id,
                        instance,
                        kinds: marker.kinds,
                        tap_dropped_bytes: marker.tap_dropped_bytes,
                    });
                }
                Entry::Event(event) => {
                    match class_of(&event) {
                        Class::Keyed => {
                            if let Some(key) = key_of(&event) {
                                self.keyed.remove(&key);
                            }
                        }
                        Class::Droppable => {
                            self.droppable.remove(&seq);
                        }
                        Class::Mandatory => {
                            self.mandatory -= 1;
                            freed_mandatory += 1;
                        }
                        Class::Completed | Class::Lost => {}
                    }
                    events.push(event);
                }
            }
        }
        Polled {
            events,
            freed_mandatory,
        }
    }
}

fn lost_kind(event: &Event) -> Option<LostKind> {
    match event {
        Event::Bell { .. } => Some(LostKind::Bell),
        Event::PromptMark { .. } => Some(LostKind::PromptMark),
        Event::Notification { .. } => Some(LostKind::Notification),
        Event::ClipboardWrite { .. } => Some(LostKind::ClipboardWrite),
        _ => None,
    }
}

fn session_id_of(event: &Event) -> Option<&SessionId> {
    match event {
        Event::Bell { id, .. }
        | Event::PromptMark { id, .. }
        | Event::Notification { id, .. }
        | Event::ClipboardWrite { id, .. } => Some(id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(droppable: usize, mandatory: usize) -> QueueBounds {
        QueueBounds {
            droppable,
            mandatory,
        }
    }

    fn sid(n: &str) -> SessionId {
        SessionId(n.to_string())
    }

    fn inst(n: &str) -> InstanceId {
        InstanceId(n.to_string())
    }

    fn bell(session: &str, instance: &str, at: u64) -> Event {
        Event::Bell {
            id: sid(session),
            instance: inst(instance),
            at,
        }
    }

    fn title(session: &str, instance: &str, text: &str) -> Event {
        Event::TitleChanged {
            id: sid(session),
            instance: inst(instance),
            title: text.to_string(),
        }
    }

    fn state(session: &str, instance: &str, state: SessionState) -> Event {
        Event::SessionState {
            id: sid(session),
            instance: inst(instance),
            state,
        }
    }

    fn drain(queue: &mut EventQueue) -> Vec<Event> {
        queue.poll(usize::MAX).events
    }

    /// Core EV-2, 6.2: the oldest droppable event is dropped first, and the marker takes the position of the first drop.
    #[test]
    fn a_full_droppable_queue_drops_the_oldest_and_leaves_one_marker_in_its_place() {
        let mut queue = EventQueue::new(bounds(3, 8));
        for at in 0..5 {
            queue.post_droppable(bell("s", "i", at));
        }
        let events = drain(&mut queue);
        assert_eq!(
            events,
            vec![
                Event::EventsLost {
                    id: sid("s"),
                    instance: inst("i"),
                    kinds: BTreeSet::from([LostKind::Bell]),
                    tap_dropped_bytes: None
                },
                bell("s", "i", 2),
                bell("s", "i", 3),
                bell("s", "i", 4),
            ]
        );
    }

    /// Core 6.2 class L: one marker per instance, and further drops add only to its kinds.
    #[test]
    fn a_session_has_one_marker_that_gains_kinds() {
        let mut queue = EventQueue::new(bounds(1, 8));
        let notification = Event::Notification {
            id: sid("s"),
            instance: inst("i"),
            source: NotificationSource::Osc9,
            title: None,
            body: "b".into(),
            truncated: false,
            at: 1,
        };
        queue.post_droppable(bell("s", "i", 0));
        queue.post_droppable(notification);
        queue.post_droppable(bell("s", "i", 2));
        let events = drain(&mut queue);
        let markers: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::EventsLost { kinds, .. } => Some(kinds.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            markers,
            vec![BTreeSet::from([LostKind::Bell, LostKind::Notification])]
        );
        assert_eq!(events.len(), 2);
    }

    /// Core EV-2: the oldest event of any session is dropped, and each session gets its own marker.
    #[test]
    fn a_drop_marks_the_session_that_lost_the_event() {
        let mut queue = EventQueue::new(bounds(2, 8));
        queue.post_droppable(bell("a", "ia", 0));
        queue.post_droppable(bell("b", "ib", 1));
        queue.post_droppable(bell("b", "ib", 2));
        let events = drain(&mut queue);
        assert!(
            matches!(&events[0], Event::EventsLost { id, .. } if id == &sid("a")),
            "{events:?}"
        );
        assert_eq!(events.len(), 3);
    }

    /// Core EV-1, EV-6: a replaced keyed event carries the latest value and moves to the newer position.
    #[test]
    fn a_replaced_keyed_event_moves_to_the_position_of_the_newer_observation() {
        let mut queue = EventQueue::new(bounds(4, 8));
        queue.post_keyed(title("s", "i", "Title1"));
        queue.post_droppable(bell("s", "i", 2));
        queue.post_keyed(title("s", "i", "Title3"));
        assert_eq!(
            drain(&mut queue),
            vec![bell("s", "i", 2), title("s", "i", "Title3")]
        );
    }

    /// Core 6.2: a key is the instance and the kind, so two kinds do not replace each other, and neither do two instances.
    #[test]
    fn keys_are_per_instance_and_kind() {
        let mut queue = EventQueue::new(bounds(4, 8));
        queue.post_keyed(title("s", "i1", "a"));
        queue.post_keyed(title("s", "i2", "b"));
        queue.post_keyed(Event::CwdChanged {
            id: sid("s"),
            instance: inst("i1"),
            cwd: "/".into(),
        });
        assert_eq!(drain(&mut queue).len(), 3);
    }

    /// Core IN-4: a client `Activity` is keyed by its route, and the output `Activity` is a key of its own.
    #[test]
    fn activity_is_keyed_by_source_and_route() {
        let mut queue = EventQueue::new(bounds(4, 8));
        let activity = |source, at| Event::Activity {
            id: sid("s"),
            instance: inst("i"),
            source,
            at,
        };
        queue.post_keyed(activity(ActivitySource::Output, 1));
        queue.post_keyed(activity(ActivitySource::Client(RouteId(1)), 2));
        queue.post_keyed(activity(ActivitySource::Client(RouteId(2)), 3));
        queue.post_keyed(activity(ActivitySource::Client(RouteId(1)), 4));
        let events = drain(&mut queue);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[2],
            activity(ActivitySource::Client(RouteId(1)), 4),
            "the replaced route event takes the later position"
        );
    }

    /// Core EV-5b: a mandatory event waits when the queue has no room, and is never replaced or dropped.
    #[test]
    fn a_full_mandatory_queue_refuses_and_hands_the_event_back() {
        let mut queue = EventQueue::new(bounds(4, 2));
        assert!(queue
            .post_mandatory(state("a", "ia", SessionState::Created))
            .is_ok());
        assert!(queue
            .post_mandatory(state("b", "ib", SessionState::Created))
            .is_ok());
        let refused = queue
            .post_mandatory(state("c", "ic", SessionState::Created))
            .unwrap_err();
        assert_eq!(*refused, state("c", "ic", SessionState::Created));
        assert!(!queue.has_mandatory_room());
        assert_eq!(drain(&mut queue).len(), 2);
    }

    /// Core EV-5a: `Completed` needs no mandatory room, because its slot was reserved at `begin`.
    #[test]
    fn completed_does_not_count_against_the_mandatory_bound() {
        let mut queue = EventQueue::new(bounds(4, 1));
        queue
            .post_mandatory(state("a", "ia", SessionState::Created))
            .unwrap();
        queue.post_completed(OpId(1), OpResult::Ok(OpOutput::Unit));
        assert_eq!(queue.len(), 2);
        assert!(!queue.has_mandatory_room());
    }

    /// Core EV-5d: a poll reports the mandatory room that it freed, and not the room of a `Completed`.
    #[test]
    fn a_poll_reports_the_mandatory_room_that_it_freed() {
        let mut queue = EventQueue::new(bounds(4, 2));
        queue
            .post_mandatory(state("a", "ia", SessionState::Created))
            .unwrap();
        queue.post_completed(OpId(1), OpResult::Ok(OpOutput::Unit));
        queue
            .post_mandatory(state("b", "ib", SessionState::Created))
            .unwrap();
        let polled = queue.poll(2);
        assert_eq!(polled.events.len(), 2);
        assert_eq!(polled.freed_mandatory, 1);
        assert!(queue.has_mandatory_room());
    }

    /// Core 6.2 retirement: `Released` retires every unpolled K, L and D event of the instance, with no marker, and keeps
    /// the class M events of the instance and every event of another instance.
    #[test]
    fn retiring_an_instance_removes_its_k_l_and_d_events_and_creates_no_marker() {
        let mut queue = EventQueue::new(bounds(2, 8));
        queue.post_keyed(title("s", "old", "t"));
        queue.post_droppable(bell("s", "old", 1));
        queue.post_droppable(bell("s", "old", 2));
        queue.post_droppable(bell("s", "old", 3));
        queue
            .post_mandatory(state("s", "old", SessionState::Exited(exit())))
            .unwrap();
        queue.post_keyed(title("t", "live", "u"));
        queue.retire_instance(&inst("old"));
        assert_eq!(
            drain(&mut queue),
            vec![
                state("s", "old", SessionState::Exited(exit())),
                title("t", "live", "u")
            ]
        );
    }

    fn exit() -> Exit {
        Exit {
            code: Some(0),
            signal: None,
            cause: ExitCause::Normal,
        }
    }

    /// Core EV-5: after a retirement no drop of the retired instance can happen, so no marker appears for it.
    #[test]
    fn a_retired_instance_never_gets_a_marker_from_later_drops() {
        let mut queue = EventQueue::new(bounds(2, 8));
        queue.post_droppable(bell("s", "old", 1));
        queue.retire_instance(&inst("old"));
        for at in 0..4 {
            queue.post_droppable(bell("t", "live", at));
        }
        let events = drain(&mut queue);
        assert!(events.iter().all(|e| !matches!(
            e,
            Event::EventsLost { instance, .. } if instance == &inst("old")
        )));
    }

    /// Core EV-5: `RouteClosed` retires the unpolled class K events that the route keys, and no other.
    #[test]
    fn retiring_a_route_removes_its_activity_only() {
        let mut queue = EventQueue::new(bounds(4, 8));
        let activity = |source| Event::Activity {
            id: sid("s"),
            instance: inst("i"),
            source,
            at: 1,
        };
        queue.post_keyed(activity(ActivitySource::Client(RouteId(1))));
        queue.post_keyed(activity(ActivitySource::Client(RouteId(2))));
        queue.post_keyed(activity(ActivitySource::Output));
        queue.retire_route(RouteId(1));
        assert_eq!(
            drain(&mut queue),
            vec![
                activity(ActivitySource::Client(RouteId(2))),
                activity(ActivitySource::Output)
            ]
        );
    }

    /// Core EV-5: a service's `Released` retires its keyed events.
    #[test]
    fn retiring_a_service_removes_its_keyed_events() {
        let mut queue = EventQueue::new(bounds(4, 8));
        let a = ServiceId([1; 32]);
        let b = ServiceId([2; 32]);
        queue.post_keyed(Event::ServiceReadable { id: a, lane: 0 });
        queue.post_keyed(Event::ServiceWritable { id: a, lane: 1 });
        queue.post_keyed(Event::ServiceReadable { id: b, lane: 0 });
        queue.retire_service(a);
        assert_eq!(
            drain(&mut queue),
            vec![Event::ServiceReadable { id: b, lane: 0 }]
        );
    }

    /// Core OR-1: a poll takes at most `max` events and keeps the rest in order.
    #[test]
    fn a_poll_takes_at_most_max_in_order() {
        let mut queue = EventQueue::new(bounds(4, 8));
        queue.post_completed(OpId(1), OpResult::Ok(OpOutput::Unit));
        queue.post_completed(OpId(2), OpResult::Ok(OpOutput::Unit));
        queue.post_completed(OpId(3), OpResult::Ok(OpOutput::Unit));
        let first = queue.poll(2).events;
        let second = queue.poll(2).events;
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 1);
        assert!(matches!(first[0], Event::Completed { op: OpId(1), .. }));
        assert!(matches!(second[0], Event::Completed { op: OpId(3), .. }));
        assert!(queue.poll(2).events.is_empty());
    }

    /// Core TP-1: a worker-reported loss creates the marker, with the dropped tap bytes added up.
    #[test]
    fn a_worker_reported_loss_creates_or_extends_the_marker() {
        let mut queue = EventQueue::new(bounds(4, 8));
        queue.post_lost(&sid("s"), &inst("i"), LostKind::Tap, 10);
        queue.post_lost(&sid("s"), &inst("i"), LostKind::Query, 0);
        queue.post_lost(&sid("s"), &inst("i"), LostKind::Tap, 5);
        assert_eq!(
            drain(&mut queue),
            vec![Event::EventsLost {
                id: sid("s"),
                instance: inst("i"),
                kinds: BTreeSet::from([LostKind::Tap, LostKind::Query]),
                tap_dropped_bytes: Some(15)
            }]
        );
    }

    /// `take_posted` counts every post, including a replacement, and resets (the `events_posted` of `PumpReport`).
    #[test]
    fn posted_counts_every_post_and_resets() {
        let mut queue = EventQueue::new(bounds(4, 8));
        queue.post_keyed(title("s", "i", "a"));
        queue.post_keyed(title("s", "i", "b"));
        queue.post_completed(OpId(1), OpResult::Ok(OpOutput::Unit));
        assert_eq!(queue.take_posted(), 3);
        assert_eq!(queue.take_posted(), 0);
    }
}
