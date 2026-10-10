//! The stream routes (P4a PR1; DESIGN.md "P4a: the stream route"): the handoff, the baseline at the consumed cut, live output
//! within the frame bound, the PTY read budget, and the closes of this PR.

use super::*;
use botster_route_codec::prelude::{
    AttachFailedReason, CloseReason, FrameBounds, HistoryState, RouteClosed as RouteClosedFrame,
    StreamReader, ToClient,
};

/// The frame bound of the test routes: it carries the attach frames (`baseline_begin` is the largest, A9-1), and output
/// frames of `FRAME - 1` payload bytes.
const FRAME: u64 = 1000;

/// Limits that differ from every `CoreLimits` default, so a value the worker computed itself would show.
fn limits() -> AppliedRouteLimits {
    AppliedRouteLimits {
        max_frame_bytes: FRAME,
        max_screen_frame_bytes: CoreLimits::default().max_snapshot_bytes + 77,
        max_history_page_bytes: 4321,
        max_chunk_bytes: 333,
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
            l.max_history_page_bytes,
            l.max_chunk_bytes
        ),
        (
            FRAME,
            CoreLimits::default().max_snapshot_bytes + 77,
            4321,
            333
        )
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

/// OU-1: no format that the client lists can be emitted (the host refuses such an attach first): the route closes with
/// `attach_failed{bad_peer}` and no baseline.
#[test]
fn an_attach_with_no_common_format_closes_the_route_bad_peer() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.terminal_formats = vec!["no_such_format".into()];
    let actions = attach(&mut w, RouteId(1), opts, limits());
    let mut client = Client::default();
    let all = client.take_all(&mut w, RouteId(1), actions);
    assert!(matches!(
        client.frames.as_slice(),
        [ToClient::RouteClosed(RouteClosedFrame {
            reason: CloseReason::AttachFailed {
                reason: AttachFailedReason::BadPeer
            },
            ..
        })]
    ));
    assert!(matches!(
        closes(&mut w, &all).as_slice(),
        [WorkerMsg::RouteClosed {
            reason: RouteCloseReason::BadPeer,
            ..
        }]
    ));
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

fn route_closed_bad_peer() -> ToClient {
    ToClient::RouteClosed(RouteClosedFrame {
        reason: CloseReason::AttachFailed {
            reason: AttachFailedReason::BadPeer,
        },
        exit: None,
    })
}

/// DP-3, A9-1: a route whose `max_frame_bytes` cannot carry an attach frame gets no frame over its bound. When
/// `route_closed{attach_failed{bad_peer}}` fits, it is the only frame; the host is told `BadPeer` once.
#[test]
fn a_route_bound_below_the_attach_frames_closes_bad_peer_with_its_frame() {
    let closed = route_closed_bad_peer().encode().len() as u64;
    let mut small = limits();
    // Above `route_closed`, below `baseline_begin` (A9-1's floor is `attached`; Core A19 is drafted).
    small.max_frame_bytes = 500;
    assert!(closed <= 500);
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let actions = attach(&mut w, RouteId(1), options(), small);
    let mut client = Client::with(&small);
    let all = client.take_all(&mut w, RouteId(1), actions);
    assert_eq!(client.frames, vec![route_closed_bad_peer()]);
    assert!(all.contains(&Action::RouteClose { route: RouteId(1) }));
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::BadPeer,
            route_tag: None
        }]
    );
}

/// DP-3: when not even `route_closed` fits the bound, nothing is written: the transport closes at once, and the host is
/// told `BadPeer` once.
#[test]
fn a_route_bound_below_route_closed_closes_with_no_frame() {
    let mut tiny = limits();
    tiny.max_frame_bytes = route_closed_bad_peer().encode().len() as u64 - 1;
    let mut w = World::running();
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
            reason: RouteCloseReason::BadPeer,
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
    assert_eq!(client.frames, vec![route_closed_bad_peer()]);
}

/// OU-2: a write error while a healthy close delivers its `route_closed` keeps the first reason, and the host is told
/// once.
#[test]
fn a_write_error_during_a_healthy_close_keeps_the_first_reason() {
    let mut w = World::running();
    w.feed(Input::Descriptor(DescriptorId(1)));
    let mut opts = options();
    opts.terminal_formats = vec!["no_such_format".into()];
    let actions = attach(&mut w, RouteId(1), opts, limits());
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
            reason: RouteCloseReason::BadPeer,
            route_tag: None
        }]
    );
}

/// OU-9, 9B, OU-3d: with `route_queue_bytes` and `max_snapshot_bytes` both exactly the snapshot, the snapshot is offered
/// (the other baseline frames do not refuse it). The whole baseline and the held suffix go into the empty queue, the PTY
/// budget is 0 while the queue is over `route_queue_bytes`, and every frame is delivered in order.
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
    let bound = attach(&mut w, RouteId(1), options(), limits());
    assert_eq!(
        budgets(&bound).last(),
        Some(&Some(0)),
        "the queue is over route_queue_bytes: no PTY read"
    );
    let mut client = Client::default();
    let all = client.take_all(&mut w, RouteId(1), bound);
    let kinds: Vec<u8> = client.frames.iter().map(ToClient::type_byte).collect();
    use botster_route_codec::prelude::{
        TYPE_ATTACHED, TYPE_BASELINE_BEGIN, TYPE_BASELINE_END, TYPE_LIVE, TYPE_OUTPUT, TYPE_SCREEN,
    };
    assert_eq!(
        kinds,
        vec![
            TYPE_ATTACHED,
            TYPE_BASELINE_BEGIN,
            TYPE_SCREEN,
            TYPE_BASELINE_END,
            TYPE_LIVE,
            TYPE_OUTPUT
        ]
    );
    assert_eq!(client.output().0, held, "the held suffix after live");
    let drained = budgets(&all).last().copied().flatten().expect("a limit");
    assert!(drained > 0, "an empty queue takes PTY bytes again");
}
