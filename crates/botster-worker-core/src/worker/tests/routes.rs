//! The stream routes (P4a PR1; DESIGN.md "P4a: the stream route"): the handoff, the baseline at the consumed cut, live output
//! within the frame bound, the PTY read budget, the closes (PR1, PR2), and the client's input (PR2).

use super::*;
use botster_core_link::msg::Observation;
use botster_route_codec::prelude::{
    AttachFailedReason, CloseReason, Exit as WireExit, FrameBounds, HistoryState, Op,
    ProtocolErrorCode, RefusalReason, RouteClosed as RouteClosedFrame, StreamReader, ToClient,
    ToWorker,
};

/// The frame bound of the test routes: it carries the attach frames (`baseline_begin` is the largest, A19-1), and output
/// frames of `FRAME - 1` payload bytes.
const FRAME: u64 = 1000;

/// Limits that differ from every `CoreLimits` default, so a value the worker computed itself would show.
fn limits() -> AppliedRouteLimits {
    AppliedRouteLimits {
        max_frame_bytes: FRAME,
        max_screen_frame_bytes: CoreLimits::default().max_snapshot_bytes + 77,
        max_history_page_bytes: 4321,
        max_paste_bytes: 1234,
        max_query_bytes: 55,
        max_query_reply_bytes: 66,
        max_file_bytes: 7777,
        query_deadline: Duration::from_millis(1500),
        stall_deadline: Duration::from_millis(2500),
        stall_close_after: Duration::from_millis(3500),
        route_input_queue_bytes: 4444,
    }
}

fn options() -> AttachOptions {
    serde_json::from_value(serde_json::json!({"file_directory": "/tmp"})).unwrap()
}

/// The route side of the test: it takes every `RouteWrite` whole and decodes the frames with the route's applied bounds, so
/// a frame over its bound fails the test (DP-3).
struct Client {
    reader: StreamReader,
    frames: Vec<ToClient>,
    bounds: FrameBounds,
}

impl Default for Client {
    fn default() -> Client {
        Client::with(&limits())
    }
}

impl Client {
    fn with(limits: &AppliedRouteLimits) -> Client {
        Client {
            reader: StreamReader::default(),
            frames: Vec::new(),
            bounds: FrameBounds {
                max_frame: limits.max_frame_bytes,
                max_screen: limits.max_screen_frame_bytes,
                max_history: limits.max_history_page_bytes,
            },
        }
    }

    /// Answers every `RouteWrite` of `route` in `actions` as fully written, until the worker writes nothing more, and keeps
    /// the frames. Returns every action of the exchange.
    fn take_all(&mut self, w: &mut World, route: RouteId, mut actions: Vec<Action>) -> Vec<Action> {
        let mut all = Vec::new();
        loop {
            let writes: Vec<Vec<u8>> = actions
                .iter()
                .filter_map(|a| match a {
                    Action::RouteWrite { route: r, bytes } if *r == route => Some(bytes.clone()),
                    _ => None,
                })
                .collect();
            all.append(&mut actions);
            if writes.is_empty() {
                return all;
            }
            for bytes in writes {
                self.reader.push(&bytes);
                actions.extend(w.feed(Input::RouteWritten {
                    route,
                    result: Ok(bytes.len()),
                }));
            }
            let bounds = self.bounds;
            while let Some(frame) = self.reader.next_frame(&bounds).unwrap() {
                match ToClient::decode(&frame, &bounds).unwrap() {
                    botster_route_codec::prelude::Decoded::Frame(f) => self.frames.push(f),
                    other => panic!("{other:?}"),
                }
            }
        }
    }

    /// The bytes of the `output` frames after `live`, and the size of each frame.
    fn output(&self) -> (Vec<u8>, Vec<usize>) {
        let live = self
            .frames
            .iter()
            .position(|f| matches!(f, ToClient::Live))
            .expect("live");
        let mut bytes = Vec::new();
        let mut sizes = Vec::new();
        for f in &self.frames[live + 1..] {
            if let ToClient::Output { payload } = f {
                bytes.extend_from_slice(payload);
                sizes.push(f.encode().len());
            }
        }
        (bytes, sizes)
    }
}

fn attach(
    w: &mut World,
    route: RouteId,
    options: AttachOptions,
    limits: AppliedRouteLimits,
) -> Vec<Action> {
    w.send(&HostMsg::AttachRoute {
        route,
        options,
        limits,
    })
}

/// A running worker with one route attached and its baseline taken.
fn attached(limits: AppliedRouteLimits) -> (World, Client, Vec<Action>) {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut client = Client::with(&limits);
    let actions = attach(&mut w, RouteId(1), options(), limits);
    let all = client.take_all(&mut w, RouteId(1), actions);
    (w, client, all)
}

fn budgets(actions: &[Action]) -> Vec<Option<usize>> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::PtyReadBudget(b) => Some(*b),
            _ => None,
        })
        .collect()
}

/// DP-2, OU-9: each `AttachRoute` binds the oldest unbound descriptor, and the route's first frames are `attached`, the
/// baseline at one point, and `live`.
#[test]
fn an_attach_binds_the_oldest_descriptor_and_writes_the_baseline_then_live() {
    use botster_route_codec::prelude::{
        TYPE_ATTACHED, TYPE_BASELINE_BEGIN, TYPE_BASELINE_END, TYPE_LIVE, TYPE_SCREEN,
    };
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    w.feed(Input::Descriptor(DescriptorId(2)));
    let actions = attach(&mut w, RouteId(5), options(), limits());
    assert!(actions.contains(&Action::BindRoute {
        descriptor: DescriptorId(1),
        route: RouteId(5)
    }));
    let mut client = Client::default();
    client.take_all(&mut w, RouteId(5), actions);
    let kinds: Vec<u8> = client.frames.iter().map(ToClient::type_byte).collect();
    assert_eq!(
        kinds,
        vec![
            TYPE_ATTACHED,
            TYPE_BASELINE_BEGIN,
            TYPE_SCREEN,
            TYPE_BASELINE_END,
            TYPE_LIVE
        ]
    );
    let ToClient::BaselineBegin(begin) = &client.frames[1] else {
        unreachable!()
    };
    assert_eq!((begin.rows, begin.cols), (24, 80));
    let ToClient::BaselineEnd(end) = &client.frames[3] else {
        unreachable!()
    };
    assert_eq!(end.history, HistoryState::NotRequested);
    let second = attach(&mut w, RouteId(6), options(), limits());
    assert!(second.contains(&Action::BindRoute {
        descriptor: DescriptorId(2),
        route: RouteId(6)
    }));
}

/// OU-1 (lead ruling on PR1): the worker announces and enforces exactly the limits that the host returned and sent, not
/// values computed from `CoreLimits`.
#[test]
fn the_worker_announces_and_enforces_exactly_the_limits_that_the_host_sent() {
    let (mut w, mut client, _) = attached(limits());
    let ToClient::Attached(attached) = &client.frames[0] else {
        unreachable!()
    };
    let l = &attached.limits;
    assert_eq!(
        (
            l.max_frame_bytes,
            l.max_screen_frame_bytes,
            l.max_history_page_bytes
        ),
        (FRAME, CoreLimits::default().max_snapshot_bytes + 77, 4321)
    );
    assert_eq!(
        (l.max_paste_bytes, l.max_file_bytes, l.max_query_reply_bytes),
        (1234, 7777, 66)
    );
    assert_eq!(
        (
            l.query_client_deadline_ms,
            l.stall_deadline_ms,
            l.stall_close_after_ms
        ),
        (1500, 2500, 3500)
    );
    // `max_frame_bytes` holds the type byte and `FRAME - 1` bytes of output.
    let out: Vec<u8> = (0..2500u32).map(|i| b'a' + (i % 26) as u8).collect();
    let actions = w.feed(Input::PtyOutput(out.clone()));
    client.take_all(&mut w, RouteId(1), actions);
    let (bytes, sizes) = client.output();
    assert_eq!(bytes, out);
    assert_eq!(
        sizes,
        vec![1000, 1000, 503],
        "each frame is within the host's bound"
    );
}

/// DP-2: an `AttachRoute` with no descriptor is a host fault, and the link closes.
#[test]
fn an_attach_route_without_a_descriptor_closes_the_link() {
    let mut w = World::running();
    let actions = attach(&mut w, RouteId(1), options(), limits());
    assert!(actions.contains(&Action::LinkClose), "{actions:?}");
    assert!(!actions
        .iter()
        .any(|a| matches!(a, Action::BindRoute { .. })));
}

