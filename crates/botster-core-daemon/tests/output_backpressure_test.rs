#![allow(missing_docs)]
//! Output backpressure: a bound route whose reader makes progress holds the
//! session's output instead of overflowing and resyncing, so a flooding
//! program blocks on write as it would on a slow terminal. A reader with no
//! progress for the reader deadline is ended and stops governing.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    TerminalRouteCloseReason,
};
use botster_core::contract::terminal_wake::{TerminalWakeSink, WakingTerminalAdapter};
use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TerminalCapabilitySet,
    TerminalWakeKind,
};
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig, SpawnSessionRequest};
use botster_core_test_support::bounded_wait::{wait_for, HANG_GUARD};
use botster_core_test_support::fixture_gate::Fifo;
use botster_terminal_protocol::{
    decode_attach_state, AttachStateCode, RoutedTerminalFrame, TerminalFrame, TerminalKind,
};

/// A reader with one write slot. The test paces it: `complete` delivers the
/// active frame and raises the writable wake, as a transport does when its
/// peer has read.
#[derive(Clone, Default)]
struct PacedReader {
    state: Arc<Mutex<PacedState>>,
}

#[derive(Default)]
struct PacedState {
    active: Option<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
    sink: Option<TerminalWakeSink>,
    closed: Option<TerminalRouteCloseReason>,
}

impl PacedReader {
    fn lock(&self) -> std::sync::MutexGuard<'_, PacedState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Deliver the active frame, if any, and wake the route.
    fn complete(&self) {
        let mut state = self.lock();
        if let Some(frame) = state.active.take() {
            state.delivered.push(frame);
            if let Some(sink) = &state.sink {
                let _ = sink.wake(TerminalWakeKind::Writable);
            }
        }
    }

    fn has_active(&self) -> bool {
        self.lock().active.is_some()
    }

    fn closed(&self) -> Option<TerminalRouteCloseReason> {
        self.lock().closed
    }

    fn bodies(&self, kind: TerminalKind) -> Vec<Vec<u8>> {
        self.lock()
            .delivered
            .iter()
            .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
            .filter(|frame| frame.kind() == kind)
            .map(|frame| frame.body().to_vec())
            .collect()
    }

    fn output(&self) -> Vec<u8> {
        self.bodies(TerminalKind::Output).concat()
    }

    fn resyncs(&self) -> usize {
        self.bodies(TerminalKind::RouteResync).len()
    }

    fn attached(&self) -> bool {
        self.lock()
            .delivered
            .iter()
            .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
            .any(|frame| {
                frame.kind() == TerminalKind::AttachState
                    && decode_attach_state(&frame)
                        .is_ok_and(|code| code == AttachStateCode::Attached)
            })
    }
}

impl TerminalAdapter for PacedReader {
    fn try_write(&mut self, frame: &RoutedTerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        let mut state = self.lock();
        if state.closed.is_some() {
            return Err(TerminalAdapterWriteError::Closed);
        }
        if state.active.is_some() {
            return Err(TerminalAdapterWriteError::Full);
        }
        state.active = Some(frame.frame.as_bytes().to_vec());
        Ok(())
    }

    fn close(&mut self, reason: TerminalRouteCloseReason) {
        self.lock().closed.get_or_insert(reason);
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        let state = self.lock();
        if state.closed.is_some() {
            TerminalAdapterPressure::Closed
        } else if state.active.is_some() {
            TerminalAdapterPressure::Full
        } else {
            TerminalAdapterPressure::Ready
        }
    }

    fn try_read(&mut self) -> TerminalIngress {
        TerminalIngress::Empty
    }
}

impl WakingTerminalAdapter for PacedReader {
    fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
        self.lock().sink = Some(sink);
    }
}

/// Lines 1..=`last` exactly as `seq` writes them.
fn seq_output(last: u32) -> Vec<u8> {
    (1..=last)
        .flat_map(|line| format!("{line}\n").into_bytes())
        .collect()
}

/// Far above one route's egress bound, so a paced reader must hold the
/// session.
const LAST_LINE: u32 = 200_000;

struct Flood {
    daemon: CoreDaemon,
    session_id: SessionId,
    go: Fifo,
    data_dir: std::path::PathBuf,
}

