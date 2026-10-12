use super::*;
use std::time::Duration;

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
    let t0 = Instant::now();
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
    let t0 = Instant::now();
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
    let t0 = Instant::now();
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
    let t0 = Instant::now();
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
    assert!(kept > 0 && kept < 100, "{kept}");
    assert!(facts.list_len() <= bound);
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
    let t0 = Instant::now();
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
    let t0 = Instant::now();
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
    let t0 = Instant::now();
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
    let t0 = Instant::now();
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
