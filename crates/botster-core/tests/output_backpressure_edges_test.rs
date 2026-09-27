#![allow(missing_docs)]
//! Output backpressure edges with a scripted runtime: a process exit and a
//! local MODES frame never overflow a full bound route.
//!
//! The pump is driven by explicit wake batches in a loop bounded by an
//! iteration count, so no test step waits on a clock.

use std::sync::{Arc, Mutex};

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    TerminalRouteCloseReason,
};
use botster_core::contract::terminal_wake::{TerminalWakeSink, WakingTerminalAdapter};
use botster_core::engine::terminal_screen::PlainTerminalScreenRuntime;
use botster_core::{
    ClientId, CoreSessionMetadata, ManagedSessionRuntime, ModeFlags, ProcessExitedPayload,
    RequestId, ResizePayload, SessionId, SessionSpawnRequest, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, TerminalCapabilitySet, TerminalWakeBatch,
    TerminalWakeKind, TerminalWakeRoute, TransportIngress,
};
use botster_core::{
    TerminalBackendError, TerminalOutputChunk, TerminalScreenRuntime, TerminalScreenSize,
    TerminalScreenState, TerminalSnapshotPayload,
};
use botster_core_test_support::fake::FakeSessionRuntime;
use botster_terminal_protocol::{RoutedTerminalFrame, TerminalFrame, TerminalKind};

/// A reader with one write slot, paced by the test: `complete` delivers the
/// active frame and raises the writable wake.
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
    ingress: std::collections::VecDeque<Vec<u8>>,
}

impl PacedReader {
    fn lock(&self) -> std::sync::MutexGuard<'_, PacedState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn complete(&self) {
        let mut state = self.lock();
        if let Some(frame) = state.active.take() {
            state.delivered.push(frame);
            if let Some(sink) = &state.sink {
                let _ = sink.wake(TerminalWakeKind::Writable);
            }
        }
    }

    /// Queue one client input frame for Core's next read of this route.
    fn inject(&self, frame: Vec<u8>) {
        self.lock().ingress.push_back(frame);
    }

    fn frames(&self) -> Vec<TerminalFrame> {
        self.lock()
            .delivered
            .iter()
            .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
            .collect()
    }

    fn output(&self) -> Vec<u8> {
        self.frames()
            .iter()
            .filter(|frame| frame.kind() == TerminalKind::Output)
            .flat_map(|frame| frame.body().to_vec())
            .collect()
    }

    fn count(&self, kind: TerminalKind) -> usize {
        self.frames()
            .iter()
            .filter(|frame| frame.kind() == kind)
            .count()
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
        self.lock()
            .ingress
            .pop_front()
            .map_or(TerminalIngress::Empty, TerminalIngress::Frame)
    }
}

impl WakingTerminalAdapter for PacedReader {
    fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
        self.lock().sink = Some(sink);
    }
}

/// A plain screen whose bracketed-paste mode follows `ESC[?2004h`, so the
/// in-process terminal model publishes MODES ahead of the chunk's OUTPUT.
struct ModalScreen {
    inner: PlainTerminalScreenRuntime,
    bracketed_paste: bool,
}

impl TerminalScreenRuntime for ModalScreen {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        if bytes.windows(8).any(|window| window == b"\x1b[?2004h") {
            self.bracketed_paste = true;
        }
        self.inner.write_output(bytes)
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.inner.resize(size);
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        self.inner.capture_snapshot()
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.inner.replay_snapshot(payload);
    }

    fn screen_state(&self) -> TerminalScreenState {
        self.inner.screen_state()
    }

    fn mode_flags(&self) -> Result<ModeFlags, TerminalBackendError> {
        Ok(ModeFlags {
            bracketed_paste: self.bracketed_paste,
            ..ModeFlags::default()
        })
    }
}

fn session_id() -> SessionId {
    SessionId("edge-session".into())
}

