//! The facade over a real host and a real worker link. Slow tier (BUILD.md testing rule 2): a real `Core`, a real control
//! socket and a real worker process. The worker process only hands its launch to the test; the test's thread then speaks
//! the worker side of the link (plan section 3) and answers what the host asks.
//!
//! Clause: Core 2 (the facade), ST-6 (captures: read, `release`, `release_owner`, the expiry deadline), DP-7 and A2-1
//! (`attach`), AD-6 (the token proof of the hello; `EndPayload` to the verified worker), LC-5 and AD-2 (a stop over a broken
//! link).
#![cfg(feature = "slow")]
// The test is the host: it reads the real clock and passes the time to `pump` (Core TM-1).
#![allow(clippy::disallowed_methods)]

mod common;

use botster_core::prelude::*;
use botster_core::Core;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::launch::WorkerLaunch;
use botster_core_link::msg::{HostMsg, PayloadId, WorkerMsg};
use botster_core_link::proof::token_proof;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

fn sid(name: &str) -> SessionId {
    SessionId(name.into())
}

fn send(stream: &mut UnixStream, kind: FrameType, payload: &[u8]) {
    let mut frame = Vec::new();
    encode_frame(kind, payload, 1 << 22, &mut frame).unwrap();
    stream.write_all(&frame).unwrap();
}

fn say(stream: &mut UnixStream, msg: &WorkerMsg) {
    let mut payload = Vec::new();
    msg.encode(&mut payload);
    send(stream, FrameType::WORKER_MSG, &payload);
}

