//! `ReadFacts` (Core ST-7) at the worker: the records of the host's writes, their end states (steward ruling R-101 Q1-Q3),
//! sequence exhaustion (R-101, follow-up), and a cursor across an adoption (R-101 Q6).

use super::*;
use botster_core_contract::prelude::KeyInput;
use botster_route_codec::prelude::{Key, KeyEvent, NamedKey};

/// The facts that a `ReadFacts` answers now.
pub(super) fn read_facts(w: &mut World, after: Option<InputCursor>) -> Facts {
    let actions = w.send(&HostMsg::Op {
        req: 900,
        op: Op::ReadFacts {
            session: SessionId("s".into()),
            after,
        },
    });
    w.reports(&actions)
        .into_iter()
        .find_map(|m| match m {
            WorkerMsg::Done {
                req: 900,
                result: OpResult::Ok(OpOutput::Facts(facts)),
            } => Some(facts),
            _ => None,
        })
        .expect("ReadFacts answers")
}

pub(super) fn status_updates(facts: &Facts) -> Vec<(InputRecordState, Option<u64>)> {
    facts
        .input
        .input_records
        .iter()
        .filter_map(|e| match e {
            InputRecordEntry::StatusUpdate(u) => Some((u.state, u.pty_bytes)),
            _ => None,
        })
        .collect()
}

pub(super) fn records(facts: &Facts) -> Vec<InputRecord> {
    facts
        .input
        .input_records
        .iter()
        .filter_map(|e| match e {
            InputRecordEntry::Record(r) => Some(r.clone()),
            _ => None,
        })
        .collect()
}

/// R-101 Q1: a host write that fails with an OS error is `partial`, with its exact PTY bytes, 0 included.
#[test]
fn a_failed_write_is_partial_with_its_exact_pty_bytes_even_zero() {
    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    w.feed(Input::PtyWritten(Err(22)));
    w.send(&write(2, b"abc", None));
    w.feed(Input::PtyWritten(Ok(2)));
    w.feed(Input::PtyWritten(Err(22)));
    let facts = read_facts(&mut w, None);
    assert_eq!(
        status_updates(&facts),
        [
            (InputRecordState::Partial, Some(0)),
            (InputRecordState::Partial, Some(2))
        ]
    );
}

/// R-101 Q3: a payload whose kind the worker cannot write is `NotWritten(Unsupported{what})`, in the op's result and in the
/// record, never an error.
#[test]
fn a_kind_that_cannot_be_written_is_not_written_unsupported_in_the_result_and_the_record() {
    let mut w = World::running();
    let actions = w.send(&HostMsg::Op {
        req: 1,
        op: Op::WriteInput {
            session: SessionId("s".into()),
            payload: InputPayload::Key(KeyInput {
                key: Key::Named(NamedKey("not_a_key".into())),
                shifted_key: None,
                base_layout_key: None,
                mods: vec![],
                event: KeyEvent::Press,
                text: None,
                repeat: None,
            }),
            guard: None,
        },
    });
    let reason = NotWrittenReason::Unsupported {
        what: UnsupportedWhat::NamedKey,
    };
    let result = input_result(&mut w, &actions, 1).expect("the write completes");
    assert_eq!(result.outcome, WriteOutcome::NotWritten(reason));
    let facts = read_facts(&mut w, None);
    assert_eq!(records(&facts)[0].kind, InputKind::Key);
    assert_eq!(
        status_updates(&facts),
        [(InputRecordState::NotWritten { reason }, Some(0))]
    );
}

/// R-101 Q2 (host side): a write refused while the payload is being stopped keeps `Stopping` in its record.
#[test]
fn a_write_while_stopping_keeps_stopping_in_its_record() {
    let mut w = World::running();
    w.send(&HostMsg::Stop);
    w.send(&write(1, b"x", None));
    let facts = read_facts(&mut w, None);
    assert_eq!(
        status_updates(&facts),
        [(
            InputRecordState::NotWritten {
                reason: NotWrittenReason::Stopping
            },
            Some(0)
        )]
    );
}

/// R-101 follow-up: with no sequence number left for a record and its status update, a host write is refused
/// `NotWritten(Refused)` with exact zero; no record is made, `input_rev{host}` does not move, and `ReadFacts` still answers.
#[test]
fn sequence_exhaustion_refuses_a_host_write_with_exact_zero_and_no_record() {
    let mut w = World::running();
    let max = facts::Facts::max_seq();
    w.worker.facts.as_mut().unwrap().use_up_to(max - 1);
    let actions = w.send(&write(1, b"x", None));
    let result = input_result(&mut w, &actions, 1).expect("the write completes");
    assert_eq!(
        result.outcome,
        WriteOutcome::NotWritten(NotWrittenReason::Refused)
    );
    assert_eq!(
        (result.payload_bytes_written, result.pty_bytes_written),
        (0, 0)
    );
    assert!(pty_writes(&actions).is_empty());
    assert!(
        !w.reports(&actions).iter().any(|m| matches!(
            m,
            WorkerMsg::Observed {
                observation: Observation::HostInput { .. }
            }
        )),
        "no host revision step"
    );
    let facts = read_facts(&mut w, None);
    assert!(facts.input.input_records.is_empty());
    assert_eq!(facts.input.host.rev, InputRev(0));
}

/// R-101 Q6: an adoption keeps the ordinal and the sequence, so a cursor read before it reads incrementally after it.
#[test]
fn a_cursor_read_before_an_adoption_reads_incrementally_after_it() {
    let mut w = World::running();
    w.send(&write(1, b"a", None));
    w.feed(Input::PtyWritten(Ok(1)));
    let before = read_facts(&mut w, None);
    let actions = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &actions);
    w.send(&write(2, b"b", None));
    w.feed(Input::PtyWritten(Ok(1)));
    let after = read_facts(&mut w, Some(before.input.next_cursor));
    assert!(!after.input.gap);
    let new = records(&after);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].payload, Some(HexBytes(b"b".to_vec())));
    assert_eq!(new[0].cursor.0 >> 29, spec().cursor_ordinal);
}

/// Steward ruling R-103 (3): the adoption fence drops the old host's queued writes (DESIGN.md 3.4). No cancel was issued,
/// so their records end `not_written{refused}` with 0 PTY bytes, not `cancelled`.
#[test]
fn a_queued_write_that_the_fence_drops_ends_not_written_refused() {
    let mut w = World::running();
    w.send(&write(1, b"a", None));
    w.send(&write(2, b"b", None));
    let actions = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &actions);
    w.feed(Input::PtyWritten(Ok(1)));
    let facts = read_facts(&mut w, None);
    let ends = status_updates(&facts);
    assert!(
        ends.contains(&(
            InputRecordState::NotWritten {
                reason: NotWrittenReason::Refused
            },
            Some(0)
        )),
        "{ends:?}"
    );
    assert!(
        !ends.iter().any(|(s, _)| *s == InputRecordState::Cancelled),
        "{ends:?}"
    );
    assert!(
        ends.contains(&(InputRecordState::Written, Some(1))),
        "{ends:?}"
    );
}