/// OU-9 (DESIGN.md "Frames"): output that the model holds and has not applied at the bind (a lone ESC after an unfinished
/// OSC) is after the cut: it is the route's first output, and the later bytes follow, each byte once.
#[test]
fn output_that_the_model_holds_at_the_bind_is_the_first_output_after_live() {
    let mut w = World::running();
    w.feed(Input::PtyOutput(b"\x1b]2;t\x1b".to_vec()));
    let held = w.worker.model.as_ref().unwrap().unfed().to_vec();
    assert!(!held.is_empty(), "the model holds the ESC");
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), limits());
    let mut client = Client::default();
    client.take_all(&mut w, RouteId(1), actions);
    let actions = w.feed(Input::PtyOutput(b"\\after".to_vec()));
    client.take_all(&mut w, RouteId(1), actions);
    let mut expected = held;
    expected.extend_from_slice(b"\\after");
    assert_eq!(client.output().0, expected);
}

fn with_limits(f: impl FnOnce(&mut CoreLimits)) -> World {
    let mut w = World::linked();
    let mut launch = spec();
    f(&mut launch.limits);
    w.send(&HostMsg::Launch(Box::new(launch)));
    w.feed(Input::Spawned(Ok(PAYLOAD)));
    w
}

fn closes(w: &mut World, actions: &[Action]) -> Vec<WorkerMsg> {
    w.reports(actions)
        .into_iter()
        .filter(|m| matches!(m, WorkerMsg::RouteClosed { .. }))
        .collect()
}

/// OU-9, F75: a snapshot over `max_snapshot_bytes` cannot be offered. The only frame is
/// `route_closed{attach_failed{snapshot_too_large}}`, then the transport closes and the host is told once.
#[test]
fn a_snapshot_over_max_snapshot_bytes_closes_the_route_with_no_baseline() {
    let mut w = with_limits(|l| l.max_snapshot_bytes = 8);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), limits());
    let mut client = Client::default();
    let all = client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        client.frames,
        vec![ToClient::RouteClosed(RouteClosedFrame {
            reason: CloseReason::AttachFailed {
                reason: AttachFailedReason::SnapshotTooLarge
            },
            exit: None
        })]
    );
    assert!(all.contains(&Action::RouteClose { route: RouteId(1) }));
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None
        }]
    );
}

/// F75: the route's own screen frame bound is checked apart from the native limit: a snapshot within `max_snapshot_bytes`
/// that, with its type byte, passes the route's `max_screen_frame_bytes` is refused the same way.
#[test]
fn a_snapshot_over_the_routes_screen_frame_bound_closes_the_route() {
    let mut tight = limits();
    tight.max_screen_frame_bytes = 8;
    let (mut w, client, all) = attached(tight);
    assert!(matches!(
        client.frames.as_slice(),
        [ToClient::RouteClosed(RouteClosedFrame {
            reason: CloseReason::AttachFailed {
                reason: AttachFailedReason::SnapshotTooLarge
            },
            ..
        })]
    ));
    assert_eq!(closes(&mut w, &all).len(), 1);
}

/// OU-1, R-44: no format that the client lists can be emitted (the host refuses such an attach first): the worker cannot
/// make a working route from the options, so the handoff failed. No frame is written, the transport closes, and the host
/// is told `HandoffFailed` once.
#[test]
fn an_attach_with_no_common_format_fails_the_handoff_with_no_frame() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.terminal_formats = vec!["no_such_format".into()];
    let actions = attach(&mut w, RouteId(1), opts, limits());
    assert_handoff_failed_with_no_frame(&mut w, &actions);
}

/// OU-1: `attached.features` is the intersection of the client's route features with the worker's (none yet).
#[test]
fn the_features_are_the_intersection_with_the_workers() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.route_features = vec!["output_deflate".into(), "x".into()];
    let actions = attach(&mut w, RouteId(1), opts, limits());
    let mut client = Client::default();
    client.take_all(&mut w, RouteId(1), actions);
    let ToClient::Attached(attached) = &client.frames[0] else {
        unreachable!()
    };
    assert!(attached.features.is_empty());
}

/// OU-3d: the PTY read budget is the free payload space of the route: it shrinks while frames wait, grows as the transport
/// accepts bytes, and the limit is lifted when the last route ends.
#[test]
fn the_pty_read_budget_follows_the_free_queue_space() {
    let mut w = World::running();
    assert!(
        budgets(&w.feed(Input::PtyOutput(b"no route".to_vec()))).is_empty(),
        "no route, no limit"
    );
    w.feed(Input::Descriptor(DescriptorId(1)));
    let bound = attach(&mut w, RouteId(1), options(), limits());
    let first = *budgets(&bound).last().expect("a budget at the bind");
    let first = first.expect("a limit");
    assert!(first < CoreLimits::default().route_queue_bytes as usize);
    let more = w.feed(Input::PtyOutput(vec![b'a'; 100]));
    let smaller = budgets(&more)
        .last()
        .copied()
        .flatten()
        .expect("a smaller limit");
    assert!(smaller < first, "{smaller} < {first}");
    let mut client = Client::default();
    let all = client.take_all(&mut w, RouteId(1), [bound, more].concat());
    let freed = budgets(&all).last().copied().flatten().expect("a limit");
    assert!(freed > smaller, "written bytes free the queue");
    // A write error ends the only route: the limit is lifted.
    w.feed(Input::PtyOutput(b"z".to_vec()));
    let ended = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Err(32),
    });
    assert_eq!(budgets(&ended).last(), Some(&None));
}

/// OU-2b: a terminal write error is a failed close: the transport closes at once, nothing more is written, and the host is
/// told `WriteFailed` once. `Ok(0)` is not an error: the write waits for `RouteWritable`.
#[test]
fn a_write_error_closes_the_route_write_failed_once() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), limits());
    assert!(actions
        .iter()
        .any(|a| matches!(a, Action::RouteWrite { .. })));
    let waiting = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(0),
    });
    assert!(!waiting
        .iter()
        .any(|a| matches!(a, Action::RouteWrite { .. } | Action::RouteClose { .. })));
    let again = w.feed(Input::RouteWritable { route: RouteId(1) });
    assert!(
        again.iter().any(|a| matches!(a, Action::RouteWrite { .. })),
        "{again:?}"
    );
    let failed = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Err(32),
    });
    assert!(failed.contains(&Action::RouteClose { route: RouteId(1) }));
    assert_eq!(
        closes(&mut w, &failed),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::WriteFailed,
            route_tag: None
        }]
    );
    let after = w.feed(Input::PtyOutput(b"x".to_vec()));
    assert!(
        !after.iter().any(|a| matches!(a, Action::RouteWrite { .. })),
        "the route is gone"
    );
    assert!(w
        .feed(Input::RouteWritten {
            route: RouteId(1),
            result: Ok(1)
        })
        .is_empty());
}

/// DP-8: the descriptors that no route took are closed when their link ends or is replaced.
#[test]
fn a_link_end_closes_the_unbound_descriptors() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(3)));
    w.feed(Input::Descriptor(DescriptorId(4)));
    let actions = w.feed(Input::LinkClosed);
    let closed: Vec<&Action> = actions
        .iter()
        .filter(|a| matches!(a, Action::CloseDescriptor(_)))
        .collect();
    assert_eq!(
        closed,
        vec![
            &Action::CloseDescriptor(DescriptorId(3)),
            &Action::CloseDescriptor(DescriptorId(4))
        ]
    );
}

/// The size of the native snapshot that a route of `w` gets now.
fn snapshot_len(w: &World) -> u64 {
    w.worker
        .model
        .as_ref()
        .unwrap()
        .term
        .snapshot()
        .unwrap()
        .len() as u64
}

fn refused(client: &Client) -> bool {
    matches!(
        client.frames.as_slice(),
        [ToClient::RouteClosed(RouteClosedFrame {
            reason: CloseReason::AttachFailed {
                reason: AttachFailedReason::SnapshotTooLarge
            },
            ..
        })]
    )
}

/// OU-9, F75: each snapshot limit is inclusive. A snapshot of exactly `max_snapshot_bytes` is offered, and a screen frame
/// (the snapshot and its type byte) of exactly `max_screen_frame_bytes` is offered; one byte less refuses.
#[test]
fn the_snapshot_limits_take_a_snapshot_at_the_exact_bound() {
    let size = snapshot_len(&World::running());
    for (native, screen, refuse) in [
        (size, size + 1, false),
        (size - 1, size + 1, true),
        (size, size, true),
    ] {
        let mut w = with_limits(|l| l.max_snapshot_bytes = native);
        assert_eq!(snapshot_len(&w), size, "the same screen");
        w.feed(Input::Descriptor(DescriptorId(1)));
        let mut route = limits();
        route.max_screen_frame_bytes = screen;
        let mut client = Client::with(&route);
        let actions = attach(&mut w, RouteId(1), options(), route);
        client.take_all(&mut w, RouteId(1), actions);
        assert_eq!(
            refused(&client),
            refuse,
            "native {native}, screen {screen}, snapshot {size}"
        );
    }
}

