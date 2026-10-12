use super::*;
use botster_route_codec::prelude::{Key, KeyEvent, NamedKey};
use std::time::Duration;

/// The real instant that a test gives the log as the worker's start: the one read of the real clock in these tests (the
/// machine crates read no clock: plan 2.3c).
#[allow(clippy::disallowed_methods)]
fn start() -> Instant {
    Instant::now()
}

fn limits(kept: u32, bytes: u64) -> CoreLimits {
    CoreLimits {
        input_events_kept: kept,
        input_events_bytes: bytes,
        ..CoreLimits::default()
    }
}

fn client() -> InputSource {
    InputSource::Client {
        route_tag: None,
        owner: None,
    }
}

fn bytes(b: &[u8]) -> InputPayload {
    InputPayload::Bytes {
        bytes: HexBytes(b.to_vec()),
    }
}

fn record(entry: &InputRecordEntry) -> &InputRecord {
    match entry {
        InputRecordEntry::Record(r) => r,
        other => panic!("not a record: {other:?}"),
    }
}

fn cursor_of(entry: &InputRecordEntry) -> u64 {
    match entry {
        InputRecordEntry::Record(r) => r.cursor.0,
        InputRecordEntry::StatusUpdate(u) => u.cursor.0,
        other => panic!("{other:?}"),
    }
}

/// ST-7: a record is `pending` at receipt with a null `pty_bytes`; its end is an appended status update with a new cursor
/// that names the record. The record itself is not edited.
#[test]
fn a_record_is_pending_at_receipt_and_its_end_is_an_appended_update() {
    let t0 = start();
    let mut facts = Facts::new(7, &CoreLimits::default(), t0);
    let id = facts
        .received(
            t0 + Duration::from_millis(5),
            client(),
            Content::Payload(&bytes(b"hi")),
        )
        .unwrap();
    let first = facts.read(t0, None);
    assert_eq!(first.records.len(), 1);
    let r = record(&first.records[0]);
    assert_eq!(r.state, InputRecordState::Pending);
    assert_eq!(r.pty_bytes, None);
    assert_eq!(r.at, 5);
    assert_eq!(r.payload, Some(HexBytes(b"hi".to_vec())));
    assert_eq!(r.cursor.0, 7 << SEQ_BITS | 1);
    assert_eq!(first.next_cursor, r.cursor);

    facts.ended(id, InputRecordState::Written, Some(2));
    let later = facts.read(t0, Some(first.next_cursor));
    assert!(!later.gap);
    assert_eq!(
        later.records,
        [InputRecordEntry::StatusUpdate(InputStatusUpdate {
            cursor: InputCursor(7 << SEQ_BITS | 2),
            updates: InputCursor(7 << SEQ_BITS | 1),
            state: InputRecordState::Written,
            pty_bytes: Some(2),
        })]
    );
    assert_eq!(
        record(&facts.read(t0, None).records[0]).state,
        InputRecordState::Pending
    );
}

/// ST-7: the oldest entry goes first past `input_events_kept`; a reader after a dropped entry gets `gap`, a reader after
/// a kept one does not.
#[test]
fn retention_by_count_drops_the_oldest_and_reports_the_gap() {
    let t0 = start();
    let mut facts = Facts::new(1, &limits(3, u64::MAX), t0);
    let mut cursors = Vec::new();
    for b in [b"a", b"b", b"c", b"d"] {
        let id = facts
            .received(t0, client(), Content::Payload(&bytes(b)))
            .unwrap();
        cursors.push(facts.read(t0, None).next_cursor);
        facts.ended(id, InputRecordState::Written, Some(1));
    }
    let all = facts.read(t0, None);
    assert_eq!(all.records.len(), 3);
    assert!(all.gap, "retention removed an entry");
    assert_eq!(
        all.records.iter().map(cursor_of).collect::<Vec<_>>(),
        [1 << SEQ_BITS | 6, 1 << SEQ_BITS | 7, 1 << SEQ_BITS | 8]
    );
    assert!(facts.read(t0, Some(cursors[0])).gap);
    assert!(!facts.read(t0, Some(InputCursor(1 << SEQ_BITS | 5))).gap);
}

