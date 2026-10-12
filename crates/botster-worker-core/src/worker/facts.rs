//! The worker's part of `ReadFacts` (Core ST-7): the input records of client and host input in one order, the client tail,
//! and the time of the last client input. Only the worker has them at one point of its ordering (AM-2).
//!
//! - **A record at receipt.** A record is made when the worker receives an input: a client frame in the same call that
//!   advances `input_rev{client}` (IN-4, IN-10), a host write when its `WriteInput` comes. Its state is `pending` and its
//!   `pty_bytes` is null.
//! - **Completion is an appended entry.** When the input's transaction ends, a status update with a new cursor is appended;
//!   the record is never edited, so an incremental reader that saw the pending record sees its end.
//! - **One cursor.** A cursor is `ordinal << 29 | seq` (steward rulings R-101 and its follow-up): the ordinal that the host
//!   gave this instance in its launch (a durable counter of the data directory, never a hash), and the sequence number of
//!   the entry in this log. An adoption keeps both, so a cursor read before it reads incrementally after it. A cursor of
//!   another ordinal is a gap. A sequence number is never reused. Each record that has not ended keeps one sequence number
//!   for its status update, so an end always has one. When no record can be made with that reserve, no record is made
//!   ([`Exhausted`]), and the worker refuses new input (R-101 follow-up).
//! - **Retention.** At most `input_events_kept` entries, and at most `input_events_bytes` bytes of the list as JSON, the
//!   form in which the link carries it (`input_records_cost`). The oldest entry goes first. The newest record and its
//!   status update are never dropped: a record whose payload would not leave room for its status update is cut to a
//!   shorter payload, and if needed to its minimal form (cursor, source, `at`, kind and state), with `payload_truncated`.
//! - **The client tail.** The last `input_tail_bytes` PTY bytes that client input wrote, exact; `tail_complete` turns
//!   false at the first byte that it drops.

use botster_core_contract::prelude::*;
use botster_route_codec::prelude::HexBytes;
use std::collections::VecDeque;
use std::time::Instant;

/// The bits of the sequence number in a cursor (R-101 follow-up). The ordinal is above them.
const SEQ_BITS: u32 = 29;
/// The bits of the ordinal: every cursor stays below 2^53, the largest integer that every JSON reader keeps exact.
pub(super) const ORDINAL_BITS: u32 = 24;
/// The largest ordinal.
pub(super) const MAX_ORDINAL: u64 = (1 << ORDINAL_BITS) - 1;
/// The largest sequence number.
const MAX_SEQ: u64 = (1 << SEQ_BITS) - 1;
const _: () = assert!(ORDINAL_BITS + SEQ_BITS <= 53);

/// What a record keeps of its input.
#[derive(Debug, Clone, Copy)]
pub(super) enum Content<'a> {
    /// The input's semantic payload (5.1A), its content cut at `input_event_payload_bytes`.
    Payload(&'a InputPayload),
    /// An input of this kind with an empty payload: a refused `paste_commit` or `file_commit` (steward ruling R-102).
    Empty(InputKind),
    /// An input of this kind whose payload the worker did not decode: no payload, `payload_truncated`.
    Undecoded(InputKind),
}

/// A record, by its sequence number in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecordId(u64);

/// The sequence numbers of this instance are used up: no entry can be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Exhausted;

/// The limits of ST-7 (Core 9B), as the log uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bounds {
    kept: usize,
    bytes: u64,
    payload: usize,
    tail: usize,
}

impl Bounds {
    fn of(limits: &CoreLimits) -> Bounds {
        let size = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
        Bounds {
            kept: size(limits.input_events_kept),
            bytes: limits.input_events_bytes,
            payload: size(limits.input_event_payload_bytes),
            tail: size(limits.input_tail_bytes),
        }
    }
}

/// A kept entry, with its sequence number, the sequence number of the record that it belongs to, and its length as JSON.
#[derive(Debug, Clone)]
struct Kept {
    seq: u64,
    record: u64,
    entry: InputRecordEntry,
    len: u64,
}