/// OU-3a, OU-3d: a frame written in parts frees the queue by the bytes that each part took. When the last frame is taken, the
/// budget is again the budget of an empty queue.
#[test]
fn a_frame_written_in_parts_frees_exactly_its_bytes() {
    fn write(actions: &[Action]) -> Vec<u8> {
        match actions
            .iter()
            .find(|a| matches!(a, Action::RouteWrite { .. }))
        {
            Some(Action::RouteWrite { bytes, .. }) => bytes.clone(),
            _ => panic!("a write: {actions:?}"),
        }
    }
    let (mut w, _client, all) = attached(limits());
    let empty = budgets(&all).last().copied().flatten().expect("a limit");
    // Two frames: each is the length, the type byte and `FRAME - 1` bytes. `one` is the budget while one frame waits.
    let fed = w.feed(Input::PtyOutput(vec![b'x'; 999]));
    let first = write(&fed);
    assert_eq!(first.len(), 1004);
    let one = budgets(&fed).last().copied().flatten().expect("a limit");
    w.feed(Input::PtyOutput(vec![b'y'; 999]));
    let part = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(3),
    });
    assert_eq!(write(&part), first[3..].to_vec(), "the rest of the frame");
    let partial = budgets(&part).last().copied().flatten().expect("a limit");
    assert!(partial < empty, "{partial} < {empty}");
    let second = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(1001),
    });
    let next = write(&second);
    assert_eq!(next.len(), 1004, "the second frame");
    assert_eq!(
        budgets(&second).last(),
        Some(&Some(one)),
        "one frame waits again"
    );
    let done = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(1004),
    });
    assert_eq!(budgets(&done).last(), Some(&Some(empty)));
}

/// R-44: a failed handoff writes no frame and binds no output. The transport closes, and the host is told
/// `HandoffFailed` once.
fn assert_handoff_failed_with_no_frame(w: &mut World, actions: &[Action]) {
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, Action::RouteWrite { .. })),
        "{actions:?}"
    );
    assert!(actions.contains(&Action::RouteClose { route: RouteId(1) }));
    assert_eq!(
        closes(w, actions),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::HandoffFailed,
            route_tag: None
        }]
    );
    assert!(
        budgets(actions).iter().all(|b| b.is_none()),
        "no route takes output"
    );
}

/// The `route_closed` frame of a snapshot that cannot be offered (OU-2b).
fn route_closed_snapshot_too_large() -> ToClient {
    ToClient::RouteClosed(RouteClosedFrame {
        reason: CloseReason::AttachFailed {
            reason: AttachFailedReason::SnapshotTooLarge,
        },
        exit: None,
    })
}

/// DP-3, A19-1, R-44: a route whose `max_frame_bytes` cannot carry an attach frame gets no frame: the handoff failed.
#[test]
fn a_route_bound_below_the_attach_frames_fails_the_handoff_with_no_frame() {
    let mut small = limits();
    // Above `route_closed`, below `baseline_begin`.
    small.max_frame_bytes = 500;
    assert!(route_closed_snapshot_too_large().encode().len() <= 500);
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), small);
    assert_handoff_failed_with_no_frame(&mut w, &actions);
}

/// DP-3: when a snapshot cannot be offered and not even its `route_closed` fits the bound, nothing is written: the
/// transport closes at once, and the host is told `SnapshotTooLarge` once.
#[test]
fn a_route_bound_below_route_closed_closes_with_no_frame() {
    let mut w = with_limits(|l| l.max_snapshot_bytes = 8);
    let mut tiny = limits();
    tiny.max_frame_bytes = route_closed_snapshot_too_large().encode().len() as u64 - 1;
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), tiny);
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, Action::RouteWrite { .. })),
        "{actions:?}"
    );
    assert!(actions.contains(&Action::RouteClose { route: RouteId(1) }));
    assert_eq!(
        closes(&mut w, &actions),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None
        }]
    );
    assert!(
        budgets(&actions).iter().all(|b| b.is_none()),
        "no route takes output"
    );
}

/// DP-3: an attach frame of exactly the bound is sent. The bound is the largest attach frame (`baseline_begin`), measured.
#[test]
fn a_route_bound_equal_to_the_largest_attach_frame_attaches() {
    let (_, probe, _) = attached(limits());
    let largest = probe
        .frames
        .iter()
        .filter(|f| !matches!(f, ToClient::Screen { .. }))
        .map(|f| f.encode().len() as u64)
        .max()
        .unwrap();
    let mut exact = limits();
    exact.max_frame_bytes = largest;
    let (_, client, _) = attached(exact);
    assert!(
        matches!(client.frames.last(), Some(ToClient::Live)),
        "{:?}",
        client.frames.first()
    );
    exact.max_frame_bytes = largest - 1;
    let (_, client, _) = attached(exact);
    assert!(client.frames.is_empty(), "{:?}", client.frames);
}

/// OU-2: a write error while a healthy close delivers its `route_closed` keeps the first reason, and the host is told
/// once.
#[test]
fn a_write_error_during_a_healthy_close_keeps_the_first_reason() {
    let mut w = with_limits(|l| l.max_snapshot_bytes = 8);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), limits());
    assert!(actions
        .iter()
        .any(|a| matches!(a, Action::RouteWrite { .. })));
    let failed = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Err(32),
    });
    assert!(failed.contains(&Action::RouteClose { route: RouteId(1) }));
    assert_eq!(
        closes(&mut w, &failed),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None
        }]
    );
}

/// The PTY read budget of a route whose threshold is `queue` and whose frames behind the baseline take `behind` bytes: the
/// formula of `free_payload`, with output frames of `FRAME - 1` payload bytes and 5 bytes of overhead each.
fn budget_behind(queue: u64, behind: u64) -> usize {
    let free = usize::try_from(queue - behind).unwrap();
    let per = usize::try_from(FRAME).unwrap() - 1;
    free.saturating_sub((free / (per + 5) + 1) * 5)
}

/// OU-9, 9B, OU-3d, steward ruling R-45: with `route_queue_bytes` and `max_snapshot_bytes` both exactly the snapshot, the
/// snapshot is offered. The whole baseline and the held suffix go into the empty queue. The baseline sequence does not
/// count against the threshold: only the held suffix behind it does, and then the PTY output behind that. Every frame is
/// delivered in order, with no resync.
#[test]
fn a_queue_of_exactly_the_snapshot_takes_the_baseline_and_the_held_suffix() {
    let held_input = b"\x1b]2;t\x1b";
    let mut probe = World::running();
    probe.feed(Input::PtyOutput(held_input.to_vec()));
    let size = snapshot_len(&probe);
    let held = probe.worker.model.as_ref().unwrap().unfed().to_vec();
    assert!(!held.is_empty(), "the model holds the ESC");
    let mut w = with_limits(|l| {
        l.max_snapshot_bytes = size;
        l.route_queue_bytes = size;
    });
    w.feed(Input::PtyOutput(held_input.to_vec()));
    assert_eq!(snapshot_len(&w), size, "the same state");
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut bound = attach(&mut w, RouteId(1), options(), limits());
    // R-45 item 1: only the held suffix's `output` frame counts (its payload and 5 bytes of overhead).
    let held_frame = held.len() as u64 + 5;
    let first = budget_behind(size, held_frame);
    assert_eq!(budgets(&bound).last(), Some(&Some(first)));
    // The output behind the sequence counts: `first` bytes in frames of `FRAME - 1` payload bytes.
    let more = vec![b'x'; first];
    bound.extend(w.feed(Input::PtyOutput(more.clone())));
    let frames = (first as u64).div_ceil(FRAME - 1);
    assert_eq!(
        budgets(&bound).last(),
        Some(&Some(budget_behind(
            size,
            held_frame + first as u64 + frames * 5
        )))
    );
    let mut client = Client::default();
    let all = client.take_all(&mut w, RouteId(1), bound);
    let kinds: Vec<u8> = client.frames.iter().map(ToClient::type_byte).collect();
    use botster_route_codec::prelude::{
        TYPE_ATTACHED, TYPE_BASELINE_BEGIN, TYPE_BASELINE_END, TYPE_LIVE, TYPE_OUTPUT, TYPE_SCREEN,
    };
    let mut expected = vec![
        TYPE_ATTACHED,
        TYPE_BASELINE_BEGIN,
        TYPE_SCREEN,
        TYPE_BASELINE_END,
        TYPE_LIVE,
        TYPE_OUTPUT,
    ];
    expected.extend((0..frames).map(|_| TYPE_OUTPUT));
    assert_eq!(kinds, expected, "no resync");
    assert_eq!(
        client.output().0,
        [held.as_slice(), more.as_slice()].concat(),
        "the held suffix, then the output, after live"
    );
    let drained = budgets(&all).last().copied().flatten().expect("a limit");
    assert!(drained > 0, "an empty queue takes PTY bytes again");
}