/// ST-7: the JSON bytes of the kept list stay within `input_events_bytes`, and the newest record and its status update are
/// never dropped.
#[test]
fn retention_by_bytes_keeps_the_list_within_its_bound_and_the_newest_pair() {
    let t0 = start();
    let bound = 1200;
    let mut facts = Facts::new(1, &limits(1000, bound), t0);
    for i in 0..40u8 {
        let id = facts
            .received(t0, client(), Content::Payload(&bytes(&[i; 40])))
            .unwrap();
        facts.ended(id, InputRecordState::Written, Some(40));
        let read = facts.read(t0, None);
        let len = serde_json::to_vec(&read.records).unwrap().len() as u64;
        assert!(len <= bound, "{len} > {bound}");
        assert_eq!(
            facts.list_len(),
            len,
            "the kept length is the link's length"
        );
        let n = read.records.len();
        assert!(matches!(
            read.records[n - 1],
            InputRecordEntry::StatusUpdate(_)
        ));
        assert_eq!(
            record(&read.records[n - 2]).payload,
            Some(HexBytes(vec![i; 40]))
        );
    }
}

/// ST-7: a newest record that would not leave room for its status update is cut to the longest payload that does, with
/// `payload_truncated`; with no room for any payload, to its minimal form.
#[test]
fn an_oversize_newest_record_is_truncated_then_minimal() {
    let t0 = start();
    let probe = Facts::new(1, &CoreLimits::default(), t0);
    // The shortest record with a payload: an empty one, cut.
    let mut cut = record_of(
        probe.cursor(1),
        client(),
        0,
        Content::Payload(&bytes(b"")),
        usize::MAX,
    );
    cut.payload_truncated = true;
    let empty = json_len(&InputRecordEntry::Record(cut));
    let bound = 3 + probe.status_room + empty + 20;
    let mut facts = Facts::new(1, &limits(64, bound), t0);
    let id = facts
        .received(t0, client(), Content::Payload(&bytes(&[0xab; 100])))
        .unwrap();
    facts.ended(id, InputRecordState::Written, Some(100));
    let read = facts.read(t0, None);
    let r = record(&read.records[0]);
    assert!(r.payload_truncated);
    let kept = r.payload.as_ref().unwrap().0.len();
    // The room is the empty prefix and 20 characters: 10 bytes, two hex characters each.
    assert_eq!(kept, 10);
    assert!(facts.list_len() <= bound);
    // The kept record itself fits its room. The list alone does not prove it: the room keeps space for the largest status
    // update, and this one is shorter.
    assert!(json_len(&InputRecordEntry::Record(r.clone())) <= bound - 3 - probe.status_room);
    // The longest prefix that fits: one byte more does not.
    let mut longer = r.clone();
    longer.payload = Some(HexBytes(vec![0xab; kept + 1]));
    assert!(json_len(&InputRecordEntry::Record(longer)) > bound - 3 - probe.status_room);

    let mut tight = Facts::new(1, &limits(64, 3 + probe.status_room + empty - 1), t0);
    tight
        .received(t0, client(), Content::Payload(&bytes(b"x")))
        .unwrap();
    let r = tight.read(t0, None).records[0].clone();
    assert_eq!(record(&r).payload, None);
    assert!(record(&r).payload_truncated);
    assert_eq!(record(&r).kind, InputKind::Bytes);
}

/// ST-7: a cursor of another ordinal (another instance) is a gap, and the reader gets every kept entry.
#[test]
fn a_cursor_of_another_ordinal_is_a_gap() {
    let t0 = start();
    let mut facts = Facts::new(2, &CoreLimits::default(), t0);
    facts
        .received(t0, InputSource::Host, Content::Payload(&bytes(b"x")))
        .unwrap();
    let read = facts.read(t0, Some(InputCursor(1 << SEQ_BITS | 1)));
    assert!(read.gap);
    assert_eq!(read.records.len(), 1);
}