/// What a read returns from the log; the worker adds the revisions and the model's facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FactsRead {
    pub(super) records: Vec<InputRecordEntry>,
    pub(super) next_cursor: InputCursor,
    pub(super) gap: bool,
    pub(super) tail: Vec<u8>,
    pub(super) tail_complete: bool,
    pub(super) age_ms: u64,
}

/// The input facts of one instance.
#[derive(Debug)]
pub(super) struct Facts {
    ordinal: u64,
    last_seq: u64,
    /// The records that have not ended: each keeps one sequence number for its status update.
    open: u64,
    entries: VecDeque<Kept>,
    /// The sum of the entries' JSON lengths.
    entries_len: u64,
    /// The highest sequence number that retention dropped (0: none).
    dropped_upto: u64,
    /// The sequence number of the newest record: it and its status update are never dropped.
    newest: Option<u64>,
    bounds: Bounds,
    /// The largest JSON length of a status update: the room that the newest record leaves for its own.
    status_room: u64,
    tail: VecDeque<u8>,
    tail_complete: bool,
    start: Instant,
    last_client: Option<Instant>,
}

impl Facts {
    /// An empty log. `start` is the worker's start on its own clock: `at` and `age_ms` count from it.
    pub(super) fn new(ordinal: u64, limits: &CoreLimits, start: Instant) -> Facts {
        Facts {
            ordinal: ordinal.min(MAX_ORDINAL),
            last_seq: 0,
            open: 0,
            entries: VecDeque::new(),
            entries_len: 0,
            dropped_upto: 0,
            newest: None,
            bounds: Bounds::of(limits),
            status_room: largest_status_update(),
            tail: VecDeque::new(),
            tail_complete: true,
            start,
            last_client: None,
        }
    }

    fn cursor(&self, seq: u64) -> InputCursor {
        InputCursor(self.ordinal << SEQ_BITS | seq)
    }

    fn next_seq(&mut self) -> u64 {
        debug_assert!(self.last_seq < MAX_SEQ, "a sequence number is reserved");
        self.last_seq += 1;
        self.last_seq
    }

    /// Whether a record can be made: its own sequence number, the one of its status update, and the reserve of every
    /// record that has not ended.
    pub(super) fn can_receive(&self) -> bool {
        MAX_SEQ - self.last_seq >= self.open + 2
    }

    /// The receipt of an input: its record, `pending`, is the newest record from now on. A client input also sets the
    /// time of the last client input (`age_ms`).
    pub(super) fn received(
        &mut self,
        now: Instant,
        source: InputSource,
        content: Content<'_>,
    ) -> Result<RecordId, Exhausted> {
        if !self.can_receive() {
            return Err(Exhausted);
        }
        let seq = self.next_seq();
        self.open += 1;
        if matches!(source, InputSource::Client { .. }) {
            self.last_client = Some(now);
        }
        let at = millis(now.saturating_duration_since(self.start));
        let record = record_of(self.cursor(seq), source, at, content, self.bounds.payload);
        let entry = self.fitted(record);
        self.newest = Some(seq);
        self.push(seq, seq, entry);
        Ok(RecordId(seq))
    }

    /// The end of a record's transaction: a status update with a new cursor is appended, with the sequence number that the
    /// record kept. A record ends once.
    pub(super) fn ended(
        &mut self,
        record: RecordId,
        state: InputRecordState,
        pty_bytes: Option<u64>,
    ) {
        debug_assert!(self.open > 0, "a record ends once");
        self.open = self.open.saturating_sub(1);
        let seq = self.next_seq();
        let entry = InputRecordEntry::StatusUpdate(InputStatusUpdate {
            cursor: self.cursor(seq),
            updates: self.cursor(record.0),
            state,
            pty_bytes,
        });
        self.push(seq, record.0, entry);
    }