/// The client's bytes of one frame, as the stream carries it (TS-3).
fn client_frame(frame: &ToWorker) -> Vec<u8> {
    botster_route_codec::prelude::stream_wrap(&frame.encode())
}

fn bytes_frame(op: u64, bytes: &[u8]) -> ToWorker {
    ToWorker::Bytes {
        op: Op(op),
        bytes: bytes.to_vec().into(),
    }
}

fn pty_writes(actions: &[Action]) -> Vec<u8> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::PtyWrite(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn route_closed(reason: CloseReason, exit: Option<WireExit>) -> ToClient {
    ToClient::RouteClosed(RouteClosedFrame { reason, exit })
}

/// DP-7, OU-2b: a `Detach` closes the route with its reason after every frame that is queued: `route_closed` is the last
/// frame, the transport closes once it is written, and the host is told once with the reason.
#[test]
fn a_detach_sends_route_closed_last_with_its_reason() {
    for (reason, close, wire) in [
        (
            DetachReason::Detached,
            RouteCloseReason::Detached,
            CloseReason::Detached,
        ),
        (
            DetachReason::Replaced,
            RouteCloseReason::Replaced,
            CloseReason::Replaced,
        ),
        (
            DetachReason::Revoked,
            RouteCloseReason::Revoked,
            CloseReason::Revoked,
        ),
    ] {
        let (mut w, mut client, _) = attached(limits());
        let mut actions = w.feed(Input::PtyOutput(b"tail".to_vec()));
        actions.extend(w.send(&HostMsg::Detach {
            route: RouteId(1),
            reason,
        }));
        let all = client.take_all(&mut w, RouteId(1), actions);
        assert_eq!(
            client.output().0,
            b"tail",
            "{reason:?}: the output before the close"
        );
        assert_eq!(client.frames.last(), Some(&route_closed(wire, None)));
        assert!(all.contains(&Action::RouteClose { route: RouteId(1) }));
        assert_eq!(
            closes(&mut w, &all),
            vec![WorkerMsg::RouteClosed {
                route: RouteId(1),
                reason: close,
                route_tag: None
            }]
        );
    }
}

/// DP-7: a `Detach` of a route that the worker does not hold is reported closed at once, so the host's `Detach` completes.
#[test]
fn a_detach_of_a_route_that_the_worker_does_not_hold_is_reported_closed() {
    let mut w = World::running();
    let actions = w.send(&HostMsg::Detach {
        route: RouteId(7),
        reason: DetachReason::Revoked,
    });
    assert!(!actions
        .iter()
        .any(|a| matches!(a, Action::RouteClose { .. })));
    assert_eq!(
        closes(&mut w, &actions),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(7),
            reason: RouteCloseReason::Revoked,
            route_tag: None
        }]
    );
}

/// OU-2: a second close of a closing route keeps the first reason, and the host is told once.
#[test]
fn a_detach_of_a_closing_route_keeps_the_first_reason() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Replaced,
    });
    actions.extend(w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Revoked,
    }));
    let all = client.take_all(&mut w, RouteId(1), actions);
    let closed: Vec<&ToClient> = client
        .frames
        .iter()
        .filter(|f| matches!(f, ToClient::RouteClosed(_)))
        .collect();
    assert_eq!(closed, vec![&route_closed(CloseReason::Replaced, None)]);
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::Replaced,
            route_tag: None
        }]
    );
}

/// OU-2, OU-7 (#217 R1-3): a detach whose close is still queued when the exit is reported keeps its reason. The session's
/// end adds no second close, and the host is told once, with the detach's reason.
#[test]
fn a_detach_that_is_closing_at_the_exit_keeps_its_reason() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Revoked,
    });
    actions.extend(w.feed(Input::PayloadExited(ExitStatus::Code(3))));
    actions.extend(w.feed(Input::PtyDrained));
    let all = client.take_all(&mut w, RouteId(1), actions);
    let closed: Vec<&ToClient> = client
        .frames
        .iter()
        .filter(|f| matches!(f, ToClient::RouteClosed(_)))
        .collect();
    assert_eq!(closed, vec![&route_closed(CloseReason::Revoked, None)]);
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::Revoked,
            route_tag: None
        }]
    );
}

/// OU-7: when the exit is reported, after the output tail, every route closes `session_ended` with the exit. The worker
/// reports each close when the route's queue is delivered (#217 R1-3): the host waits for it, and posts it with the exit
/// and cause that it decides (LC-5).
#[test]
fn the_exit_closes_every_route_session_ended_after_the_tail_and_reports_each_close_at_delivery() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    w.feed(Input::Descriptor(DescriptorId(2)));
    let mut one = Client::default();
    let mut two = Client::default();
    let a = attach(&mut w, RouteId(1), options(), limits());
    one.take_all(&mut w, RouteId(1), a);
    let b = attach(&mut w, RouteId(2), options(), limits());
    two.take_all(&mut w, RouteId(2), b);
    let mut actions = w.feed(Input::PtyOutput(b"bye".to_vec()));
    actions.extend(w.feed(Input::PayloadExited(ExitStatus::Code(3))));
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, Action::RouteClose { .. })),
        "no close before the drain reports the exit"
    );
    actions.extend(w.feed(Input::PtyDrained));
    let ended = route_closed(
        CloseReason::SessionEnded,
        Some(WireExit {
            code: Some(3),
            signal: None,
        }),
    );
    assert_eq!(
        closes(&mut w, &actions),
        vec![],
        "no close is reported before its queue is delivered"
    );
    let mut all = one.take_all(&mut w, RouteId(1), actions.clone());
    all.extend(two.take_all(&mut w, RouteId(2), actions));
    for client in [&one, &two] {
        assert_eq!(client.output().0, b"bye");
        assert_eq!(client.frames.last(), Some(&ended));
    }
    assert!(all.contains(&Action::RouteClose { route: RouteId(1) }));
    assert!(all.contains(&Action::RouteClose { route: RouteId(2) }));
    let reason = RouteCloseReason::SessionEnded {
        exit: Exit {
            code: Some(3),
            signal: None,
            cause: ExitCause::Other,
        },
    };
    let reported: Vec<(RouteId, RouteCloseReason)> = closes(&mut w, &all)
        .into_iter()
        .filter_map(|m| match m {
            WorkerMsg::RouteClosed { route, reason, .. } => Some((route, reason)),
            _ => None,
        })
        .collect();
    assert_eq!(reported, vec![(RouteId(1), reason), (RouteId(2), reason)]);
}

/// OU-7: a route that attaches after the exit was reported gets its baseline, then the session's close.
#[test]
fn a_route_that_attaches_after_the_exit_gets_its_baseline_then_session_ended() {
    let mut w = World::running();
    w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    w.feed(Input::PtyDrained);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut client = Client::default();
    let actions = attach(&mut w, RouteId(1), options(), limits());
    client.take_all(&mut w, RouteId(1), actions);
    assert!(client.frames.contains(&ToClient::Live));
    assert_eq!(
        client.frames.last(),
        Some(&route_closed(
            CloseReason::SessionEnded,
            Some(WireExit {
                code: None,
                signal: Some(9)
            })
        ))
    );
}

/// DP-5, IN-4, AM-2: after `live`, a client's `bytes` and `text` frames go to the PTY in their receive order. Each complete
/// frame advances `input_rev{client}` on receipt and is reported with its route; a frame split across reads waits for its
/// last byte.
#[test]
fn client_input_after_live_reaches_the_pty_in_order_and_is_tagged() {
    let (mut w, _, _) = attached(limits());
    let mut stream = client_frame(&bytes_frame(1, b"ab"));
    stream.extend(client_frame(&ToWorker::Text {
        op: Op(0),
        text: "cd".into(),
    }));
    let (first, rest) = stream.split_at(3);
    let mut all = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: first.to_vec(),
    });
    assert!(
        pty_writes(&all).is_empty(),
        "no write before the frame is whole"
    );
    all.extend(w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: rest.to_vec(),
    }));
    // The first transaction owns the PTY until its count (AM-2); then the second starts.
    all.extend(w.feed(Input::PtyWritten(Ok(2))));
    assert_eq!(pty_writes(&all), b"abcd");
    let tags: Vec<WorkerMsg> = w
        .reports(&all)
        .into_iter()
        .filter(|m| matches!(m, WorkerMsg::Observed { .. }))
        .collect();
    assert_eq!(
        tags,
        [1, 2]
            .map(|rev| WorkerMsg::Observed {
                observation: Observation::ClientInput {
                    route: RouteId(1),
                    input_rev: InputRev(rev),
                },
            })
            .to_vec()
    );
}