/// R-101 follow-up: the last sequence numbers are kept for the status updates of the records that have not ended. When
/// no record can be made, `received` is `Exhausted`, every open record can still end, and a read still answers.
#[test]
fn sequence_exhaustion_refuses_a_record_and_every_open_record_still_ends() {
    let t0 = start();
    let mut facts = Facts::new(3, &CoreLimits::default(), t0);
    facts.last_seq = MAX_SEQ - 5;
    let a = facts
        .received(t0, client(), Content::Payload(&bytes(b"a")))
        .unwrap();
    let b = facts
        .received(t0, client(), Content::Payload(&bytes(b"b")))
        .unwrap();
    assert!(!facts.can_receive(), "3 left, 2 reserved");
    assert_eq!(
        facts.received(t0, client(), Content::Payload(&bytes(b"c"))),
        Err(Exhausted)
    );
    facts.ended(a, InputRecordState::Written, Some(1));
    facts.ended(b, InputRecordState::Written, Some(1));
    assert_eq!(facts.last_seq, MAX_SEQ - 1);
    let c = facts.received(t0, InputSource::Host, Content::Payload(&bytes(b"c")));
    assert_eq!(c, Err(Exhausted), "2 needed, 1 left");
    let read = facts.read(t0, None);
    assert_eq!(read.next_cursor, InputCursor(3 << SEQ_BITS | (MAX_SEQ - 1)));
    assert!(read.next_cursor.0 < 1 << 53);
}

/// ST-7: `age_ms` counts from the last client input, or from the worker's start before any; host input does not move it.
#[test]
fn age_counts_from_the_last_client_input_on_the_workers_clock() {
    let t0 = start();
    let ms = Duration::from_millis;
    let mut facts = Facts::new(1, &CoreLimits::default(), t0);
    assert_eq!(facts.read(t0 + ms(30), None).age_ms, 30);
    facts
        .received(t0 + ms(40), client(), Content::Payload(&bytes(b"x")))
        .unwrap();
    facts
        .received(
            t0 + ms(50),
            InputSource::Host,
            Content::Payload(&bytes(b"y")),
        )
        .unwrap();
    assert_eq!(facts.read(t0 + ms(70), None).age_ms, 30);
}

/// ST-7: the client tail keeps the last `input_tail_bytes` bytes exactly; `tail_complete` turns false at the first drop.
#[test]
fn the_client_tail_keeps_its_last_bytes_and_marks_a_drop() {
    let t0 = start();
    let mut facts = Facts::new(
        1,
        &CoreLimits {
            input_tail_bytes: 4,
            ..CoreLimits::default()
        },
        t0,
    );
    facts.client_wrote(b"abcd");
    let read = facts.read(t0, None);
    assert_eq!(read.tail, b"abcd");
    assert!(read.tail_complete);
    facts.client_wrote(b"ef");
    let read = facts.read(t0, None);
    assert_eq!(read.tail, b"cdef");
    assert!(!read.tail_complete);
}

/// Steward ruling R-103: `payload_truncated` means only that something was cut. A record with no truthful payload has none
/// and is not `payload_truncated`, whole or in its minimal form; a minimal form that dropped a payload is.
#[test]
fn a_record_with_no_payload_is_never_payload_truncated() {
    let t0 = start();
    let mut facts = Facts::new(1, &CoreLimits::default(), t0);
    facts
        .received(t0, client(), Content::NoPayload(InputKind::Text))
        .unwrap();
    let r = record(&facts.read(t0, None).records[0]).clone();
    assert_eq!(
        (r.kind, r.payload, r.payload_truncated),
        (InputKind::Text, None, false)
    );

    let whole = record_of(
        probe_cursor(),
        client(),
        0,
        Content::NoPayload(InputKind::Focus),
        usize::MAX,
    );
    assert!(!minimal(whole).payload_truncated, "nothing to cut");
    let with_payload = record_of(
        probe_cursor(),
        client(),
        0,
        Content::Payload(&bytes(b"x")),
        usize::MAX,
    );
    assert!(
        minimal(with_payload).payload_truncated,
        "the payload was cut"
    );
}