fn spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: RequestId("edge-spawn".into()),
        session_id: session_id(),
        executable: "fake-shell".into(),
        arguments: Vec::new(),
        working_directory: SpawnWorkingDirectory {
            path: "/workspace".into(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

/// Subscribe and bind a paced reader, and complete its capture.
fn bind<T: TerminalScreenRuntime + 'static>(
    runtime: &mut ManagedSessionRuntime<FakeSessionRuntime, T>,
) -> (SubscriptionId, PacedReader) {
    let client_id = ClientId("edge-client".into());
    let subscription_id = SubscriptionId("edge-sub".into());
    runtime.expect_terminal_adapter(client_id.clone(), session_id(), subscription_id.clone());
    runtime
        .handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id: client_id.clone(),
                session_id: session_id(),
                subscription_id: subscription_id.clone(),
            },
            1,
        )
        .expect("subscribe");
    let generation = runtime
        .terminal_subscription_generation(&session_id(), &subscription_id)
        .expect("generation");
    let reader = PacedReader::default();
    runtime
        .bind_waking_terminal_adapter(
            client_id,
            session_id(),
            subscription_id.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(reader.clone()),
        )
        .expect("bind");
    runtime.test_complete_route_capture(&session_id(), &subscription_id);
    (subscription_id, reader)
}

/// Pump the session and its route, then let the reader take one frame,
/// until `done` or the iteration bound; returns whether `done` held and
/// whether the session ever held output.
fn pace_until<T: TerminalScreenRuntime + 'static>(
    runtime: &mut ManagedSessionRuntime<FakeSessionRuntime, T>,
    subscription_id: &SubscriptionId,
    reader: &PacedReader,
    mut done: impl FnMut() -> bool,
) -> (bool, bool) {
    let batch = TerminalWakeBatch {
        adapter_routes: vec![TerminalWakeRoute {
            session_id: session_id(),
            subscription_id: subscription_id.clone(),
        }],
        ingress_sessions: vec![session_id()],
    };
    let mut held = false;
    for _ in 0..10_000 {
        runtime.pump_woken(&batch, 2).expect("pump");
        held |= runtime.session_output_held(&session_id());
        reader.complete();
        if done() {
            return (true, held);
        }
    }
    (false, held)
}

fn chunk(index: usize) -> Vec<u8> {
    format!("chunk-{index:04}:{}\n", "x".repeat(990)).into_bytes()
}

#[test]
fn a_process_exit_waits_for_room_behind_a_full_route_and_loses_no_output() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn");
    let (subscription_id, reader) = bind(&mut runtime);
    let expected: Vec<u8> = (0..100).flat_map(chunk).collect();
    for index in 0..100 {
        runtime
            .session_runtime_mut()
            .emit_output(session_id(), chunk(index));
    }
    runtime.session_runtime_mut().emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );
    let (done, held) = pace_until(&mut runtime, &subscription_id, &reader, || {
        reader.count(TerminalKind::ProcessExit) > 0
    });
    assert!(done, "the exit reaches the reader");
    assert!(held, "100 chunks exceed the route bound: the session held");
    assert_eq!(reader.count(TerminalKind::RouteResync), 0);
    assert!(
        reader.output() == expected,
        "every chunk arrives, in order, before the exit"
    );
    let last = reader.frames().last().map(TerminalFrame::kind);
    assert_eq!(last, Some(TerminalKind::ProcessExit), "the exit comes last");
}

#[test]
fn a_mode_changing_chunk_waits_for_room_for_its_modes_and_output() {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            Ok::<_, std::convert::Infallible>(ModalScreen {
                inner: PlainTerminalScreenRuntime::new(size),
                bracketed_paste: false,
            })
        });
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn");
    let (subscription_id, reader) = bind(&mut runtime);
    // 63 chunks leave one free slot in the 64-frame route just before the
    // mode-changing chunk, whose MODES frame precedes its OUTPUT.
    let mut chunks: Vec<Vec<u8>> = (0..63).map(chunk).collect();
    chunks.push(b"\x1b[?2004hmodes-on\n".to_vec());
    chunks.extend((63..90).map(chunk));
    let expected: Vec<u8> = chunks.concat();
    for data in chunks {
        runtime
            .session_runtime_mut()
            .emit_output(session_id(), data);
    }
    let (done, held) = pace_until(&mut runtime, &subscription_id, &reader, || {
        reader.output().len() >= expected.len()
    });
    assert!(done, "every chunk reaches the reader");
    assert!(held, "the chunks exceed the route bound: the session held");
    assert_eq!(
        reader.count(TerminalKind::RouteResync),
        0,
        "a MODES frame never overflows the route"
    );
    assert!(reader.output() == expected, "every byte arrives, in order");
    assert!(reader.count(TerminalKind::Modes) >= 1);
}