    /// PTY bytes that a client input wrote: they join the tail, and its oldest bytes go past `input_tail_bytes`.
    pub(super) fn client_wrote(&mut self, bytes: &[u8]) {
        self.tail.extend(bytes);
        while self.tail.len() > self.bounds.tail {
            self.tail.pop_front();
            self.tail_complete = false;
        }
    }

    /// The facts of the log now. With `after`, only the entries after it; `gap` when retention removed an entry after it,
    /// or when it is a cursor of another ordinal (then every kept entry is returned). Without `after`, every kept entry, and
    /// `gap` when retention ever removed one.
    pub(super) fn read(&self, now: Instant, after: Option<InputCursor>) -> FactsRead {
        let after = after.map(|InputCursor(c)| (c >> SEQ_BITS, c & MAX_SEQ));
        let (from, gap) = match after {
            Some((ordinal, seq)) if ordinal == self.ordinal => (seq, self.dropped_upto > seq),
            Some(_) => (0, true),
            None => (0, self.dropped_upto > 0),
        };
        let since = self.last_client.unwrap_or(self.start);
        FactsRead {
            records: self
                .entries
                .iter()
                .filter(|k| k.seq > from)
                .map(|k| k.entry.clone())
                .collect(),
            next_cursor: self.cursor(self.last_seq),
            gap,
            tail: self.tail.iter().copied().collect(),
            tail_complete: self.tail_complete,
            age_ms: millis(now.saturating_duration_since(since)),
        }
    }

    /// The JSON length of the kept list, as the link carries it: `[`, the entries with a comma between two, `]`.
    fn list_len(&self) -> u64 {
        let n = self.entries.len() as u64;
        2 + self.entries_len + n.saturating_sub(1)
    }

    fn push(&mut self, seq: u64, record: u64, entry: InputRecordEntry) {
        let len = json_len(&entry);
        self.entries_len += len;
        self.entries.push_back(Kept {
            seq,
            record,
            entry,
            len,
        });
        self.retain();
    }

    /// Drops the oldest entries that are not protected until both bounds hold.
    fn retain(&mut self) {
        while self.entries.len() > self.bounds.kept || self.list_len() > self.bounds.bytes {
            let newest = self.newest;
            let Some(at) = self.entries.iter().position(|k| Some(k.record) != newest) else {
                return;
            };
            let gone = self.entries.remove(at).expect("found");
            self.entries_len -= gone.len;
            self.dropped_upto = self.dropped_upto.max(gone.seq);
        }
    }

