//! A route's paste (Codec TS-5d, TS-5b; Core DP-5, IN-8): the open paste of the route, its sequencing refusals, its bound,
//! its abort, and its commit as one transaction through the one encoder. The expected markers come from an independent
//! libghostty terminal (BUILD.md architecture rule 2).

use super::*;
use botster_route_codec::prelude::Input as Wire;

fn begin(op: u64) -> ToWorker {
    begin_with(op, None, None)
}

fn begin_with(op: u64, total_bytes: Option<u64>, require_bracketed: Option<bool>) -> ToWorker {
    ToWorker::Input(Wire::PasteBegin {
        op: Op(op),
        total_bytes,
        require_bracketed,
    })
}

fn chunk(op: u64, bytes: &[u8]) -> ToWorker {
    ToWorker::PasteChunk {
        op: Op(op),
        bytes: bytes.to_vec().into(),
    }
}

fn commit(op: u64, want_result: Option<bool>) -> ToWorker {
    ToWorker::Input(Wire::PasteCommit {
        op: Op(op),
        want_result,
    })
}

fn abort(op: u64) -> ToWorker {
    ToWorker::Input(Wire::PasteAbort { op: Op(op) })
}

/// A chunk, a commit and an abort of `op`.
fn unmatched(op: u64) -> [ToWorker; 3] {
    [chunk(op, b"x"), commit(op, None), abort(op)]
}

/// Sends each frame in a read of its own, and the client reads every frame that the worker writes.
fn send(w: &mut World, client: &mut Client, frames: &[ToWorker]) -> Vec<Action> {
    let mut all = Vec::new();
    for frame in frames {
        let actions = w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes: client_frame(frame),
        });
        all.extend(client.take_all(w, RouteId(1), actions));
    }
    all
}

/// The PTY takes `n` bytes of the write that is out.
fn pty_took(w: &mut World, client: &mut Client, n: usize) -> Vec<Action> {
    let actions = w.feed(Input::PtyWritten(Ok(n)));
    client.take_all(w, RouteId(1), actions)
}