impl Flood {
    /// Spawn `seq 1 LAST_LINE | cat`, gated on a FIFO so no line precedes
    /// the routes. Through the pipe the lines reach the PTY in full blocks,
    /// as a flooding program's writes do. `-onlcr` keeps the bytes exactly
    /// seq's (the PTY's CR insertion can repeat a CR at a blocked write), and
    /// the child then keeps the terminal open: a PTY discards output still
    /// queued when its last slave closes, which is outside this property.
    fn spawn(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!("{label}-{}-{nanos}", std::process::id()));
        let worker = botster_core_test_support::real_worker::WorkerBinary::from_env()
            .unwrap_or_else(|failure| panic!("{failure}"))
            .path;
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker));
        let go = Fifo::new(&format!("{label}-go"));
        let session_id = SessionId(format!("{label}-session"));
        daemon
            .spawn(
                SpawnSessionRequest {
                    request: SessionSpawnRequest {
                        request_id: RequestId(format!("{label}-spawn")),
                        session_id: session_id.clone(),
                        executable: "sh".into(),
                        arguments: vec![
                            "-c".into(),
                            format!(
                                "stty -echo -onlcr; /bin/cat '{}' >/dev/null; \
                                 seq 1 {LAST_LINE} | cat; exec cat >/dev/null",
                                go.path().display()
                            ),
                        ],
                        working_directory: SpawnWorkingDirectory { path: ".".into() },
                        environment: SpawnEnvironment::default(),
                        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                    },
                    metadata: CoreSessionMetadata::new(),
                },
                1,
            )
            .expect("spawn seq producer");
        Self {
            daemon,
            session_id,
            go,
            data_dir,
        }
    }

    /// Attach and bind a paced reader, pacing it through its capture.
    fn attach(&mut self, label: &str) -> (SubscriptionId, PacedReader) {
        let client_id = ClientId(format!("{label}-client"));
        let subscription_id = SubscriptionId(format!("{label}-sub"));
        self.daemon
            .expect_terminal_adapter(
                client_id.clone(),
                self.session_id.clone(),
                subscription_id.clone(),
            )
            .expect("declare");
        self.daemon
            .attach(
                client_id.clone(),
                self.session_id.clone(),
                subscription_id.clone(),
                2,
            )
            .expect("attach");
        let generation = self
            .daemon
            .terminal_subscription_generation(&self.session_id, &subscription_id)
            .expect("generation");
        let reader = PacedReader::default();
        self.daemon
            .bind_waking_terminal_adapter(
                client_id,
                self.session_id.clone(),
                subscription_id.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(reader.clone()),
            )
            .expect("bind paced reader");
        (subscription_id, reader)
    }

    /// Pump on wakes until `done`; after each pump, `pace` may complete reads.
    fn pump_until(
        &mut self,
        what: &str,
        mut pace: impl FnMut(),
        mut done: impl FnMut(&CoreDaemon) -> bool,
    ) {
        let daemon = &mut self.daemon;
        wait_for(what, HANG_GUARD, |remaining| {
            // timer: deadline — wait_for's remaining hang guard; expiry fails the test
            let batch = daemon.wait_wakes(remaining);
            daemon
                .pump_woken(&batch, 3)
                .unwrap_or_else(|error| panic!("pump during {what}: {error:?}"));
            pace();
            done(daemon).then_some(())
        });
    }

    /// Pace `readers` until each is attached and the capture has closed.
    fn settle(&mut self, readers: &[&PacedReader]) {
        let session_id = self.session_id.clone();
        self.pump_until(
            "the readers' attach",
            || readers.iter().for_each(|reader| reader.complete()),
            |daemon| {
                readers.iter().all(|reader| reader.attached())
                    && !daemon.capture_active(&session_id)
            },
        );
    }

    fn release(&self) {
        self.go.release(Duration::from_secs(5));
    }
}

impl Drop for Flood {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown(None, 9);
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

#[test]
fn a_progressing_reader_receives_every_byte_in_order_without_resync() {
    let expected = seq_output(LAST_LINE);
    let mut flood = Flood::spawn("backpressure-one");
    let (_, reader) = flood.attach("backpressure-one");
    flood.settle(&[&reader]);
    flood.release();
    let mut backpressured = false;
    let session_id = flood.session_id.clone();
    flood.pump_until(
        "every line through one paced reader",
        || reader.complete(),
        |daemon| {
            backpressured |= daemon.session_output_held(&session_id);
            reader.output().len() >= expected.len()
        },
    );
    assert!(
        backpressured,
        "the paced reader must have held the session at least once"
    );
    assert_eq!(reader.resyncs(), 0, "a progressing reader never resyncs");
    assert!(reader.output() == expected, "every byte arrives, in order");
    assert_eq!(reader.closed(), None);
}

#[test]
fn a_reader_that_stalls_and_resumes_before_the_deadline_does_not_resync() {
    let expected = seq_output(LAST_LINE);
    let mut flood = Flood::spawn("backpressure-resume");
    let (_, reader) = flood.attach("backpressure-resume");
    flood.settle(&[&reader]);
    flood.release();
    // Stall: read nothing until the full route holds the session.
    let session_id = flood.session_id.clone();
    flood.pump_until(
        "the stalled reader holds the session",
        || {},
        |daemon| daemon.session_output_held(&session_id) && reader.has_active(),
    );
    // Resume long before the reader deadline.
    flood.pump_until(
        "every line after the reader resumes",
        || reader.complete(),
        |_| reader.output().len() >= expected.len(),
    );
    assert_eq!(reader.resyncs(), 0, "a resumed reader never resyncs");
    assert!(reader.output() == expected, "every byte arrives, in order");
    assert_eq!(reader.closed(), None);
}

#[test]
fn a_reader_stopped_past_the_deadline_ends_and_no_longer_throttles_its_sibling() {
    let expected = seq_output(LAST_LINE);
    let mut flood = Flood::spawn("backpressure-slowest");
    let (_, progressing) = flood.attach("backpressure-slowest-a");
    let (_, stopped) = flood.attach("backpressure-slowest-b");
    flood.settle(&[&progressing, &stopped]);
    flood.release();
    // The stopped reader governs: the session holds while the progressing
    // one has drained everything it was given.
    let session_id = flood.session_id.clone();
    flood.pump_until(
        "the stopped reader holds the session",
        || progressing.complete(),
        |daemon| {
            daemon.session_output_held(&session_id)
                && stopped.has_active()
                && !progressing.has_active()
        },
    );
    assert!(
        progressing.output().len() < expected.len(),
        "the stopped reader throttled the session before its deadline"
    );
    // The host wait ends at the stopped reader's deadline, which ends it.
    flood.pump_until(
        "every line through the progressing reader",
        || progressing.complete(),
        |_| progressing.output().len() >= expected.len(),
    );
    assert_eq!(
        stopped.closed(),
        Some(TerminalRouteCloseReason::Stalled),
        "a reader with no progress for the deadline is ended"
    );
    assert_eq!(progressing.resyncs(), 0);
    assert!(
        progressing.output() == expected,
        "the progressing reader receives every byte, in order"
    );
    assert_eq!(progressing.closed(), None);
}