fn refusals(client: &Client) -> Vec<(Option<Op>, RefusalReason)> {
    client
        .frames
        .iter()
        .filter_map(|f| match f {
            ToClient::InputRefused(r) => Some((r.op, r.reason.clone())),
            _ => None,
        })
        .collect()
}

/// DP-5: input before `live` is written is refused `not_ready` with its op and reaches no PTY.
#[test]
fn input_before_live_is_written_is_refused_not_ready() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let bound = attach(&mut w, RouteId(1), options(), limits());
    // The baseline is queued, and no write of it is answered yet.
    let mut actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(5, b"x")),
    });
    assert!(pty_writes(&actions).is_empty());
    actions.splice(0..0, bound);
    let mut client = Client::default();
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals(&client),
        vec![(Some(Op(5)), RefusalReason::NotReady)]
    );
    let live = client
        .frames
        .iter()
        .position(|f| f == &ToClient::Live)
        .unwrap();
    assert!(
        matches!(client.frames[live + 1], ToClient::InputRefused(_)),
        "the refusal is behind the baseline"
    );
}

/// OU-1: a view-only route (`input: false`) refuses its input `not_writable`; op 0 is no op (DP-5).
#[test]
fn a_view_only_route_refuses_input_not_writable() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.input = false;
    let mut client = Client::default();
    let bound = attach(&mut w, RouteId(1), opts, limits());
    client.take_all(&mut w, RouteId(1), bound);
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(0, b"x")),
    });
    assert!(pty_writes(&actions).is_empty());
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(refusals(&client), vec![(None, RefusalReason::NotWritable)]);
}

/// DP-5, TS-5b (PR3 F102): a frame kind that this worker does not apply yet is refused `unsupported` with its kind and the
/// frame's own op, and the route stays open.
#[test]
fn an_unapplied_input_kind_is_refused_unsupported_with_its_op_and_the_route_stays_open() {
    use botster_route_codec::prelude::{stream_wrap, TYPE_INPUT};
    let (mut w, mut client, _) = attached(limits());
    let mut all = Vec::new();
    let key = [
        &[TYPE_INPUT][..],
        br#"{"kind":"key","op":"5","key":{"char":"a"},"event":"press"}"#,
    ]
    .concat();
    for bytes in [
        client_frame(&ToWorker::PasteChunk {
            op: Op(2),
            bytes: b"p".to_vec().into(),
        }),
        client_frame(&ToWorker::FileChunk {
            op: Op(3),
            bytes: b"f".to_vec().into(),
        }),
        stream_wrap(&key),
    ] {
        let actions = w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes,
        });
        all.extend(client.take_all(&mut w, RouteId(1), actions));
    }
    let unsupported = |op: Option<u64>, what: &str| {
        (op.map(Op), RefusalReason::Unsupported { what: what.into() })
    };
    assert_eq!(
        refusals(&client),
        vec![
            unsupported(Some(2), "paste_chunk"),
            unsupported(Some(3), "file_chunk"),
            unsupported(Some(5), "input"),
        ]
    );
    assert!(
        !all.contains(&Action::RouteClose { route: RouteId(1) }),
        "{:?}",
        client.frames
    );
}

/// IN-4, DP-5 (PR3 F87, integration R1-4): a complete input frame that is refused is still client input. Invalid UTF-8
/// text, an unknown enum value and an invalid field are each refused with their op, advance `input_rev{client}` once on
/// receipt, carry their route and reach no PTY. The route stays open, and a valid frame after them is applied.
#[test]
fn a_refused_complete_input_frame_advances_the_client_revision_once() {
    use botster_route_codec::prelude::{stream_wrap, TYPE_INPUT, TYPE_TEXT};
    let (mut w, mut client, _) = attached(limits());
    let mut text = vec![TYPE_TEXT];
    text.extend(7u64.to_be_bytes());
    text.push(0xff);
    let json = |body: &str| [&[TYPE_INPUT][..], body.as_bytes()].concat();
    let mut all = Vec::new();
    for raw in [
        text,
        json(r#"{"kind":"key","key":{"char":"a"},"event":"wiggle"}"#),
        json(r#"{"kind":"key","key":{"char":"ab"},"event":"press"}"#),
    ] {
        let actions = w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes: stream_wrap(&raw),
        });
        all.extend(client.take_all(&mut w, RouteId(1), actions));
    }
    assert_eq!(
        refusals(&client),
        vec![
            (
                Some(Op(7)),
                RefusalReason::Unsupported {
                    what: "utf8".into()
                }
            ),
            (
                None,
                RefusalReason::Unsupported {
                    what: "event".into()
                }
            ),
            (None, RefusalReason::InvalidInput),
        ]
    );
    assert!(
        pty_writes(&all).is_empty(),
        "a refused frame reaches no PTY"
    );
    assert!(!all.contains(&Action::RouteClose { route: RouteId(1) }));
    all.extend(w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(0, b"ok")),
    }));
    assert_eq!(pty_writes(&all), b"ok");
    let tags: Vec<WorkerMsg> = w
        .reports(&all)
        .into_iter()
        .filter(|m| matches!(m, WorkerMsg::Observed { .. }))
        .collect();
    assert_eq!(
        tags,
        [1, 2, 3, 4]
            .map(|rev| WorkerMsg::Observed {
                observation: Observation::ClientInput {
                    route: RouteId(1),
                    input_rev: InputRev(rev),
                },
            })
            .to_vec()
    );
}

/// OU-5: a frame that cannot be decoded closes only that route, `BadFrame` with its code: `route_closed{protocol_error}`
/// is the last frame, and the host is told once.
#[test]
fn a_bad_client_frame_closes_the_route_bad_frame_with_its_code() {
    let (mut w, mut client, _) = attached(limits());
    // A declared length over the route's frame bound.
    let mut bytes = (u32::try_from(FRAME).unwrap() + 1).to_be_bytes().to_vec();
    bytes.push(0x10);
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes,
    });
    let all = client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        client.frames.last(),
        Some(&route_closed(
            CloseReason::ProtocolError {
                code: ProtocolErrorCode::FrameTooLarge
            },
            None
        ))
    );
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::BadFrame {
                code: ProtocolErrorCode::FrameTooLarge
            },
            route_tag: None
        }]
    );
}

/// OU-2 (PR3 F86, integration R1-1): the client's end closes while a healthy close waits to write its `route_closed`. The
/// transport closes at once, once, and the host is told the first reason once, not `PeerClosed`.
#[test]
fn the_clients_close_during_a_healthy_close_keeps_the_first_reason() {
    let (mut w, _, _) = attached(limits());
    let mut all = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Replaced,
    });
    assert!(
        !all.iter().any(|a| matches!(a, Action::RouteClose { .. })),
        "the close waits for its frame: {all:?}"
    );
    all.extend(w.feed(Input::RouteEnded { route: RouteId(1) }));
    all.extend(w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(1),
    }));
    all.extend(w.feed(Input::RouteEnded { route: RouteId(1) }));
    let transport_closes = all
        .iter()
        .filter(|a| matches!(a, Action::RouteClose { .. }))
        .count();
    assert_eq!(transport_closes, 1, "{all:?}");
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::Replaced,
            route_tag: None
        }]
    );
}

/// OU-5: the client's close of its end closes that route `PeerClosed` with no frame, and the host is told once.
#[test]
fn the_clients_close_closes_the_route_peer_closed() {
    let (mut w, _, _) = attached(limits());
    let actions = w.feed(Input::RouteEnded { route: RouteId(1) });
    assert!(actions.contains(&Action::RouteClose { route: RouteId(1) }));
    assert!(!actions
        .iter()
        .any(|a| matches!(a, Action::RouteWrite { .. })));
    assert_eq!(
        closes(&mut w, &actions),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::PeerClosed,
            route_tag: None
        }]
    );
}

/// The last read allowance of `route` in `actions` (DP-5).
fn allowance(actions: &[Action], route: RouteId) -> Option<usize> {
    actions.iter().rev().find_map(|a| match a {
        Action::RouteReadAllowance { route: r, bytes } if *r == route => Some(*bytes),
        _ => None,
    })
}

/// The complete client input frames that the worker received in `actions` (IN-4: one `ClientInput` each).
fn received(w: &mut World, actions: &[Action]) -> usize {
    w.reports(actions)
        .into_iter()
        .filter(|m| {
            matches!(
                m,
                WorkerMsg::Observed {
                    observation: Observation::ClientInput { .. }
                }
            )
        })
        .count()
}

