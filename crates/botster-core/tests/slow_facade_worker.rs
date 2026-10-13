//! The facade over a real host and a real worker link. Slow tier (BUILD.md testing rule 2): a real `Core`, a real control
//! socket and a real worker process. The worker process only hands its launch to the test; the test's thread then speaks
//! the worker side of the link (plan section 3) and answers what the host asks.
//!
//! Clause: Core 2 (the facade), ST-6 (captures: read, `release`, `release_owner`, the expiry deadline), DP-7 and A2-1
//! (`attach`), AD-6 (the token proof of the hello; `EndPayload` to the verified worker), LC-5 and AD-2 (a stop over a broken
//! link).
#![cfg(feature = "slow")]
// The test is the host: it reads the real clock and passes the time to `pump` (Core TM-1).

mod common;

use botster_core::prelude::*;
use botster_core::Core;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::launch::WorkerLaunch;
use botster_core_link::msg::{AdoptReport, AdoptedPayload, HostMsg, PayloadId, WorkerMsg};
use botster_core_link::proof::token_proof;
use botster_core_sys::fd_pass;
use botster_test_process::{quoted, Blocker, Bounded, Deadline, Guard, OwnedChild};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

/// What the stand-in worker saw of one `AttachRoute` frame: its route, the descriptors of the read that held its start, and
/// the number of reads that the frame took.
#[derive(Debug, PartialEq, Eq)]
struct Handoff {
    route: RouteId,
    fds: usize,
    reads: usize,
}