/// The worker side of the link: the hello with the token proof, `Launched`, and a one-page capture for each
/// `CaptureSnapshot`. It ends when the link closes.
fn stand_in_worker(mut stream: UnixStream, launch: WorkerLaunch) {
    let hello = Hello {
        protocol: 1,
        instance: launch.instance.clone(),
        proof: token_proof(&launch.token, &launch.instance, launch.host_epoch),
        host_epoch: launch.host_epoch,
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    send(&mut stream, FrameType::HELLO, &payload);
    let mut decoder = FrameDecoder::new(1 << 22);
    let mut buf = [0u8; 4096];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let mut rest = &buf[..n];
        while !rest.is_empty() {
            let took = decoder.push(rest);
            rest = &rest[took..];
            while let Ok(Some(frame)) = decoder.next_frame() {
                match HostMsg::decode(&frame.payload) {
                    Ok(HostMsg::Launch(_)) => say(
                        &mut stream,
                        &WorkerMsg::Launched {
                            features: BTreeSet::new(),
                            terminal: TerminalState {
                                size: Size {
                                    rows: 24,
                                    cols: 80,
                                    cell_px: None,
                                },
                                modes: ModeFlags::default(),
                                title: None,
                                cwd: None,
                                last_output_at: None,
                                focused: Some(false),
                                model_rev: ModelRev(1),
                                input_rev: InputRevs {
                                    client: InputRev(0),
                                    host: InputRev(0),
                                },
                            },
                            formats: vec![],
                            // A payload that this test never signals: the host records the identity only.
                            payload: PayloadId {
                                pid: u32::MAX - 1,
                                start_time: 1,
                            },
                        },
                    ),
                    Ok(HostMsg::Op {
                        req,
                        op: Op::CaptureSnapshot { .. },
                    }) => {
                        say(
                            &mut stream,
                            &WorkerMsg::Pages {
                                req,
                                pages: vec![Page {
                                    index: 0,
                                    last: true,
                                    bytes: botster_route_codec::prelude::HexBytes(vec![1, 2, 3]),
                                }],
                            },
                        );
                        say(
                            &mut stream,
                            &WorkerMsg::Done {
                                req,
                                result: OpResult::Ok(OpOutput::Capture(Capture {
                                    capture: CaptureId(0),
                                    page_count: 1,
                                    total_bytes: 3,
                                    model_rev: ModelRev(1),
                                })),
                            },
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Pumps until `done` holds for the events so far. The host pumps only after a wake (TM-6): the worker's frames wake it.
fn pump_until(core: &mut Core, done: impl Fn(&[Event]) -> bool) -> Vec<Event> {
    let wake = core.wake_handle();
    let began = Instant::now();
    let mut events = Vec::new();
    loop {
        // timer: deadline — a host that never settles must fail the test, not spin
        assert!(
            began.elapsed() < Duration::from_secs(30),
            "the host never got there: {events:?}"
        );
        loop {
            let report = core.pump(Now {
                monotonic: Instant::now(),
                unix: 1_000_000,
            });
            events.extend(core.poll_events(64));
            if !report.more {
                break;
            }
        }
        if done(&events) {
            return events;
        }
        let woke = wake
            // timer: deadline — a failing run must not hang
            .wait(Duration::from_secs(30));
        assert_eq!(woke, Wake::Woken, "the host waited in vain: {events:?}");
    }
}

fn completed(events: &[Event], op: OpId) -> Option<OpResult> {
    events.iter().find_map(|e| match e {
        Event::Completed { op: o, result } if *o == op => Some(result.clone()),
        _ => None,
    })
}

fn capture(core: &mut Core, owner: &str) -> CaptureId {
    let op = core
        .begin(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId(owner.into()),
        })
        .unwrap();
    let events = pump_until(core, |e| completed(e, op).is_some());
    match completed(&events, op) {
        Some(OpResult::Ok(OpOutput::Capture(c))) => c.capture,
        other => panic!("{other:?}"),
    }
}

/// Core 2, ST-6, DP-7, A2-1, LC-5, AD-2: the facade's calls reach a real host with a worker on a real link. `release`
/// frees one capture and `release_owner` frees the captures of its owner (a read of either is then `UnknownCapture`);
/// `attach` registers a route synchronously, and the real edge, which hands no route over yet, closes it; a stop whose link
/// breaks ends the worker process through the real signal edge.
#[test]
fn the_facade_reaches_a_host_with_a_worker_on_a_real_link() {
    let tmp = tempfile::tempdir().unwrap();
    let launch = tmp.path().join("launch");
    common::mkfifo(&launch);
    // The worker process hands its launch (token, then arguments) to the test with external commands, then waits, without
    // the CPU, until a signal ends it or the test process is gone.
    let worker = common::ScriptWorker::new(
        tmp.path(),
        &format!(
            "/bin/echo \"$BOTSTER_WORKER_TOKEN\" \"$@\" | /usr/bin/tee '{}' >/dev/null\n{}",
            launch.display(),
            common::WAIT_WHILE_THE_PARENT_LIVES
        ),
    );
    let mut core = Core::open(OpenConfig {
        data_dir: tmp.path().join("d"),
        worker_path: Some(worker.path.clone()),
        limits: CoreLimits::default(),
    })
    .expect("open");
    let create = core
        .begin(Op::Create {
            session: sid("s1"),
            request: SpawnRequest {
                argv: vec!["/bin/true".into()],
                env: BTreeMap::new(),
                cwd: "/".into(),
                size: Size {
                    rows: 24,
                    cols: 80,
                    cell_px: None,
                },
                labels: BTreeMap::new(),
                color_profile: None,
                notification_policy: None,
                size_policy: None,
            },
        })
        .unwrap();
    pump_until(&mut core, |e| completed(e, create).is_some());
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    pump_until(&mut core, |_| true);
    // Opening the FIFO waits for the worker's `tee`; the read ends when `tee` closes it.
    let text = std::fs::read_to_string(&launch).unwrap();
    assert!(
        worker.pid().is_some(),
        "the registered worker recorded its PID"
    );
    let mut words = text.split_whitespace();
    let token = words.next().expect("the token");
    let args: Vec<&str> = words.collect();
    let parsed = WorkerLaunch::parse(&args, Some(token)).expect("a launch");
    let stream = UnixStream::connect(&parsed.control).expect("the host listens");
    let link = stream.try_clone().unwrap();
    let peer = std::thread::spawn(move || stand_in_worker(stream, parsed));
    let events = pump_until(&mut core, |e| completed(e, start).is_some());
    assert!(
        matches!(completed(&events, start), Some(OpResult::Ok(_))),
        "{events:?}"
    );
    assert_eq!(core.get(&sid("s1")).unwrap().state, SessionState::Running);

    // `attach`: registered at once; the real edge hands no route over yet (P4a), so the route closes.
    let (ours, _theirs) = UnixStream::pair().unwrap();
    let attached = core
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(ours)),
            AttachOptions {
                file_directory: tmp.path().display().to_string(),
                file_permissions: None,
                route_features: vec![],
                terminal_formats: vec![],
                connect_deadline: None,
                owner: None,
                query_deadline: Some(Duration::from_secs(1)),
                route_tag: None,
                route_limits: None,
                history: None,
                stall_deadline: None,
                answers_queries: true,
                input: true,
            },
        )
        .expect("the route is registered");
    let route = attached.route;
    pump_until(&mut core, |e| {
        e.iter()
            .any(|e| matches!(e, Event::RouteClosed { route: r, .. } if *r == route))
    });

    // `release` frees one capture.
    let first = capture(&mut core, "a");
    assert_eq!(core.read_page(first, 0).unwrap().bytes.0, vec![1, 2, 3]);
    core.release(first);
    assert_eq!(
        core.read_page(first, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
    // `release_owner` frees the captures of that owner only.
    let second = capture(&mut core, "b");
    let kept = capture(&mut core, "k");
    core.release_owner(&ClientId("b".into()));
    assert_eq!(
        core.read_page(second, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
    assert!(core.read_page(kept, 0).is_ok());
    // The open capture's expiry is a deadline of the host (ST-6, TM-3).
    assert!(core.next_deadline().is_some());

    // A stop whose control link breaks still ends (LC-5): the host asks the worker process itself, by its verified
    // identity, to end its payload (`EndPayload`, SIGUSR1, AD-6). This worker process has no handler, so the signal ends it,
    // and the host sees the worker's exit (AD-2: `Lost(WorkerGone)`). A host that sent no signal would wait for
    // `stop_grace` and end the session `Lost(WorkerUnreachable)`.
    let stop = core.begin(Op::Stop { id: sid("s1") }).unwrap();
    pump_until(&mut core, |_| true);
    link.shutdown(std::net::Shutdown::Both).unwrap();
    peer.join().unwrap();
    let events = pump_until(&mut core, |e| completed(e, stop).is_some());
    assert_eq!(
        core.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone),
        "{events:?}"
    );
}