/// The refusals that the client read, with their written bytes.
fn refusals_written(client: &Client) -> Vec<(Option<Op>, RefusalReason, Option<u64>)> {
    client
        .frames
        .iter()
        .filter_map(|f| match f {
            ToClient::InputRefused(r) => Some((r.op, r.reason.clone(), r.written_bytes)),
            _ => None,
        })
        .collect()
}

/// DP-5 (PR3 F88, integration R1-2): while the PTY takes nothing, the route's input waits at the admission point, and the
/// route's read allowance is its input bound (`route_input_queue_bytes`) less the input that it holds: the queued and
/// in-progress input, and the bytes of a frame that is not whole yet. A frame split at the allowance's edge completes when
/// the PTY takes bytes again. No byte is lost, and the PTY gets every frame in order.
#[test]
fn route_input_is_held_within_its_allowance_while_the_pty_is_blocked() {
    let (mut w, _, all) = attached(limits());
    let bound = limits().route_input_queue_bytes as usize;
    assert_eq!(
        allowance(&all, RouteId(1)),
        Some(bound),
        "the bind's allowance"
    );
    let payload = |k: u8| vec![b'a' + k; 991];
    let mut writes = Vec::new();
    // Four frames of 991 payload bytes: the first is written, and its PTY write is not answered (the PTY is blocked).
    for k in 0..4 {
        let actions = w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes: client_frame(&bytes_frame(0, &payload(k))),
        });
        writes.extend(pty_writes(&actions));
        assert_eq!(
            allowance(&actions, RouteId(1)),
            Some(bound - 991 * (usize::from(k) + 1)),
            "frame {k}"
        );
    }
    assert_eq!(writes, payload(0), "one write is out");
    // The fifth frame's first 300 bytes: it is not whole, and they count.
    let fifth = client_frame(&bytes_frame(0, &payload(4)));
    let left = bound - 4 * 991;
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: fifth[..300].to_vec(),
    });
    // The driver spent 300 bytes of its allowance, so its rest is `left - 300`: the value now, so no new allowance goes out.
    assert_eq!(allowance(&actions, RouteId(1)), None);
    // The PTY takes the first frame: the allowance grows by its bytes, and the rest of the fifth frame fits.
    let actions = w.feed(Input::PtyWritten(Ok(991)));
    writes.extend(pty_writes(&actions));
    assert_eq!(allowance(&actions, RouteId(1)), Some(left - 300 + 991));
    assert!(fifth.len() - 300 <= left - 300 + 991);
    w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: fifth[300..].to_vec(),
    });
    for _ in 0..4 {
        writes.extend(pty_writes(&w.feed(Input::PtyWritten(Ok(991)))));
    }
    let expected: Vec<u8> = (0..5).flat_map(payload).collect();
    assert_eq!(writes, expected, "every frame, in order");
}

/// DP-5 (PR3 F88, F106): a client floods refused input and does not read. Every complete frame of the read is received on
/// the read (IN-4). The refusals wait in the route's queue under `route_queue_bytes`; when one more cannot fit, the worker
/// applies no more frames and the read allowance is 0. When the client reads, the frames that waited are refused in order,
/// and none is lost.
#[test]
fn refused_input_against_a_client_that_does_not_read_is_held_at_the_queue_bound() {
    let queue = 1000;
    let mut w = with_limits(|l| l.route_queue_bytes = queue);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.input = false;
    let mut client = Client::default();
    let bound = attach(&mut w, RouteId(1), opts, limits());
    client.take_all(&mut w, RouteId(1), bound);
    let flood: Vec<u8> = (1..=200)
        .flat_map(|k| client_frame(&bytes_frame(k, b"x")))
        .collect();
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: flood,
    });
    // Before the client reads: every frame is received, the refusals that the worker queued fit the queue, and the other
    // frames wait.
    assert_eq!(received(&mut w, &actions), 200, "each frame on its read");
    let (queued, waiting) = w.worker.route_pressure(RouteId(1));
    assert!(queued <= queue as usize, "{queued} bytes of refusals");
    assert!(waiting > 0, "the frames past the queue wait");
    assert_eq!(allowance(&actions, RouteId(1)), Some(0), "the reads stop");
    let all = client.take_all(&mut w, RouteId(1), actions);
    let ops: Vec<Option<Op>> = refusals(&client).into_iter().map(|(op, _)| op).collect();
    assert_eq!(ops, (1..=200).map(|k| Some(Op(k))).collect::<Vec<_>>());
    assert_eq!(
        allowance(&all, RouteId(1)),
        Some(limits().route_input_queue_bytes as usize),
        "nothing is held: the whole bound"
    );
}

/// DP-5, OU-2b (PR3 F88, integration R1-2): a closing route whose client does not read keeps none of the client's later
/// bytes: they are discarded, nothing is refused or written, and the route reads with no limit until its close.
#[test]
fn a_gated_closing_route_discards_the_clients_bytes() {
    let (mut w, _, _) = attached(limits());
    let mut actions = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Detached,
    });
    for k in 0..50 {
        actions.extend(w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes: client_frame(&bytes_frame(k, &[b'y'; 900])),
        }));
    }
    assert!(pty_writes(&actions).is_empty());
    assert_eq!(received(&mut w, &actions), 0, "no frame is decoded");
    assert_eq!(allowance(&actions, RouteId(1)), Some(usize::MAX));
    let mut client = Client::default();
    client.take_all(&mut w, RouteId(1), actions);
    assert!(refusals(&client).is_empty());
}

/// DP-5 (PR3 F90, integration R1-5): a client's `bytes` write that fails after part of it is written is refused `failed`
/// with its op and the bytes written; one that the payload's end cuts is refused `session_ended` with its bytes written.
#[test]
fn a_partly_written_route_input_is_refused_with_its_written_bytes() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(9, b"abcd")),
    });
    actions.extend(w.feed(Input::PtyWritten(Ok(2))));
    actions.extend(w.feed(Input::PtyWritten(Err(5))));
    actions.extend(w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&ToWorker::Text {
            op: Op(10),
            text: "efgh".into(),
        }),
    }));
    actions.extend(w.feed(Input::PtyWritten(Ok(3))));
    actions.extend(w.feed(Input::PayloadExited(ExitStatus::Code(0))));
    actions.extend(w.feed(Input::PtyWritten(Err(5))));
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals_written(&client),
        vec![
            (Some(Op(9)), RefusalReason::Failed, Some(2)),
            (Some(Op(10)), RefusalReason::SessionEnded, Some(3)),
        ]
    );
}

/// DP-5 (PR3 F90): a client's input that waits behind a host write when the payload ends is refused `session_ended` with
/// its op and no written bytes, and reaches no PTY.
#[test]
fn queued_route_input_at_the_payloads_end_is_refused_session_ended() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.send(&write(1, b"abc", None));
    actions.extend(w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(4, b"zz")),
    }));
    actions.extend(w.feed(Input::PayloadExited(ExitStatus::Code(0))));
    actions.extend(w.feed(Input::PtyWritten(Ok(3))));
    assert_eq!(pty_writes(&actions), b"abc", "only the host's write");
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals_written(&client),
        vec![(Some(Op(4)), RefusalReason::SessionEnded, None)]
    );
}

/// DP-8, AM-2 (PR3 F90): an adoption fences the old host's requests only. A client's input that waits behind the old
/// host's write keeps its place at the admission point and is written after that write, with no refusal.
#[test]
fn queued_route_input_survives_an_adoption() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.send(&write(1, b"abc", None));
    actions.extend(w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(4, b"zz")),
    }));
    w.send(&write(2, b"old", None));
    let adoption = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &adoption);
    actions.extend(adoption);
    actions.extend(w.feed(Input::PtyWritten(Ok(3))));
    actions.extend(w.feed(Input::PtyWritten(Ok(2))));
    assert_eq!(
        pty_writes(&actions),
        b"abczz",
        "the old host's queued write never runs; the route's input does"
    );
    client.take_all(&mut w, RouteId(1), actions);
    assert!(refusals(&client).is_empty());
}

