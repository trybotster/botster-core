//! A client's input in the facts (Core ST-7, IN-4): one `rev` step and one record at receipt, the record's IN-2 reason when
//! the input is refused (steward ruling R-101 Q2), sequence exhaustion (R-101, follow-up), and the records of the inputs
//! that a closing route discards (P5's mapping (4)).

use super::super::read_facts::{read_facts, records, status_updates};
use super::*;

fn not_written(reason: NotWrittenReason) -> (InputRecordState, Option<u64>) {
    (InputRecordState::NotWritten { reason }, Some(0))
}

/// R-101 Q2: the wire merges `Stopping` into `session_ended`; the record keeps `Stopping`.
#[test]
fn a_client_input_while_stopping_is_session_ended_on_the_wire_and_stopping_in_its_record() {
    let (mut w, mut client, _) = attached(limits());
    w.send(&HostMsg::Stop);
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(1, b"x")),
    });
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals(&client),
        vec![(Some(Op(1)), RefusalReason::SessionEnded)]
    );
    let facts = read_facts(&mut w, None);
    assert_eq!(
        status_updates(&facts),
        [not_written(NotWrittenReason::Stopping)]
    );
    assert_eq!(facts.input.client.rev, InputRev(1));
}

/// R-101 Q2 (P5's mapping (2)): a view-only route's input is refused `not_writable` and recorded `not_written{refused}`,
/// at receipt, with its payload.
#[test]
fn a_view_only_input_is_recorded_refused_at_receipt() {
    let mut w = World::running();
    let mut opts = options();
    opts.input = false;
    let mut client = Client::default();
    let bound = attach(&mut w, DescriptorId(1), RouteId(1), opts, limits());
    client.take_all(&mut w, RouteId(1), bound);
    w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(0, b"x")),
    });
    let facts = read_facts(&mut w, None);
    let recorded = records(&facts);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].payload, Some(HexBytes(b"x".to_vec())));
    assert!(matches!(recorded[0].source, InputSource::Client { .. }));
    assert_eq!(
        status_updates(&facts),
        [not_written(NotWrittenReason::Refused)]
    );
}

/// R-101 follow-up: with no sequence number left, a client frame is refused `refused`, `input_rev{client}` does not
/// advance, no record is made, and `ReadFacts` still answers.
#[test]
fn sequence_exhaustion_refuses_a_client_frame_with_no_rev_step() {
    let (mut w, mut client, _) = attached(limits());
    let max = crate::worker::facts::Facts::max_seq();
    w.worker.facts.as_mut().unwrap().use_up_to(max - 1);
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(1, b"x")),
    });
    let all = client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals(&client),
        vec![(Some(Op(1)), RefusalReason::Refused)]
    );
    assert_eq!(received(&mut w, &all), 0, "no rev step");
    assert!(pty_writes(&all).is_empty());
    let facts = read_facts(&mut w, None);
    assert!(facts.input.input_records.is_empty());
    assert_eq!(facts.input.client.rev, InputRev(0));
}

/// P5's mapping (4): the received inputs that wait for room when the route closes are discarded; each record ends
/// `cancelled` with 0 PTY bytes, so no record stays pending.
#[test]
fn the_inputs_that_a_closing_route_discards_end_cancelled() {
    let mut w = with_limits(|l| {
        l.route_queue_bytes = 1000;
        l.input_events_kept = 1000;
        l.input_events_bytes = 1 << 20;
    });
    let mut client = Client::default();
    let bind = attach(&mut w, DescriptorId(1), RouteId(1), options(), limits());
    client.take_all(&mut w, RouteId(1), bind);
    let batch: Vec<u8> = (1..=50)
        .flat_map(|k| client_frame(&bytes_frame(k, b"x")))
        .collect();
    let read = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: batch,
    });
    let detach = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Revoked,
    });
    client.take_all(&mut w, RouteId(1), [read, detach].concat());
    let facts = read_facts(&mut w, None);
    let ends = status_updates(&facts);
    assert!(
        ends.contains(&(InputRecordState::Cancelled, Some(0))),
        "{ends:?}"
    );
    // Every record has ended, but the inputs at the admission point: they end at their turn.
    let ended: Vec<InputCursor> = facts
        .input
        .input_records
        .iter()
        .filter_map(|e| match e {
            InputRecordEntry::StatusUpdate(u) => Some(u.updates),
            _ => None,
        })
        .collect();
    let open = records(&facts)
        .iter()
        .filter(|r| !ended.contains(&r.cursor))
        .count();
    let held = w.worker.input.route_inputs(RouteId(1));
    assert!(held > 0);
    assert_eq!(open, held);
    assert_eq!(records(&facts).len(), 50);
}