fn probe_cursor() -> InputCursor {
    InputCursor(1 << SEQ_BITS | 1)
}

fn key() -> InputPayload {
    InputPayload::Key(KeyInput {
        key: Key::Named(NamedKey("enter".into())),
        shifted_key: None,
        base_layout_key: None,
        mods: vec![],
        event: KeyEvent::Press,
        text: Some("x".repeat(64)),
        repeat: None,
    })
}

/// ST-7: a log that retention never cut is no gap, with or without a cursor of its own ordinal.
#[test]
fn a_read_with_nothing_dropped_has_no_gap() {
    let t0 = start();
    let mut facts = Facts::new(1, &CoreLimits::default(), t0);
    assert!(!facts.read(t0, None).gap);
    facts
        .received(t0, client(), Content::Payload(&bytes(b"x")))
        .unwrap();
    assert!(!facts.read(t0, None).gap);
    assert!(!facts.read(t0, Some(InputCursor(1 << SEQ_BITS))).gap);
}

/// ST-7: a list whose JSON length is exactly `input_events_bytes` is within the bound: nothing is dropped.
#[test]
fn retention_at_exactly_the_byte_bound_drops_nothing() {
    let t0 = start();
    let run = |bound: u64| {
        let mut facts = Facts::new(1, &limits(1000, bound), t0);
        for b in [b"a", b"b"] {
            let id = facts
                .received(t0, client(), Content::Payload(&bytes(b)))
                .unwrap();
            facts.ended(id, InputRecordState::Written, Some(1));
        }
        facts
    };
    let exact = run(u64::MAX).list_len();
    let facts = run(exact);
    assert_eq!(facts.list_len(), exact);
    let read = facts.read(t0, None);
    assert_eq!(read.records.len(), 4);
    assert!(!read.gap);
    assert!(run(exact - 1).read(t0, None).gap, "one byte less drops one");
}

/// R-101 follow-up: the largest cursor is the largest ordinal and the largest sequence number, 2^53 - 1, and the room kept
/// for a status update holds every status update at that cursor.
#[test]
fn the_largest_cursor_and_the_room_of_a_status_update() {
    assert_eq!(LARGEST_CURSOR, (MAX_ORDINAL << SEQ_BITS) + MAX_SEQ);
    assert_eq!(LARGEST_CURSOR, (1 << 53) - 1);
    let largest = largest_status_update();
    let lens: Vec<u64> = [
        InputRecordState::Written,
        InputRecordState::Unknown,
        InputRecordState::NotWritten {
            reason: NotWrittenReason::ModePreconditionFailed,
        },
        InputRecordState::NotWritten {
            reason: NotWrittenReason::Unsupported {
                what: UnsupportedWhat::WheelDirection,
            },
        },
    ]
    .into_iter()
    .map(|state| {
        json_len(&InputRecordEntry::StatusUpdate(InputStatusUpdate {
            cursor: InputCursor(LARGEST_CURSOR),
            updates: InputCursor(LARGEST_CURSOR),
            state,
            pty_bytes: Some(u64::MAX),
        }))
    })
    .collect();
    assert!(lens.iter().all(|&n| n <= largest), "{lens:?} {largest}");
    assert!(lens.contains(&largest), "{lens:?} {largest}");
}

/// ST-7: a record keeps at most `cap` payload bytes, and is `payload_truncated` only when it kept fewer than it got.
#[test]
fn a_record_keeps_its_payload_up_to_the_cap() {
    for (cap, kept, cut) in [(2, 2, true), (3, 3, false), (4, 3, false)] {
        let r = record_of(
            probe_cursor(),
            client(),
            0,
            Content::Payload(&bytes(b"abc")),
            cap,
        );
        assert_eq!(r.payload, Some(HexBytes(b"abc"[..kept].to_vec())), "{cap}");
        assert_eq!(r.payload_truncated, cut, "{cap}");
    }
}