/// Each PTY write, one entry per write.
fn writes(actions: &[Action]) -> Vec<Vec<u8>> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::PtyWrite(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

/// The `input_done` frames that the client read.
fn dones(client: &Client) -> Vec<(Op, u64)> {
    client
        .frames
        .iter()
        .filter_map(|f| match f {
            ToClient::InputDone(done) => Some((done.op, done.written_bytes)),
            _ => None,
        })
        .collect()
}

fn refused(ops: &[u64]) -> Vec<(Option<Op>, RefusalReason, Option<u64>)> {
    ops.iter()
        .map(|&op| (Some(Op(op)), RefusalReason::Refused, None))
        .collect()
}

/// The program turns bracketed paste on; the markers of the mode come from an independent terminal.
fn bracketed(w: &mut World, client: &mut Client) -> (Vec<u8>, Vec<u8>) {
    let on = b"\x1b[?2004h";
    let actions = w.feed(Input::PtyOutput(on.to_vec()));
    client.take_all(w, RouteId(1), actions);
    let mut expected = oracle();
    expected.vt_write(on);
    expected.paste_frame().expect("the mode is on")
}

fn wrapped((start, end): &(Vec<u8>, Vec<u8>), body: &[u8]) -> Vec<u8> {
    [&start[..], body, &end[..]].concat()
}

/// Codec TS-5d (R-5): a chunk, a commit and an abort with no open paste are each refused `refused` with their op, with zero
/// effect: no PTY byte, no paste opened, and the route stays open. A paste after them is written as sent.
#[test]
fn unmatched_paste_continuations_without_an_open_paste_are_refused() {
    let (mut w, mut client, _) = attached(limits());
    let mut all = send(&mut w, &mut client, &unmatched(1));
    assert_eq!(refusals_written(&client), refused(&[1, 1, 1]));
    assert!(writes(&all).is_empty());
    all.extend(send(
        &mut w,
        &mut client,
        &[begin(1), chunk(1, b"ab"), commit(1, None)],
    ));
    assert_eq!(writes(&all), [b"ab".to_vec()]);
    assert_eq!(refusals_written(&client).len(), 3, "no other refusal");
    assert!(!all.contains(&Action::RouteClose { route: RouteId(1) }));
}

/// Codec TS-5d (R-5): after its commit, a paste is not open. A chunk, a commit and an abort of its op are refused `refused`,
/// and none of them writes a byte.
#[test]
fn unmatched_paste_continuations_after_a_committed_paste_are_refused() {
    let (mut w, mut client, _) = attached(limits());
    let mut all = send(
        &mut w,
        &mut client,
        &[begin(51), chunk(51, b"x"), commit(51, None)],
    );
    all.extend(pty_took(&mut w, &mut client, 1));
    all.extend(send(&mut w, &mut client, &unmatched(51)));
    assert_eq!(writes(&all), [b"x".to_vec()]);
    assert_eq!(refusals_written(&client), refused(&[51, 51, 51]));
}

/// Codec TS-5d (R-5): a chunk, a commit or an abort of another op, and a second `paste_begin`, are refused `refused`. The open
/// paste keeps its bytes, takes its next chunk, and its commit writes all of them in one write.
#[test]
fn unmatched_paste_continuations_keep_a_different_open_paste() {
    let (mut w, mut client, _) = attached(limits());
    let mut all = send(&mut w, &mut client, &[begin(60), chunk(60, b"abc")]);
    all.extend(send(&mut w, &mut client, &unmatched(61)));
    all.extend(send(&mut w, &mut client, &[begin(62)]));
    assert!(writes(&all).is_empty());
    assert_eq!(refusals_written(&client), refused(&[61, 61, 61, 62]));
    all.extend(send(
        &mut w,
        &mut client,
        &[chunk(60, b"d"), commit(60, None)],
    ));
    assert_eq!(writes(&all), [b"abcd".to_vec()]);
}

/// Codec TS-5d, Core DP-5, AM-2: a paste is written at its commit, in the receive order, as one transaction. Input that the
/// route sent between its chunks is written first, and no byte of it is between the paste's bytes.
#[test]
fn a_paste_is_written_at_its_commit_as_one_transaction() {
    let (mut w, mut client, _) = attached(limits());
    let mut all = send(
        &mut w,
        &mut client,
        &[
            begin(1),
            chunk(1, b"AA"),
            bytes_frame(2, b"zz"),
            chunk(1, b"BB"),
        ],
    );
    assert_eq!(
        writes(&all),
        [b"zz".to_vec()],
        "the paste waits for its commit"
    );
    all.extend(send(&mut w, &mut client, &[commit(1, None)]));
    all.extend(pty_took(&mut w, &mut client, 2));
    assert_eq!(writes(&all), [b"zz".to_vec(), b"AABB".to_vec()]);
}

/// Codec TS-5b, Core IN-8: a paste that asks for its result is answered `input_done` with the PTY bytes after encoding,
/// markers included. A paste with `want_result` false or absent gets no frame.
#[test]
fn input_done_answers_only_a_paste_with_want_result_true() {
    let (mut w, mut client, _) = attached(limits());
    let markers = bracketed(&mut w, &mut client);
    let body = wrapped(&markers, b"hi");
    let mut all = Vec::new();
    for (op, want_result) in [(1, Some(true)), (2, Some(false)), (3, None)] {
        all.extend(send(
            &mut w,
            &mut client,
            &[begin(op), chunk(op, b"hi"), commit(op, want_result)],
        ));
        all.extend(pty_took(&mut w, &mut client, body.len()));
    }
    assert_eq!(writes(&all), [body.clone(), body.clone(), body.clone()]);
    assert_eq!(dones(&client), [(Op(1), body.len() as u64)]);
    assert!(refusals_written(&client).is_empty());
}

/// Codec TS-5e, Core IN-8: `require_bracketed` with the mode off at the write's start is refused `mode_precondition_failed`,
/// with zero effect: no PTY byte and no `input_done`. The route's next input is written.
#[test]
fn a_paste_that_requires_bracketed_mode_with_the_mode_off_is_refused_with_zero_effect() {
    let (mut w, mut client, _) = attached(limits());
    let all = send(
        &mut w,
        &mut client,
        &[
            begin_with(1, None, Some(true)),
            chunk(1, b"x"),
            commit(1, Some(true)),
            bytes_frame(2, b"ok"),
        ],
    );
    assert_eq!(
        refusals_written(&client),
        [(Some(Op(1)), RefusalReason::ModePreconditionFailed, None)]
    );
    assert_eq!(writes(&all), [b"ok".to_vec()]);
    assert!(dones(&client).is_empty());
}

/// Codec TS-5d: a paste of exactly `max_paste_bytes` is written. The first chunk that takes a paste over the bound is
/// refused `too_large` with zero effect, and the paste is discarded: its commit is then refused `refused`. A `total_bytes`
/// over the bound is refused `too_large` at `paste_begin` (R-9), and opens no paste; one at the bound opens it, and a
/// mismatch with the bytes sent has no effect.
#[test]
fn a_paste_over_max_paste_bytes_is_refused_too_large_with_zero_effect() {
    let (mut w, mut client, _) = attached(limits());
    let max = limits().max_paste_bytes;
    let half = vec![b'h'; usize::try_from(max / 2).unwrap()];
    assert_eq!(2 * half.len() as u64, max);
    let mut all = send(
        &mut w,
        &mut client,
        &[begin(1), chunk(1, &half), chunk(1, &half), commit(1, None)],
    );
    all.extend(pty_took(&mut w, &mut client, 2 * half.len()));
    assert_eq!(writes(&all), [[half.clone(), half.clone()].concat()]);
    all.extend(send(
        &mut w,
        &mut client,
        &[
            begin(2),
            chunk(2, &half),
            chunk(2, &half),
            chunk(2, b"o"),
            commit(2, None),
            begin_with(3, Some(max + 1), None),
            chunk(3, b"x"),
            begin_with(4, Some(max), None),
            chunk(4, b"y"),
            commit(4, None),
        ],
    ));
    assert_eq!(
        refusals_written(&client),
        [
            (Some(Op(2)), RefusalReason::TooLarge, None),
            (Some(Op(2)), RefusalReason::Refused, None),
            (Some(Op(3)), RefusalReason::TooLarge, None),
            (Some(Op(3)), RefusalReason::Refused, None),
        ]
    );
    assert_eq!(writes(&all)[1..], [b"y".to_vec()]);
}

/// Codec TS-5d, Core IN-4: an abort before the commit writes nothing and sends no frame, and the route can open its next
/// paste. Each paste frame is client input on receipt: it advances `input_rev{client}` once.
#[test]
fn an_abort_before_the_commit_writes_nothing_and_sends_no_frame() {
    let (mut w, mut client, _) = attached(limits());
    let frames = client.frames.len();
    let all = send(&mut w, &mut client, &[begin(1), chunk(1, b"x"), abort(1)]);
    assert!(writes(&all).is_empty());
    assert_eq!(client.frames.len(), frames, "no frame");
    assert_eq!(received(&mut w, &all), 3);
    let all = send(
        &mut w,
        &mut client,
        &[commit(1, None), begin(5), chunk(5, b"p"), commit(5, None)],
    );
    assert_eq!(writes(&all), [b"p".to_vec()]);
    assert_eq!(refusals_written(&client), refused(&[1]));
}

/// Codec TS-5b, Core IN-2: a paste that fails part way is refused `failed` with its PTY bytes, markers included, and gets no
/// `input_done`.
#[test]
fn a_partly_written_paste_is_refused_with_its_pty_bytes() {
    let (mut w, mut client, _) = attached(limits());
    let markers = bracketed(&mut w, &mut client);
    let part = markers.0.len() - 1;
    assert!(part > 0);
    send(
        &mut w,
        &mut client,
        &[begin(1), chunk(1, b"hi"), commit(1, Some(true))],
    );
    pty_took(&mut w, &mut client, part);
    let actions = w.feed(Input::PtyWritten(Err(5)));
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals_written(&client),
        [(Some(Op(1)), RefusalReason::Failed, Some(part as u64))]
    );
    assert!(dones(&client).is_empty());
}