#[test]
fn a_process_exit_waits_for_room_for_its_input_results_too() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn");
    let (subscription_id, reader) = bind(&mut runtime);
    let batch = TerminalWakeBatch {
        adapter_routes: vec![TerminalWakeRoute {
            session_id: session_id(),
            subscription_id: subscription_id.clone(),
        }],
        ingress_sessions: vec![session_id()],
    };
    // 63 output frames wait on the 64-frame route: the reader takes none.
    let expected: Vec<u8> = (0..63).flat_map(chunk).collect();
    for index in 0..63 {
        runtime
            .session_runtime_mut()
            .emit_output(session_id(), chunk(index));
    }
    runtime.pump_woken(&batch, 2).expect("fill the route");
    // An input operation arrives in the same pump as the exit: the exit
    // fails it with a SessionEnded INPUT_RESULT ahead of PROCESS_EXIT, so
    // the exit sequence needs two frames on a route that has one free.
    reader.inject(
        botster_terminal_protocol_client::encode_terminal_input(
            &botster_terminal_protocol_client::TerminalInputCommand::RawBytes {
                operation_id: 1,
                data: b"late".to_vec(),
            },
        )
        .expect("input frame")
        .into_bytes(),
    );
    runtime.session_runtime_mut().emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );
    let (done, _) = pace_until(&mut runtime, &subscription_id, &reader, || {
        reader.count(TerminalKind::ProcessExit) > 0
    });
    assert!(done, "the exit reaches the reader");
    assert_eq!(reader.count(TerminalKind::RouteResync), 0);
    assert!(
        reader.output() == expected,
        "every queued frame arrives, in order, before the exit"
    );
    assert_eq!(reader.count(TerminalKind::InputResult), 1);
    let last = reader.frames().last().map(TerminalFrame::kind);
    assert_eq!(last, Some(TerminalKind::ProcessExit), "the exit comes last");
}

#[test]
fn more_than_sixteen_inputs_queued_at_exit_keep_the_route_and_its_output() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn");
    let (subscription_id, reader) = bind(&mut runtime);
    let batch = TerminalWakeBatch {
        adapter_routes: vec![TerminalWakeRoute {
            session_id: session_id(),
            subscription_id: subscription_id.clone(),
        }],
        ingress_sessions: vec![session_id()],
    };
    // Visual output waits on the route: the reader takes none yet.
    let expected: Vec<u8> = (0..40).flat_map(chunk).collect();
    for index in 0..40 {
        runtime
            .session_runtime_mut()
            .emit_output(session_id(), chunk(index));
    }
    runtime.pump_woken(&batch, 2).expect("queue the output");
    // Twenty accepted operations are still queued when the exit is routed:
    // more than the route's 16 queued rejections, within its 64 frames.
    for operation_id in 1..=20 {
        reader.inject(
            botster_terminal_protocol_client::encode_terminal_input(
                &botster_terminal_protocol_client::TerminalInputCommand::RawBytes {
                    operation_id,
                    data: b"late".to_vec(),
                },
            )
            .expect("input frame")
            .into_bytes(),
        );
    }
    runtime.session_runtime_mut().emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );
    let (done, _) = pace_until(&mut runtime, &subscription_id, &reader, || {
        reader.count(TerminalKind::ProcessExit) > 0
    });
    assert!(done, "the exit reaches the reader");
    assert_eq!(reader.count(TerminalKind::RouteResync), 0);
    assert!(
        reader.output() == expected,
        "the queued output survives the exit's results"
    );
    assert_eq!(
        reader.count(TerminalKind::InputResult),
        20,
        "every queued operation gets its SessionEnded result"
    );
    let last = reader.frames().last().map(TerminalFrame::kind);
    assert_eq!(last, Some(TerminalKind::ProcessExit), "the exit comes last");
}