/// ST-7, R-103: a record keeps the fields of its kind: text as its bytes, a mouse event, a focus change, a key.
#[test]
fn a_record_keeps_the_fields_of_its_kind() {
    let of = |payload: &InputPayload| {
        record_of(
            probe_cursor(),
            client(),
            0,
            Content::Payload(payload),
            usize::MAX,
        )
    };
    let text = of(&InputPayload::Text { text: "hi".into() });
    assert_eq!(
        (text.kind, text.payload),
        (InputKind::Text, Some(HexBytes(b"hi".to_vec())))
    );
    let mouse: MouseInput = serde_json::from_value(serde_json::json!({
        "action": "press", "button": "left", "row": 0, "col": 3, "mods": []
    }))
    .unwrap();
    let r = of(&InputPayload::Mouse(mouse.clone()));
    assert_eq!(
        (r.kind, r.mouse, r.payload),
        (InputKind::Mouse, Some(mouse), None)
    );
    let r = of(&InputPayload::Focus { focused: true });
    assert_eq!(
        (r.kind, r.focus, r.payload),
        (InputKind::Focus, Some(true), None)
    );
    let r = of(&key());
    assert_eq!(r.kind, InputKind::Key);
    assert_eq!(r.text.as_deref(), Some("x".repeat(64).as_str()));
}

/// ST-7: with room for exactly the empty payload, the newest record keeps an empty payload, `payload_truncated`; it is not
/// minimal.
#[test]
fn a_newest_record_with_room_for_only_an_empty_payload_keeps_it() {
    let t0 = start();
    let probe = Facts::new(1, &CoreLimits::default(), t0);
    let mut cut = record_of(
        probe.cursor(1),
        client(),
        0,
        Content::Payload(&bytes(b"")),
        usize::MAX,
    );
    cut.payload_truncated = true;
    let empty = json_len(&InputRecordEntry::Record(cut));
    let mut facts = Facts::new(1, &limits(64, 3 + probe.status_room + empty), t0);
    facts
        .received(t0, client(), Content::Payload(&bytes(b"xyz")))
        .unwrap();
    let r = record(&facts.read(t0, None).records[0]).clone();
    assert_eq!(r.payload, Some(HexBytes(Vec::new())));
    assert!(r.payload_truncated);
}

/// ST-7: a newest record with no payload that does not fit is minimal, even when only `payload_truncated` would make it
/// fit: a record with no payload has no prefix to cut.
#[test]
fn an_oversize_record_with_no_payload_is_minimal() {
    let t0 = start();
    let probe = Facts::new(1, &CoreLimits::default(), t0);
    let whole = record_of(
        probe.cursor(1),
        client(),
        0,
        Content::Payload(&key()),
        usize::MAX,
    );
    let len = json_len(&InputRecordEntry::Record(whole));
    let mut facts = Facts::new(1, &limits(64, 3 + probe.status_room + len - 1), t0);
    facts
        .received(t0, client(), Content::Payload(&key()))
        .unwrap();
    let r = record(&facts.read(t0, None).records[0]).clone();
    assert_eq!(
        (r.kind, r.key, r.text, r.payload),
        (InputKind::Key, None, None, None)
    );
    assert!(r.payload_truncated);
}