/// Codec TS-5b, Core IN-8: an empty paste with the mode off writes nothing and is answered `input_done` with 0 bytes; with
/// the mode on, its markers are written and counted.
#[test]
fn an_empty_paste_writes_only_its_markers() {
    let (mut w, mut client, _) = attached(limits());
    let all = send(&mut w, &mut client, &[begin(1), commit(1, Some(true))]);
    assert!(writes(&all).is_empty());
    assert_eq!(dones(&client), [(Op(1), 0)]);
    let markers = bracketed(&mut w, &mut client);
    let empty = wrapped(&markers, b"");
    let mut all = send(&mut w, &mut client, &[begin(2), commit(2, Some(true))]);
    all.extend(pty_took(&mut w, &mut client, empty.len()));
    assert_eq!(dones(&client), [(Op(1), 0), (Op(2), empty.len() as u64)]);
    assert_eq!(writes(&all), [empty]);
}

/// Codec TS-5d: a paste that is not committed when its route closes writes nothing.
#[test]
fn a_paste_that_is_open_at_the_close_writes_nothing() {
    let (mut w, mut client, _) = attached(limits());
    let mut all = send(&mut w, &mut client, &[begin(1), chunk(1, b"x")]);
    let actions = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Detached,
    });
    all.extend(client.take_all(&mut w, RouteId(1), actions));
    assert!(writes(&all).is_empty());
    assert!(all.contains(&Action::RouteClose { route: RouteId(1) }));
}