    /// The newest record as it is kept: whole when it leaves room for its status update in `input_events_bytes`; else with
    /// the longest payload that does; else in its minimal form.
    fn fitted(&self, record: InputRecord) -> InputRecordEntry {
        // The list of the protected pair: `[`, the record, `,`, the status update, `]`.
        let room = self.bounds.bytes.saturating_sub(3 + self.status_room);
        let fits = |r: &InputRecord| json_len(&InputRecordEntry::Record(r.clone())) <= room;
        if fits(&record) {
            return InputRecordEntry::Record(record);
        }
        let content = record.payload.as_ref().map_or(0, |p| p.0.len());
        // The longest payload prefix that fits: a binary search over its length.
        let (mut lo, mut hi) = (0usize, content);
        let cut = |n: usize| {
            let mut r = record.clone();
            r.payload = r.payload.map(|p| HexBytes(p.0[..n].to_vec()));
            r.payload_truncated = true;
            r
        };
        if record.payload.is_some() && fits(&cut(0)) {
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                if fits(&cut(mid)) {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            return InputRecordEntry::Record(cut(lo));
        }
        InputRecordEntry::Record(minimal(record))
    }
}

fn millis(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// The length of `entry` as JSON, the form in which the link carries it.
fn json_len(entry: &InputRecordEntry) -> u64 {
    serde_json::to_vec(entry).map_or(u64::MAX, |v| v.len() as u64)
}

/// The JSON length of the largest status update: both cursors at their largest, the longest state and the largest count.
fn largest_status_update() -> u64 {
    let cursor = InputCursor(MAX_ORDINAL << SEQ_BITS | MAX_SEQ);
    let states = [
        InputRecordState::NotWritten {
            reason: NotWrittenReason::ModePreconditionFailed,
        },
        InputRecordState::NotWritten {
            reason: NotWrittenReason::Unsupported {
                what: UnsupportedWhat::PixelPosition,
            },
        },
        InputRecordState::NotWritten {
            reason: NotWrittenReason::Unsupported {
                what: UnsupportedWhat::WheelDirection,
            },
        },
    ];
    states
        .into_iter()
        .map(|state| {
            json_len(&InputRecordEntry::StatusUpdate(InputStatusUpdate {
                cursor,
                updates: cursor,
                state,
                pty_bytes: Some(u64::MAX),
            }))
        })
        .max()
        .unwrap_or(0)
}

/// The record of a received input: the 5.1A fields as sent, and its content cut at `input_event_payload_bytes`.
fn record_of(
    cursor: InputCursor,
    source: InputSource,
    at: u64,
    content: Content<'_>,
    cap: usize,
) -> InputRecord {
    let mut record = InputRecord {
        cursor,
        source,
        at,
        kind: InputKind::Bytes,
        event: None,
        key: None,
        mods: None,
        shifted_key: None,
        base_layout_key: None,
        text: None,
        repeat: None,
        require_bracketed: None,
        mouse: None,
        focus: None,
        payload: None,
        payload_truncated: false,
        state: InputRecordState::Pending,
        pty_bytes: None,
    };
    let keep = |bytes: &[u8], record: &mut InputRecord| {
        let kept = &bytes[..bytes.len().min(cap)];
        record.payload_truncated = kept.len() < bytes.len();
        record.payload = Some(HexBytes(kept.to_vec()));
    };
    let payload = match content {
        Content::Payload(payload) => payload,
        Content::Empty(kind) => {
            record.kind = kind;
            record.payload = Some(HexBytes(Vec::new()));
            return record;
        }
        Content::Undecoded(kind) => {
            record.kind = kind;
            record.payload_truncated = true;
            return record;
        }
    };
    match payload {
        InputPayload::Bytes { bytes } => keep(&bytes.0, &mut record),
        InputPayload::Text { text } => {
            record.kind = InputKind::Text;
            keep(text.as_bytes(), &mut record);
        }
        InputPayload::Paste {
            bytes,
            require_bracketed,
        } => {
            record.kind = InputKind::Paste;
            record.require_bracketed = Some(*require_bracketed);
            keep(&bytes.0, &mut record);
        }
        InputPayload::Key(key) => {
            record.kind = InputKind::Key;
            record.key = Some(key.key.clone());
            record.shifted_key = key.shifted_key.clone();
            record.base_layout_key = key.base_layout_key.clone();
            record.mods = Some(key.mods.clone());
            record.event = Some(key.event);
            record.text = key.text.clone();
            record.repeat = key.repeat;
        }
        InputPayload::Mouse(mouse) => {
            record.kind = InputKind::Mouse;
            record.mouse = Some(mouse.clone());
        }
        InputPayload::Focus { focused } => {
            record.kind = InputKind::Focus;
            record.focus = Some(*focused);
        }
        // A later kind of the non-exhaustive enum: its kind is not known here, so the record keeps no content.
        _ => record.payload_truncated = true,
    }
    record
}

/// The minimal form of a record (ST-7): its cursor, source, `at`, kind and state, with `payload_truncated`.
fn minimal(record: InputRecord) -> InputRecord {
    InputRecord {
        cursor: record.cursor,
        source: record.source,
        at: record.at,
        kind: record.kind,
        event: None,
        key: None,
        mods: None,
        shifted_key: None,
        base_layout_key: None,
        text: None,
        repeat: None,
        require_bracketed: None,
        mouse: None,
        focus: None,
        payload: None,
        payload_truncated: true,
        state: record.state,
        pty_bytes: record.pty_bytes,
    }
}

#[cfg(test)]
mod tests;