/// Steward ruling R-116: the newest record and its status update are always kept. When their minimal form alone exceeds
/// `input_events_bytes` (route tags of control bytes at `max_route_tag_bytes`, which JSON writes as six bytes each, with
/// `input_events_bytes` at the 9B minimum), the list keeps both, with the full source, drops every older record and
/// reports `gap`. This is the only case where the list exceeds the bound.
#[test]
fn protected_records_over_the_byte_bound_are_kept_with_a_gap_and_nothing_older() {
    let t0 = start();
    let tag_bytes = CoreLimits::default().max_route_tag_bytes;
    let minimum = 2 * (128 + 2 * u64::from(tag_bytes));
    let mut facts = Facts::new(1, &limits(64, minimum), t0);
    let older = facts
        .received(t0, client(), Content::Payload(&bytes(b"a")))
        .unwrap();
    facts.ended(older, InputRecordState::Written, Some(1));
    let tag = "\u{1}".repeat(tag_bytes as usize);
    let source = InputSource::Client {
        route_tag: Some(tag.clone()),
        owner: Some(tag),
    };
    let newest = facts
        .received(t0, source.clone(), Content::Payload(&bytes(b"bc")))
        .unwrap();
    facts.ended(newest, InputRecordState::Written, Some(2));
    let read = facts.read(t0, None);
    assert!(read.gap, "the older record was dropped");
    let [InputRecordEntry::Record(r), InputRecordEntry::StatusUpdate(u)] = read.records.as_slice()
    else {
        panic!("only the protected pair: {:?}", read.records);
    };
    assert_eq!(u.updates, r.cursor);
    assert_eq!(r.source, source, "the full source is kept");
    assert_eq!(
        (r.kind, r.payload.as_ref()),
        (InputKind::Bytes, None),
        "the minimal form"
    );
    assert!(r.payload_truncated);
    let len = serde_json::to_vec(&read.records).unwrap().len() as u64;
    assert!(len > minimum, "{len} <= {minimum}");
    assert_eq!(facts.list_len(), len);
}

/// Steward ruling R-116: the overflow itself is the `gap`, with no older record to drop. The first input of an empty log
/// overflows while it is pending and after it ends, and every read reports `gap` while the list is over the bound,
/// incremental reads of the same instance included. A list within the bound reports no `gap` (the other retention tests).
#[test]
fn an_overflowing_first_input_reports_gap_on_every_read() {
    let t0 = start();
    let tag_bytes = CoreLimits::default().max_route_tag_bytes;
    let minimum = 2 * (128 + 2 * u64::from(tag_bytes));
    let mut facts = Facts::new(1, &limits(64, minimum), t0);
    let tag = "\u{1}".repeat(tag_bytes as usize);
    let source = InputSource::Client {
        route_tag: Some(tag.clone()),
        owner: Some(tag),
    };
    let id = facts
        .received(t0, source.clone(), Content::Payload(&bytes(b"x")))
        .unwrap();
    let pending = facts.read(t0, None);
    let [InputRecordEntry::Record(r)] = pending.records.as_slice() else {
        panic!("the pending record: {:?}", pending.records);
    };
    assert_eq!((r.state, &r.source), (InputRecordState::Pending, &source));
    assert!(facts.list_len() > minimum);
    assert!(pending.gap, "a pending record over the bound is a gap");
    assert!(
        facts.read(t0, Some(pending.next_cursor)).gap,
        "an incremental read too"
    );
    assert!(facts.read(t0, Some(InputCursor(1 << SEQ_BITS))).gap);

    facts.ended(id, InputRecordState::Written, Some(1));
    let done = facts.read(t0, None);
    assert_eq!(done.records.len(), 2);
    assert!(facts.list_len() > minimum);
    assert!(done.gap, "the completed pair over the bound is a gap");
    assert!(facts.read(t0, Some(pending.next_cursor)).gap);
    assert!(facts.read(t0, Some(done.next_cursor)).gap);
}

fn host_write(facts: &mut Facts, t0: Instant, b: &[u8]) -> RecordId {
    facts
        .received(t0, InputSource::Host, Content::Payload(&bytes(b)))
        .unwrap()
}