/// DP-5: the route's read allowance counts the caller's bytes of its held input: a `text` frame and a committed paste at the
/// admission point. The chunks of an open paste are outside the input bound (`max_paste_bytes` bounds them).
#[test]
fn held_text_and_a_committed_paste_count_in_the_allowance() {
    let (mut w, mut client, _) = attached(limits());
    let bound = limits().route_input_queue_bytes as usize;
    let mut all = send(&mut w, &mut client, &[bytes_frame(1, &[b'a'; 100])]);
    assert_eq!(writes(&all), [vec![b'a'; 100]], "the write is out");
    all.extend(send(
        &mut w,
        &mut client,
        &[ToWorker::Text {
            op: Op(2),
            text: "xyz".into(),
        }],
    ));
    assert_eq!(allowance(&all, RouteId(1)), Some(bound - 100 - 3));
    all.extend(send(&mut w, &mut client, &[begin(3), chunk(3, b"pq")]));
    assert_eq!(allowance(&all, RouteId(1)), Some(bound - 100 - 3));
    all.extend(send(&mut w, &mut client, &[commit(3, None)]));
    assert_eq!(allowance(&all, RouteId(1)), Some(bound - 100 - 3 - 2));
}

/// Codec TS-5: `bytes` and `text` have no success acknowledgement. An empty one with an op is written as nothing, and gets no
/// `input_done`.
#[test]
fn empty_bytes_and_text_get_no_input_done() {
    let (mut w, mut client, _) = attached(limits());
    let all = send(
        &mut w,
        &mut client,
        &[
            bytes_frame(1, b""),
            ToWorker::Text {
                op: Op(2),
                text: String::new(),
            },
        ],
    );
    assert!(writes(&all).is_empty());
    assert!(dones(&client).is_empty());
    assert!(refusals_written(&client).is_empty());
}

/// Core DP-5, IN-2: a paste that the PTY took in part, then blocked, is refused `session_ended` with its PTY bytes when the
/// payload ends, before the route's `route_closed`.
#[test]
fn a_paste_blocked_at_the_payloads_end_is_refused_session_ended_with_its_pty_bytes() {
    let (mut w, mut client, _) = attached(limits());
    send(
        &mut w,
        &mut client,
        &[begin(1), chunk(1, b"abcdefgh"), commit(1, None)],
    );
    pty_took(&mut w, &mut client, 3);
    pty_took(&mut w, &mut client, 0);
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Code(0)));
    actions.extend(w.feed(Input::PtyDrained));
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals_written(&client),
        [(Some(Op(1)), RefusalReason::SessionEnded, Some(3))]
    );
    let refused = client
        .frames
        .iter()
        .position(|f| matches!(f, ToClient::InputRefused(_)))
        .unwrap();
    assert!(matches!(
        client.frames.last(),
        Some(ToClient::RouteClosed(_))
    ));
    assert!(refused < client.frames.len() - 1);
}