/// DP-5 (PR3 F88): each route's read allowance counts only its own held input. Route 1's write is in progress, part of it
/// written, and one more frame waits; route 2's frame waits behind them. Route 1 holds the unwritten rest of its write and its
/// queued frame; route 2 holds only its own frame, and its input does not change route 1's allowance.
#[test]
fn each_routes_allowance_counts_only_its_own_input_and_the_unwritten_rest_of_its_write() {
    let (mut w, _, _) = attached(limits());
    let bound = limits().route_input_queue_bytes as usize;
    w.feed(Input::Descriptor(DescriptorId(2)));
    let mut second = Client::default();
    let bind = attach(&mut w, RouteId(2), options(), limits());
    let bind = second.take_all(&mut w, RouteId(2), bind);
    assert_eq!(allowance(&bind, RouteId(2)), Some(bound));
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(1, &[b'a'; 100])),
    });
    assert_eq!(pty_writes(&actions), [b'a'; 100], "route 1's write is out");
    assert_eq!(allowance(&actions, RouteId(1)), Some(bound - 100));
    // The PTY takes 40 bytes: route 1 holds the 60 that are not written.
    let actions = w.feed(Input::PtyWritten(Ok(40)));
    assert_eq!(allowance(&actions, RouteId(1)), Some(bound - 60));
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(2, &[b'b'; 30])),
    });
    assert_eq!(allowance(&actions, RouteId(1)), Some(bound - 60 - 30));
    let actions = w.feed(Input::RouteRead {
        route: RouteId(2),
        bytes: client_frame(&bytes_frame(3, &[b'c'; 50])),
    });
    assert_eq!(allowance(&actions, RouteId(2)), Some(bound - 50));
    assert_eq!(
        allowance(&actions, RouteId(1)),
        None,
        "route 2's input leaves route 1's allowance as it was"
    );
}

/// DP-5 (PR3 F90): a client's write that fails before any byte is written is refused `failed` with no written bytes.
#[test]
fn a_route_input_that_fails_before_any_byte_is_written_is_refused_with_no_written_bytes() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(7, b"abcd")),
    });
    actions.extend(w.feed(Input::PtyWritten(Err(5))));
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals_written(&client),
        vec![(Some(Op(7)), RefusalReason::Failed, None)]
    );
}

/// DP-5, R-45: the baseline sequence is exempt from `route_queue_bytes`, so a refusal needs room only behind it. With
/// `route_queue_bytes` equal to the baseline's size and a client that has read nothing, the route still decodes and
/// refuses a frame: the refusal is queued behind the baseline, and the reads go on.
#[test]
fn a_refusal_needs_room_only_behind_an_unread_baseline() {
    let mut opts = options();
    opts.input = false;
    // The baseline's size, from a client that reads all of it.
    let mut w = with_limits(|_| {});
    w.feed(Input::Descriptor(DescriptorId(1)));
    let bind = attach(&mut w, RouteId(1), opts.clone(), limits());
    let all = Client::default().take_all(&mut w, RouteId(1), bind);
    let baseline: usize = all
        .iter()
        .filter_map(|a| match a {
            Action::RouteWrite { route, bytes } if *route == RouteId(1) => Some(bytes.len()),
            _ => None,
        })
        .sum();
    let mut w = with_limits(|l| l.route_queue_bytes = baseline as u64);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let bind = attach(&mut w, RouteId(1), opts, limits());
    assert!(allowance(&bind, RouteId(1)).is_some_and(|n| n > 0));
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(5, b"x")),
    });
    assert_eq!(received(&mut w, &actions), 1, "the frame is decoded");
    assert!(allowance(&actions, RouteId(1)).is_none_or(|n| n > 0));
    let mut client = Client::default();
    client.take_all(&mut w, RouteId(1), [bind, actions].concat());
    assert_eq!(
        refusals(&client),
        vec![(Some(Op(5)), RefusalReason::NotWritable)]
    );
}

/// DP-5 (PR3 F100): each input of a route at the admission point keeps the room of one refusal. A host write blocks the PTY,
/// and a client that does not read sends one-byte inputs; when the payload ends, every queued input is refused at once.
/// Every frame is received on its read (IN-4, F106). The worker applied only the inputs whose refusals fit
/// `route_queue_bytes`, so those refusals fit it; the other frames wait, and every input is refused in order when the
/// client reads.
#[test]
fn deferred_refusals_of_queued_route_input_stay_within_the_route_queue() {
    let queue = 1000;
    let mut w = with_limits(|l| l.route_queue_bytes = queue);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut client = Client::default();
    let bind = attach(&mut w, RouteId(1), options(), limits());
    client.take_all(&mut w, RouteId(1), bind);
    let mut actions = w.send(&write(1, b"abc", None));
    let flood: Vec<u8> = (1..=50)
        .flat_map(|k| client_frame(&bytes_frame(k, b"x")))
        .collect();
    actions.extend(w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: flood,
    }));
    assert_eq!(received(&mut w, &actions), 50, "each frame on its read");
    assert!(
        w.worker.route_pressure(RouteId(1)).1 > 0,
        "the frames past the reserved room wait"
    );
    assert_eq!(allowance(&actions, RouteId(1)), Some(0), "the reads stop");
    actions.extend(w.feed(Input::PayloadExited(ExitStatus::Code(0))));
    actions.extend(w.feed(Input::PtyWritten(Ok(3))));
    assert_eq!(pty_writes(&actions), b"abc", "only the host's write");
    // The payload ended: every input that the worker applied is refused at once, and those refusals fit the queue.
    let (queued, _) = w.worker.route_pressure(RouteId(1));
    assert!(queued <= queue as usize, "{queued} bytes of refusals");
    let all = client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        refusals_written(&client),
        (1..=50)
            .map(|k| (Some(Op(k)), RefusalReason::SessionEnded, None))
            .collect::<Vec<_>>(),
        "no outcome is lost"
    );
    assert!(!all.contains(&Action::RouteClose { route: RouteId(1) }));
}

/// OU-2b, DP-5 (PR3 F101): `route_closed` is a closing route's last frame. A client's write is out on the PTY, the host
/// detaches the route while the client reads nothing, and the write then fails: no `input_refused` follows `route_closed`.
#[test]
fn a_route_input_that_fails_after_a_detach_sends_no_frame_after_route_closed() {
    let (mut w, mut client, _) = attached(limits());
    let mut actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: client_frame(&bytes_frame(6, b"abcd")),
    });
    assert_eq!(pty_writes(&actions), b"abcd", "the write is out");
    actions.extend(w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Detached,
    }));
    actions.extend(w.feed(Input::PtyWritten(Err(5))));
    client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(
        client.frames.last(),
        Some(&route_closed(CloseReason::Detached, None))
    );
    assert!(refusals(&client).is_empty(), "{:?}", client.frames);
}

/// DP-5 (PR3 F99): the driver spends an allowance read by read. Each read takes one whole frame, the worker refuses it at
/// once, and the client reads its refusals, so the route holds nothing after a read and the value that the worker computes
/// does not change; the worker still sends a new allowance when the driver's rest is lower. A driver that reads only within
/// its allowance moves more bytes than the first allowance, and every frame is refused. (A read that ends inside a frame
/// changes the value, so it would send a new allowance anyway: only whole-frame reads show the driver's rest.)
#[test]
fn a_driver_that_spends_its_allowance_on_refused_frames_gets_a_new_one() {
    let mut opts = options();
    opts.input = false;
    // A queue that holds every refusal: the worker decodes each read whole, and no full queue stops the reads.
    let mut w = with_limits(|l| l.route_queue_bytes = 1 << 20);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut client = Client::default();
    let bind = attach(&mut w, RouteId(1), opts, limits());
    let bind = client.take_all(&mut w, RouteId(1), bind);
    let first = allowance(&bind, RouteId(1)).expect("the bind's allowance");
    let sent: Vec<u8> = (1..=400)
        .flat_map(|k| client_frame(&bytes_frame(k, b"x")))
        .collect();
    assert!(
        sent.len() > first,
        "{} bytes, first allowance {first}",
        sent.len()
    );
    let frame = sent.len() / 400;
    let (mut rest, mut at) = (first, 0);
    while at < sent.len() {
        // One whole frame, when the rest allows it.
        let n = if rest >= frame { frame } else { 0 };
        assert!(
            n > 0,
            "the reads stopped at {at} of {} (rest {rest})",
            sent.len()
        );
        let actions = w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes: sent[at..at + n].to_vec(),
        });
        at += n;
        rest -= n;
        let all = client.take_all(&mut w, RouteId(1), actions);
        if let Some(new) = allowance(&all, RouteId(1)) {
            rest = new;
        }
    }
    let ops: Vec<Option<Op>> = refusals(&client).into_iter().map(|(op, _)| op).collect();
    assert_eq!(ops, (1..=400).map(|k| Some(Op(k))).collect::<Vec<_>>());
}