/// Steward ruling R-117: one client input, then host writes past `input_events_kept`. The list keeps the record of the
/// current client `rev` and its status update, and the newest host record and its status update, over the count bound,
/// and every read is `gap`.
#[test]
fn the_current_client_record_and_the_newest_host_record_are_both_kept() {
    let t0 = start();
    let mut facts = Facts::new(1, &limits(2, u64::MAX), t0);
    let client_id = facts
        .received(t0, client(), Content::Payload(&bytes(b"a")))
        .unwrap();
    facts.ended(client_id, InputRecordState::Written, Some(1));
    let after_client = facts.read(t0, None).next_cursor;
    for b in [b"b", b"c", b"d"] {
        let id = host_write(&mut facts, t0, b);
        facts.ended(id, InputRecordState::Written, Some(1));
    }
    let read = facts.read(t0, None);
    let [InputRecordEntry::Record(c), InputRecordEntry::StatusUpdate(cu), InputRecordEntry::Record(h), InputRecordEntry::StatusUpdate(hu)] =
        read.records.as_slice()
    else {
        panic!("both protected pairs: {:?}", read.records);
    };
    assert_eq!(
        (&c.source, &c.payload),
        (&client(), &Some(HexBytes(b"a".to_vec())))
    );
    assert_eq!(cu.updates, c.cursor);
    assert_eq!(
        (&h.source, &h.payload),
        (&InputSource::Host, &Some(HexBytes(b"d".to_vec())))
    );
    assert_eq!(hu.updates, h.cursor);
    assert!(
        read.gap,
        "older host records were dropped, and the list is over the count bound"
    );
    assert!(facts.read(t0, Some(after_client)).gap);
    assert!(
        facts.read(t0, Some(read.next_cursor)).gap,
        "the count overflow itself"
    );
}

/// Steward ruling R-117: before the first client input only the newest record is protected; a pending client record stays
/// protected while host writes pass it, and its status update joins it when it ends. A later client record takes over the
/// protection.
#[test]
fn a_pending_client_record_stays_protected_until_a_later_client_record() {
    let t0 = start();
    let mut facts = Facts::new(1, &limits(2, u64::MAX), t0);
    for b in [b"h", b"i"] {
        let id = host_write(&mut facts, t0, b);
        facts.ended(id, InputRecordState::Written, Some(1));
    }
    let hosts = facts.read(t0, None);
    assert_eq!(
        hosts.records.len(),
        2,
        "only the newest pair before any client input"
    );
    assert_eq!(
        record(&hosts.records[0]).payload,
        Some(HexBytes(b"i".to_vec()))
    );
    let first = facts
        .received(t0, client(), Content::Payload(&bytes(b"a")))
        .unwrap();
    for b in [b"j", b"k"] {
        let id = host_write(&mut facts, t0, b);
        facts.ended(id, InputRecordState::Written, Some(1));
    }
    facts.ended(first, InputRecordState::Written, Some(1));
    let read = facts.read(t0, None);
    let payloads: Vec<_> = read
        .records
        .iter()
        .map(|e| match e {
            InputRecordEntry::Record(r) => r.payload.clone(),
            _ => None,
        })
        .collect();
    let a = Some(HexBytes(b"a".to_vec()));
    let k = Some(HexBytes(b"k".to_vec()));
    assert_eq!(payloads, [a.clone(), k, None, None], "{:?}", read.records);
    let second = facts
        .received(t0, client(), Content::Payload(&bytes(b"b")))
        .unwrap();
    facts.ended(second, InputRecordState::Written, Some(1));
    let read = facts.read(t0, None);
    assert_eq!(read.records.len(), 2, "{:?}", read.records);
    assert_eq!(
        record(&read.records[0]).payload,
        Some(HexBytes(b"b".to_vec()))
    );
}

/// Steward ruling R-117 with R-116 item 3: a host record beside the protected client pair is cut so that the list stays
/// within `input_events_bytes`, the room of the client record's later status update included. Nothing older is dropped,
/// so there is no `gap`.
#[test]
fn a_newest_host_record_leaves_room_for_the_protected_client_pair() {
    let t0 = start();
    let bound = 1280;
    let mut facts = Facts::new(1, &limits(64, bound), t0);
    let client_id = facts
        .received(t0, client(), Content::Payload(&bytes(&[7; 200])))
        .unwrap();
    let host = host_write(&mut facts, t0, &[9; 256]);
    assert!(facts.list_len() <= bound, "{} > {bound}", facts.list_len());
    facts.ended(client_id, InputRecordState::Written, Some(200));
    facts.ended(host, InputRecordState::Written, Some(256));
    let read = facts.read(t0, None);
    assert_eq!(read.records.len(), 4, "{:?}", read.records);
    assert_eq!(
        record(&read.records[0]).payload,
        Some(HexBytes(vec![7; 200]))
    );
    let h = record(&read.records[1]);
    assert!(h.payload_truncated);
    assert!(h
        .payload
        .as_ref()
        .is_some_and(|p| !p.0.is_empty() && p.0.len() < 256));
    let len = serde_json::to_vec(&read.records).unwrap().len() as u64;
    assert!(len <= bound, "{len} > {bound}");
    assert!(!read.gap);
}