/// The read size of the stand-in worker in the facade test: smaller than a frame header (`HEADER_LEN`, 5 bytes), so every
/// frame takes more than one read, and the `AttachRoute` frame ends in a later read than the one with its descriptor.
const SMALL_READ: usize = 3;

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
/// `CaptureSnapshot`. It reads at most `read_size` bytes at a time. For each `AttachRoute`, it calls `on_attach` with the
/// link, the route, the number of descriptors of the read that held the frame's start, the number of reads that the frame
/// took, and those descriptors (DP-2, plan 23ae: a read with descriptors holds the start of exactly one `AttachRoute`
/// frame, and the descriptors belong to that frame, also when the frame ends in a later read). It ends when the link
/// closes.
fn stand_in_worker(
    mut stream: UnixStream,
    launch: WorkerLaunch,
    read_size: usize,
    mut on_attach: impl FnMut(&mut UnixStream, Handoff, Vec<OwnedFd>),
) {
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
    let mut buf = vec![0u8; read_size];
    // The descriptor count of the read that holds the start of the frame in the decoder, and the reads of that frame.
    let mut started = (0, 0);
    // The descriptors of the last read that had some.
    let mut descriptors = Vec::new();
    loop {
        let read = match fd_pass::recv_with_fds(&stream, &mut buf) {
            Ok(read) if read.len > 0 => read,
            _ => return,
        };
        let count = read.fds.len();
        if count > 0 {
            descriptors = read.fds;
        }
        let mut rest = &buf[..read.len];
        while !rest.is_empty() {
            // An empty decoder means that a frame starts with this push (`push` takes no byte past a complete frame).
            if decoder.buffered() == 0 {
                started = (count, 0);
            }
            started.1 += 1;
            let took = decoder.push(rest);
            rest = &rest[took..];
            while let Ok(Some(frame)) = decoder.next_frame() {
                match HostMsg::decode(&frame.payload) {
                    Ok(HostMsg::AttachRoute { route, .. }) => on_attach(
                        &mut stream,
                        Handoff {
                            route,
                            fds: started.0,
                            reads: started.1,
                        },
                        std::mem::take(&mut descriptors),
                    ),
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
    let began = common::real_now();
    let mut events = Vec::new();
    loop {
        // timer: deadline — a host that never settles must fail the test, not spin
        assert!(
            began.elapsed() < Duration::from_secs(30),
            "the host never got there: {events:?}"
        );
        loop {
            let report = core.pump(Now {
                monotonic: common::real_now(),
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

/// A host with session `s1` `Running` over a real link: the worker process hands its launch to the test, and the test's
/// thread `peer` speaks the worker side (`stand_in_worker`, with `on_attach`). `link` is a second handle of that link.
struct Running {
    core: Core,
    _worker: common::ScriptWorker,
    link: UnixStream,
    peer: std::thread::JoinHandle<()>,
}

fn running(
    dir: &Path,
    on_attach: impl FnMut(&mut UnixStream, Handoff, Vec<OwnedFd>) + Send + 'static,
) -> Running {
    let launch = dir.join("launch");
    common::mkfifo(&launch);
    // The worker process hands its launch (token, then arguments) to the test with external commands, then waits, without
    // the CPU, until a signal ends it or the test process is gone.
    let worker = common::ScriptWorker::new(
        dir,
        &format!(
            "/bin/echo \"$BOTSTER_WORKER_TOKEN\" \"$@\" | /usr/bin/tee '{}' >/dev/null\n{}",
            launch.display(),
            common::WAIT_WHILE_THE_PARENT_LIVES
        ),
    );
    let mut core = Core::open(OpenConfig {
        data_dir: dir.join("d"),
        worker_path: Some(worker.path.clone()),
        limits: CoreLimits::default(),
    })
    .expect("open");
    let create = core
        .begin(Op::Create {
            session: sid("s1"),
            request: request(),
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
    let peer = std::thread::spawn(move || stand_in_worker(stream, parsed, SMALL_READ, on_attach));
    let events = pump_until(&mut core, |e| completed(e, start).is_some());
    assert!(
        matches!(completed(&events, start), Some(OpResult::Ok(_))),
        "{events:?}"
    );
    assert_eq!(core.get(&sid("s1")).unwrap().state, SessionState::Running);
    Running {
        core,
        _worker: worker,
        link,
        peer,
    }
}

fn route_options(dir: &Path) -> AttachOptions {
    AttachOptions {
        file_directory: dir.display().to_string(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        owner: None,
        query_deadline: Some(Duration::from_secs(1)),
        route_tag: None,
        route_limits: None,
        history: None,
        stall_deadline: None,
        answers_queries: true,
        input: true,
    }
}

/// Stops `s1` over a broken link (LC-5): the host signals the worker process itself, which has no handler, so the stop
/// ends `Lost(WorkerGone)` (AD-2, AD-6).
fn stop_over_a_broken_link(running: Running) {
    let Running {
        mut core,
        _worker,
        link,
        peer,
    } = running;
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

/// Core 2, ST-6, DP-7, A2-1, LC-5, AD-2: the facade's calls reach a real host with a worker on a real link. `release`
/// frees one capture and `release_owner` frees the captures of its owner (a read of either is then `UnknownCapture`);
/// `attach` registers a route synchronously, and the real edge hands its stream to the worker on its `AttachRoute` frame
/// (DP-2); a stop whose link breaks ends the worker process through the real signal edge.
#[test]
fn the_facade_reaches_a_host_with_a_worker_on_a_real_link() {
    let tmp = tempfile::tempdir().unwrap();
    let (handed, handoffs) = mpsc::channel();
    let mut running = running(tmp.path(), move |_, handoff, _| {
        let _ = handed.send(handoff);
    });
    let core = &mut running.core;

    // `attach`: registered at once; the real edge sends the route's stream with its `AttachRoute` frame (DP-2).
    let (ours, _theirs) = UnixStream::pair().unwrap();
    let attached = core
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(ours)),
            route_options(tmp.path()),
        )
        .expect("the route is registered");
    let route = attached.route;
    pump_until(core, |_| true);
    let handoff = handoffs
        // timer: deadline — a host that never sends the route must fail the test, not hang
        .recv_timeout(Duration::from_secs(30))
        .expect("the host sends the route");
    // The frame ends in a later read than its start, which held its one descriptor (plan 23ae).
    assert!(handoff.reads > 1, "{handoff:?}");
    assert_eq!(
        (handoff.route, handoff.fds),
        (route, 1),
        "the route's stream rides on its frame"
    );

    // `release` frees one capture.
    let first = capture(core, "a");
    assert_eq!(core.read_page(first, 0).unwrap().bytes.0, vec![1, 2, 3]);
    core.release(first);
    assert_eq!(
        core.read_page(first, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
    // `release_owner` frees the captures of that owner only.
    let second = capture(core, "b");
    let kept = capture(core, "k");
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
    stop_over_a_broken_link(running);
}

/// Core DP-2 (`conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`, real-only `slow:fd-handoff`), steward
/// ruling R-65: the host keeps its copy of a route's stream until the worker reports `RouteReceived`, then
/// closes it. The worker side closes its received copy first. A one-byte write of the client must then succeed, because the
/// host's copy keeps the peer open. After `RouteReceived`, the host has no copy: the client's read reports the close (EOF
/// on macOS; `ECONNRESET` on Linux, because the closed end held the client's unread byte) and a write fails with `EPIPE`. The worker answers a capture after `RouteReceived` on the same link, so the capture's `Completed` event
/// shows that the host handled `RouteReceived` (a real event, BUILD rule 5).
///
/// OS behavior that the test relies on: a write to a unix stream socket whose peer is closed on all descriptors fails with
/// `EPIPE` (macOS and Linux). Rust std starts every process with `SIGPIPE` ignored (bug catalog B6), so the write returns the
/// error instead of a signal ending the test.
#[test]
fn the_host_keeps_its_copy_of_a_route_stream_until_the_worker_reports_it_received() {
    let tmp = tempfile::tempdir().unwrap();
    let (closed, worker_closed) = mpsc::channel();
    let (go, gone) = mpsc::channel::<()>();
    let mut running = running(tmp.path(), move |link, handoff, descriptors| {
        let route = handoff.route;
        // The worker side closes its received copy, then waits for the test before it reports the route received.
        drop(descriptors);
        let _ = closed.send(handoff);
        if gone
            // timer: deadline — a test that never answers must not hang the worker side
            .recv_timeout(Duration::from_secs(30))
            .is_ok()
        {
            say(link, &WorkerMsg::RouteReceived { route });
        }
    });
    let core = &mut running.core;
    let (ours, mut client) = UnixStream::pair().unwrap();
    // Set while the peer is open: macOS refuses the option (`EINVAL`) on a socket whose peer is closed.
    // timer: deadline — a host copy that stays open must fail the test, not hang
    client
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let route = core
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(ours)),
            route_options(tmp.path()),
        )
        .expect("the route is registered")
        .route;
    pump_until(core, |_| true);
    let handoff = worker_closed
        // timer: deadline — a host that never sends the route must fail the test, not hang
        .recv_timeout(Duration::from_secs(30))
        .expect("the host sends the route");
    assert_eq!((handoff.route, handoff.fds), (route, 1));

    // Only the host's copy keeps the peer open now.
    client
        .write_all(b"x")
        .expect("the host keeps its copy until RouteReceived");

    go.send(()).unwrap();
    // The worker reads the capture's request after it sent `RouteReceived`.
    capture(core, "a");
    // The closed peer held an unread byte (the "x" above). Linux reports that close as `ECONNRESET` on the next read
    // (`af_unix` `unix_release_sock`); macOS reports EOF.
    let read = client.read(&mut [0u8; 1]).map_err(|e| e.kind());
    #[cfg(target_os = "linux")]
    let closed = Err(std::io::ErrorKind::ConnectionReset);
    #[cfg(not(target_os = "linux"))]
    let closed = Ok(0);
    assert_eq!(read, closed, "the host closed its copy");
    assert_eq!(
        client.write(b"y").unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );

    stop_over_a_broken_link(running);
}

/// Core DP-2 (`conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`, real-only `slow:fd-handoff`): from a
/// successful `attach` the route's stream belongs to Core, the worker gets its own copy, and the host never reads, writes or
/// relays a route byte. The client writes its bytes and ends its writing before the host hands the stream over. The worker
/// reads exactly those bytes, up to EOF, from its own copy, so the host read none of them. Then the worker writes its bytes, and the client reads exactly
/// those bytes first, so the host wrote none before them. After the stop, when no copy of the stream is open, the client
/// reads its end of file with no other byte, so the host wrote none after them either: also not in the window after the
/// worker sends `RouteReceived` and before the host handles it and closes its copy. The host's copy and its close are the
/// R-65 test above.
#[test]
fn the_worker_owns_the_route_stream_and_the_host_moves_no_byte_of_it() {
    const FROM_CLIENT: &[u8] = b"client bytes before the handoff";
    // One line, so that the client reads it with `first_line`.
    const FROM_WORKER: &str = "worker bytes\n";
    let tmp = tempfile::tempdir().unwrap();
    let (heard, worker_heard) = mpsc::channel();
    let mut running = running(tmp.path(), move |link, handoff, mut descriptors| {
        assert_eq!(descriptors.len(), 1, "{handoff:?}");
        let mut own = Bounded::new(UnixStream::from(descriptors.remove(0)));
        let read = own.to_eof(Deadline::cleanup()).map_err(|e| e.to_string());
        say(
            link,
            &WorkerMsg::RouteReceived {
                route: handoff.route,
            },
        );
        own.into_inner().write_all(FROM_WORKER.as_bytes()).unwrap();
        let _ = heard.send((handoff, read));
    });
    let core = &mut running.core;
    let (ours, mut client) = UnixStream::pair().unwrap();
    let route = core
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(ours)),
            route_options(tmp.path()),
        )
        .expect("the route is registered")
        .route;
    // The host holds the stream now, and has not handed it over: the host pumps only below. The client's end of writing
    // makes the worker's read end at EOF, after the client's last byte.
    client.write_all(FROM_CLIENT).unwrap();
    client.shutdown(std::net::Shutdown::Write).unwrap();
    pump_until(core, |_| true);
    let (handoff, read) = worker_heard
        // timer: deadline — a host that never sends the route must fail the test, not hang
        .recv_timeout(Duration::from_secs(30))
        .expect("the host sends the route");
    assert_eq!((handoff.route, handoff.fds), (route, 1));
    assert_eq!(
        read.as_deref(),
        Ok(FROM_CLIENT),
        "the worker's own copy has every byte of the client"
    );
    let (mut client, line) = botster_test_process::first_line(client);
    assert_eq!(
        line, FROM_WORKER,
        "the client's first bytes are the worker's"
    );

    stop_over_a_broken_link(running);
    // Every copy of the worker end is closed now: the worker's after its write, the host's at `RouteReceived` or with `Core`.
    assert_eq!(
        client
            .to_eof(Deadline::cleanup())
            .map_err(|e| e.to_string()),
        Ok(Vec::new()),
        "the client gets no byte after the worker's, so the host wrote none"
    );
}

/// A FIFO in `dir` that a fixture script writes and the test reads by deadlines. The test holds it open for reading and
/// writing, so the script's open for writing does not wait. Close-on-exec: no child inherits it.
fn marker_fifo(dir: &Path, name: &str) -> (Bounded<std::fs::File>, PathBuf) {
    let path = dir.join(name);
    let made = OwnedChild::spawn(Command::new("/usr/bin/mkfifo").arg(&path))
        .unwrap()
        .status();
    assert!(made.success());
    let file = std::fs::File::from(
        rustix::fs::open(
            &path,
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    (Bounded::new(file), path)
}

/// Reads one frame of `stream`.
fn read_frame(stream: &mut UnixStream) -> botster_core_link::frame::Frame {
    let mut decoder = FrameDecoder::new(1 << 22);
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "the host closed the link before a frame");
        let mut rest = &buf[..n];
        // `push` takes no byte past a complete frame, so the frame is taken before the rest is pushed.
        loop {
            let took = decoder.push(rest);
            rest = &rest[took..];
            if let Ok(Some(frame)) = decoder.next_frame() {
                return frame;
            }
            if rest.is_empty() {
                break;
            }
        }
    }
}

/// The worker side of an adoption on the connection that the new host made to the worker endpoint (DESIGN.md "Adoption
/// (P5)" part 3): the host sends its hello first; the worker proves its token for that host's epoch (AD-6) and reports a
/// running payload. The link is returned open.
fn adopted_worker(mut stream: UnixStream, launch: &WorkerLaunch) -> UnixStream {
    // timer: deadline — the limit of a wait for the host's hello; not a contract value.
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let host = read_frame(&mut stream);
    assert_eq!(host.kind, FrameType::HELLO);
    let host = Hello::decode(&host.payload).expect("the host's hello");
    let hello = Hello {
        protocol: botster_worker_core::WORKER_PROTOCOL,
        instance: launch.instance.clone(),
        proof: token_proof(&launch.token, &launch.instance, host.host_epoch),
        host_epoch: host.host_epoch,
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    send(&mut stream, FrameType::HELLO, &payload);
    say(
        &mut stream,
        &WorkerMsg::Adopted {
            report: Box::new(AdoptReport {
                payload: AdoptedPayload::Running {
                    payload: PayloadId {
                        pid: u32::MAX - 1,
                        start_time: 1,
                    },
                },
                routes: 0,
                features: BTreeSet::new(),
                terminal: None,
                formats: vec![],
            }),
        },
    );
    stream
}

fn request() -> SpawnRequest {
    SpawnRequest {
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
    }
}

/// DESIGN.md "Adoption (P5)" parts 1 and 3, AD-6, LC-12, LC-7: the real edges keep the worker endpoints in a private
/// directory of the data directory. A new host adopts a live worker by a connect to its endpoint (`connect_worker`): the
/// session is `Running` again. `Remove` deletes the endpoint that a session left (`remove_endpoint`). Without the connect,
/// the adoption is `Lost(WorkerUnreachable)`; without the removal, the endpoint stays.
#[test]
fn a_new_host_adopts_a_live_worker_at_its_endpoint_and_remove_deletes_an_endpoint() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    // Plan 6.1 (shared test code): the worker is a script under the guard of botster-test-process. It hands its launch
    // (token, then arguments) to the test through a FIFO that the test reads by a deadline, then blocks on a fixture child
    // until the guard ends its group.
    let (mut launch, launch_path) = marker_fifo(tmp.path(), "launch");
    let hold = Blocker::new(tmp.path(), "hold").unwrap();
    let mut guard = Guard::new(tmp.path()).unwrap();
    let worker = tmp.path().join("worker");
    let body = format!(
        "/bin/echo \"$BOTSTER_WORKER_TOKEN\" \"$@\" > {}; exec {}",
        quoted(&launch_path),
        hold.shell()
    );
    guard
        .wrapper(&worker, Path::new("/bin/sh"), &["-c", &body, "worker"])
        .unwrap();
    let data_dir = tmp.path().join("d");
    let open = || {
        Core::open(OpenConfig {
            data_dir: data_dir.clone(),
            worker_path: Some(worker.clone()),
            limits: CoreLimits::default(),
        })
        .expect("open")
    };
    let mut core = open();
    let endpoints = data_dir.join("w");
    let mode = std::fs::symlink_metadata(&endpoints)
        .expect("the endpoint directory")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700, "AD-6: only the host's user reaches it");

    let create = core
        .begin(Op::Create {
            session: sid("s1"),
            request: request(),
        })
        .unwrap();
    pump_until(&mut core, |e| completed(e, create).is_some());
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    pump_until(&mut core, |_| true);
    let text = launch
        .line(Deadline::cleanup())
        .expect("the launch line")
        .expect("the worker wrote its launch");
    guard
        .anchors(1, Deadline::cleanup())
        .expect("the guard holds the worker's group");
    let mut words = text.split_whitespace();
    let token = words.next().expect("the token");
    let args: Vec<&str> = words.collect();
    let parsed = WorkerLaunch::parse(&args, Some(token)).expect("a launch");
    assert_eq!(parsed.endpoint, endpoints.join(&parsed.instance.0));
    // The worker binds its endpoint before its first hello.
    let listener = UnixListener::bind(&parsed.endpoint).expect("the worker binds its endpoint");
    let stream = UnixStream::connect(&parsed.control).expect("the host listens");
    // The stand-in reports its end on a channel, which the test reads by a deadline.
    let (ended, peer) = std::sync::mpsc::channel();
    {
        let parsed = parsed.clone();
        std::thread::spawn(move || {
            stand_in_worker(stream, parsed, 4096, |_, _, _| {});
            let _ = ended.send(());
        });
    }
    let events = pump_until(&mut core, |e| completed(e, start).is_some());
    assert_eq!(
        core.get(&sid("s1")).unwrap().state,
        SessionState::Running,
        "{events:?}"
    );

    // LC-12: a dropped host leaves its worker running; the worker's link to it ends.
    drop(core);
    // timer: deadline — bounds the stand-in's end after the host drops its link.
    peer.recv_timeout(Deadline::cleanup().remaining())
        .expect("the stand-in ends with its link");
    let mut again = open();
    let adopt = again.begin(Op::AdoptAll).unwrap();
    // One pump: the new host connects to the endpoint and sends its hello. The connection waits in the listener's queue.
    pump_until(&mut again, |_| true);
    listener.set_nonblocking(true).unwrap();
    // The accept never waits: the listener is non-blocking. A host that did not connect fails here at once.
    let (stream, _) = listener
        .accept()
        .expect("the new host connected to the endpoint");
    stream.set_nonblocking(false).unwrap();
    let _link = adopted_worker(stream, &parsed);
    let events = pump_until(&mut again, |e| completed(e, adopt).is_some());
    assert!(
        matches!(completed(&events, adopt), Some(OpResult::Ok(_))),
        "{events:?}"
    );
    assert_eq!(
        again.get(&sid("s1")).unwrap().state,
        SessionState::Running,
        "{events:?}"
    );

    // LC-7 step 4: `Remove` deletes the session's endpoint. A `Created` session has no worker; its endpoint file stands
    // for one that a killed worker left.
    let create = again
        .begin(Op::Create {
            session: sid("s2"),
            request: request(),
        })
        .unwrap();
    let events = pump_until(&mut again, |e| completed(e, create).is_some());
    let instance = events
        .iter()
        .find_map(|e| match e {
            Event::SessionState { id, instance, .. } if *id == sid("s2") => Some(instance.clone()),
            _ => None,
        })
        .expect("the created session's instance");
    let left = endpoints.join(&instance.0);
    std::fs::write(&left, b"").unwrap();
    let remove = again.begin(Op::Remove { id: sid("s2") }).unwrap();
    let events = pump_until(&mut again, |e| completed(e, remove).is_some());
    assert!(
        matches!(completed(&events, remove), Some(OpResult::Ok(_))),
        "{events:?}"
    );
    assert!(!left.exists(), "the endpoint is removed");
}

/// Steward ruling R-78 item 4 (design test 13) and its follow-up, Core AD-2: a worker process that a kill ends while its
/// session is `Stopping` never leaves the session `Lost(WorkerUnreachable)`. Core's reaper reaps it and reports the exit, and
/// the host drains the worker's link without waiting before it classifies: a session whose worker wrote `Exited` before it
/// ended is `Exited` with that code; one whose worker wrote nothing is `Lost(WorkerGone)`. Here the test's thread holds the
/// worker side of the link and keeps it open until the stop completed, so no end of file comes: the drain ends at the first
/// would-block.
#[test]
fn a_worker_lost_while_stopping_ends_worker_gone() {
    for wrote_exited in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let Running {
            mut core,
            _worker,
            mut link,
            peer,
        } = running(tmp.path(), |_, _, _| {});
        let stop = core.begin(Op::Stop { id: sid("s1") }).unwrap();
        pump_until(&mut core, |_| true);
        assert_eq!(core.get(&sid("s1")).unwrap().state, SessionState::Stopping);
        // The worker's last frame is written before it ends: a process writes nothing after its exit.
        if wrote_exited {
            say(
                &mut link,
                &WorkerMsg::Exited {
                    code: Some(7),
                    signal: None,
                },
            );
        }
        let pid = _worker.pid().expect("the worker recorded its pid");
        botster_core_sys::signal::signal_process(
            pid.as_raw_nonzero().get().unsigned_abs(),
            rustix::process::Signal::KILL,
        )
        .unwrap();
        let ended = botster_test_process::platform::await_end(pid, Deadline::cleanup()).unwrap();
        assert_ne!(
            ended,
            botster_test_process::platform::Waited::Deadline,
            "the worker did not end"
        );
        // The link stays open on the test's side: the stop completes without an end of file.
        let events = pump_until(&mut core, |e| completed(e, stop).is_some());
        let result = completed(&events, stop).unwrap();
        if wrote_exited {
            assert!(
                matches!(&result, OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) if exit.code == Some(7)),
                "{result:?}"
            );
        } else {
            // Only the exit report ends a gone worker with no word, so Core's reaper reaped it.
            assert_eq!(
                result,
                OpResult::Ok(OpOutput::End(SessionEnd::Lost(LostReason::WorkerGone)))
            );
            assert!(
                rustix::process::test_kill_process(pid).is_err(),
                "Core's reaper reaped the worker"
            );
        }
        // The host closed its end after the drain; macOS then reports the test's socket as not connected.
        match link.shutdown(std::net::Shutdown::Both) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotConnected => {}
            Err(e) => panic!("the test's end of the link: {e}"),
        }
        peer.join().unwrap();
    }
}

// This test binary was built in the worktree where it runs (bug catalog B13).
botster_test_process::built_in_test!();