/// The applied limits of a route that asks for none, at the default `CoreLimits` (the host's attach, OU-1).
fn default_limits() -> AppliedRouteLimits {
    let core = CoreLimits::default();
    AppliedRouteLimits {
        max_frame_bytes: core.effective_max_route_frame_bytes(),
        max_screen_frame_bytes: core.max_snapshot_bytes + 1,
        max_history_page_bytes: core.max_history_page_bytes,
        max_paste_bytes: core.max_paste_bytes,
        max_query_bytes: core.max_query_bytes,
        max_query_reply_bytes: core.max_query_reply_bytes,
        max_file_bytes: core.max_file_bytes,
        route_input_queue_bytes: core.route_input_queue_bytes,
        ..limits()
    }
}

/// Feeds `frame` to the route within the driver's allowance, answering every PTY write in full, and returns every action.
fn feed_within_allowance(w: &mut World, frame: &[u8], mut rest: usize) -> Vec<Action> {
    let (mut all, mut at) = (Vec::new(), 0);
    while at < frame.len() {
        assert!(rest > 0, "the reads stopped at {at} of {}", frame.len());
        let n = rest.min(frame.len() - at).min(64 * 1024);
        let mut actions = w.feed(Input::RouteRead {
            route: RouteId(1),
            bytes: frame[at..at + n].to_vec(),
        });
        at += n;
        rest -= n;
        while let Some(Action::PtyWrite(bytes)) = actions
            .iter()
            .rev()
            .find(|a| matches!(a, Action::PtyWrite(_)))
            .cloned()
        {
            all.append(&mut actions);
            actions = w.feed(Input::PtyWritten(Ok(bytes.len())));
        }
        if let Some(new) = allowance(&actions, RouteId(1)) {
            rest = new;
        }
        all.append(&mut actions);
    }
    all
}

/// DP-3, DP-5, steward ruling R-53: at the default limits `max_frame_bytes` (`max_paste_bytes + 4096`) is larger than
/// `route_input_queue_bytes`. One `bytes` frame of exactly `max_frame_bytes` is read whole within the allowances and
/// written, never stalled: the allowance is at least one whole frame with its stream prefix.
#[test]
fn one_frame_of_exactly_max_frame_bytes_at_the_default_limits_is_written() {
    let limits = default_limits();
    assert!(limits.max_frame_bytes > limits.route_input_queue_bytes);
    let (mut w, _, bind) = attached(limits);
    let first = allowance(&bind, RouteId(1)).expect("the bind's allowance");
    assert_eq!(first as u64, limits.max_frame_bytes + 4, "R-53");
    // The type byte and the op: the payload fills the rest of the frame.
    let payload = vec![b'z'; usize::try_from(limits.max_frame_bytes).unwrap() - 9];
    let frame = client_frame(&bytes_frame(1, &payload));
    assert_eq!(frame.len() as u64, limits.max_frame_bytes + 4);
    let all = feed_within_allowance(&mut w, &frame, first);
    assert_eq!(pty_writes(&all), payload, "the whole frame reaches the PTY");
    assert!(!all.contains(&Action::RouteClose { route: RouteId(1) }));
}

/// DP-5, R-53 (PR3 F104): a legal `route_input_queue_bytes` below one frame. The route still reads one whole frame, and it
/// holds no more input than one frame with its prefix: while the frame's write is out, the allowance is the rest of that
/// bound, so a second whole frame waits in the stream until the write is done.
#[test]
fn a_route_input_queue_below_one_frame_still_completes_a_frame_and_holds_no_more() {
    let limits = AppliedRouteLimits {
        route_input_queue_bytes: 100,
        ..limits()
    };
    let (mut w, _, bind) = attached(limits);
    let bound = usize::try_from(FRAME).unwrap() + 4;
    assert_eq!(allowance(&bind, RouteId(1)), Some(bound), "R-53");
    let payload = vec![b'w'; usize::try_from(FRAME).unwrap() - 9];
    let frame = client_frame(&bytes_frame(1, &payload));
    assert_eq!(frame.len(), bound, "a frame of exactly max_frame_bytes");
    let actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: frame.clone(),
    });
    assert_eq!(
        pty_writes(&actions),
        payload,
        "the frame is decoded and its write is out"
    );
    let held = allowance(&actions, RouteId(1)).expect("a new allowance");
    assert_eq!(held, bound - payload.len(), "the unwritten payload is held");
    assert!(held < frame.len(), "a second frame does not fit");
    let actions = w.feed(Input::PtyWritten(Ok(payload.len())));
    assert_eq!(
        allowance(&actions, RouteId(1)),
        Some(bound),
        "the write is done"
    );
}

/// DP-5 (PR3 F105): a finite batch of input, all in one read within the allowance, on a route with a small queue. The
/// reserved refusal room lets the worker apply only part of it at once. Each PTY write succeeds, the payload is quiet,
/// and the client sends nothing more and reads nothing: the end of each input frees its room, and the frames that wait
/// are applied then. The whole batch reaches the PTY, in order, with no client read and no outgoing frame.
#[test]
fn a_quiet_batch_that_waits_for_reserved_room_reaches_the_pty_as_the_writes_end() {
    let mut w = with_limits(|l| l.route_queue_bytes = 1000);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut client = Client::default();
    let bind = attach(&mut w, RouteId(1), options(), limits());
    let first = allowance(&bind, RouteId(1)).expect("an allowance");
    client.take_all(&mut w, RouteId(1), bind);
    let batch: Vec<u8> = (0..50u8).map(|k| b'A' + k).collect();
    let read: Vec<u8> = batch
        .iter()
        .enumerate()
        .flat_map(|(k, &b)| client_frame(&bytes_frame(k as u64 + 1, &[b])))
        .collect();
    assert!(read.len() <= first, "{} of {first}: one read", read.len());
    let mut actions = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: read,
    });
    assert!(
        w.worker.route_pressure(RouteId(1)).1 > 0,
        "part of the batch waits for room"
    );
    let mut writes = Vec::new();
    loop {
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, Action::RouteWrite { .. })),
            "no outgoing frame"
        );
        let started = pty_writes(&actions);
        if started.is_empty() {
            break;
        }
        let n = started.len();
        writes.extend(started);
        actions = w.feed(Input::PtyWritten(Ok(n)));
    }
    assert_eq!(writes, batch, "the whole batch, in order");
    assert_eq!(w.worker.route_pressure(RouteId(1)), (0, 0));
}

/// IN-4, IN-10 (PR3 F106): a view-only route whose client floods input and reads nothing. Each complete frame of the read
/// is received on the read, whatever room the refusals have: the last client revision that the worker reports counts
/// them all. A host write guarded by an earlier reported revision is `Stale` with exact zero; a host write guarded by the
/// last one starts, and no later report counts a frame of that read.
#[test]
fn a_guard_at_the_reported_client_revision_counts_every_frame_of_the_read() {
    let queue = 1000;
    let mut w = with_limits(|l| l.route_queue_bytes = queue);
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.input = false;
    let mut client = Client::default();
    let bind = attach(&mut w, RouteId(1), opts, limits());
    let first = allowance(&bind, RouteId(1)).expect("an allowance");
    client.take_all(&mut w, RouteId(1), bind);
    let flood: Vec<u8> = (1..=50)
        .flat_map(|k| client_frame(&bytes_frame(k, b"x")))
        .collect();
    assert!(flood.len() <= first, "{} of {first}: one read", flood.len());
    let read = w.feed(Input::RouteRead {
        route: RouteId(1),
        bytes: flood,
    });
    assert!(
        w.worker.route_pressure(RouteId(1)).1 > 0,
        "refusals wait for room"
    );
    let revs: Vec<InputRev> = w
        .reports(&read)
        .into_iter()
        .filter_map(|m| match m {
            WorkerMsg::Observed {
                observation: Observation::ClientInput { input_rev, .. },
            } => Some(input_rev),
            _ => None,
        })
        .collect();
    let (&last, &partial) = (&revs[revs.len() - 1], &revs[revs.len() - 2]);
    assert_eq!(last, InputRev(50), "each frame of the read");
    let guard = |rev| Guard {
        input: Some(InputGuard {
            source_class: SourceClass::Client,
            rev,
        }),
        model_rev: None,
    };
    let stale = w.send(&write(1, b"zzz", Some(guard(partial))));
    assert!(
        pty_writes(&stale).is_empty(),
        "a partial revision writes nothing"
    );
    let result = input_result(&mut w, &stale, 1).expect("done");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::NotWritten(NotWrittenReason::Stale), 0, 0)
    );
    let actions = w.send(&write(2, b"abc", Some(guard(last))));
    assert_eq!(pty_writes(&actions), b"abc", "the guard passes");
    let later = client.take_all(&mut w, RouteId(1), [read, stale, actions].concat());
    // The actions hold the read's 50 reports, and no later report: no frame was left to count.
    assert_eq!(received(&mut w, &later), 50, "only the read's reports");
    assert_eq!(refusals(&client).len(), 50, "every frame is refused");
}