/// #320 R320-SI-5: each protected entry past `input_events_bytes` is at most `PROTECTED_ENTRY_FIXED` and its two route
/// tags at six bytes a byte (`facts_reply_bound`): the largest minimal record, with tags of control bytes at
/// `max_route_tag_bytes`, the largest numbers and the longest kind and state, and the largest status update.
#[test]
fn a_protected_entry_is_within_its_share_of_the_facts_reply_bound() {
    use botster_core_link::msg::PROTECTED_ENTRY_FIXED;
    let tag_bytes = CoreLimits::default().max_route_tag_bytes;
    let tag = "\u{1}".repeat(tag_bytes as usize);
    let source = InputSource::Client {
        route_tag: Some(tag.clone()),
        owner: Some(tag),
    };
    let payload = bytes(&[1; 64]);
    let mut record = record_of(
        InputCursor(LARGEST_CURSOR),
        source,
        u64::MAX,
        Content::Payload(&payload),
        8,
    );
    record.state = InputRecordState::NotWritten {
        reason: NotWrittenReason::Unsupported {
            what: UnsupportedWhat::PixelPosition,
        },
    };
    record.pty_bytes = Some(u64::MAX);
    let mut longest = 0;
    for kind in [
        InputKind::Bytes,
        InputKind::Text,
        InputKind::Paste,
        InputKind::Key,
        InputKind::Mouse,
        InputKind::Focus,
    ] {
        record.kind = kind;
        let cut = minimal(record.clone());
        assert!(cut.payload_truncated);
        longest = longest.max(json_len(&InputRecordEntry::Record(cut)));
    }
    let share = PROTECTED_ENTRY_FIXED + 12 * u64::from(tag_bytes);
    assert!(longest <= share, "{longest} > {share}");
    assert!(longest > 12 * u64::from(tag_bytes), "the tags are escaped");
    assert!(largest_status_update() <= PROTECTED_ENTRY_FIXED);
}

/// Steward ruling R-117: the room of a newest host record beside the client pair is exact. While the client record is
/// pending, the room of its status update is kept; once its status update exists, its own length counts. A cut payload
/// fills the room to within one byte (hex is two characters a byte).
#[test]
fn the_room_of_a_host_record_beside_the_client_pair_is_exact() {
    let t0 = start();
    let bound = 1280;
    let mut facts = Facts::new(1, &limits(64, bound), t0);
    let client_id = facts
        .received(t0, client(), Content::Payload(&bytes(&[7; 200])))
        .unwrap();
    let check = |facts: &mut Facts| -> RecordId {
        let client = facts.client;
        let pair: Vec<u64> = facts
            .entries
            .iter()
            .filter(|k| Some(k.record) == client)
            .map(|k| k.len)
            .collect();
        let update = if pair.len() == 1 {
            facts.status_room
        } else {
            0
        };
        let other = pair.iter().sum::<u64>() + update + 2;
        let room = bound - 3 - facts.status_room - other;
        let id = host_write(facts, t0, &[9; 256]);
        let kept = facts.entries.back().unwrap();
        assert!(record(&kept.entry).payload_truncated);
        assert!(
            kept.len <= room && room - kept.len <= 1,
            "{} vs {room}",
            kept.len
        );
        id
    };
    let first = check(&mut facts);
    facts.ended(client_id, InputRecordState::Written, Some(200));
    facts.ended(first, InputRecordState::Written, Some(256));
    let second = check(&mut facts);
    facts.ended(second, InputRecordState::Written, Some(256));
    assert!(facts.list_len() <= bound);
}
