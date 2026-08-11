//! Terminal screen boundary contract tests.

use std::collections::HashMap;

use botster_core::{
    ModeFlags, ModeFlagsReady, PreparedSnapshotReady, PreparedSnapshotRequest, RequestId,
    ScreenReady, SendFileFailed, SendFileRequest, SendFileWritten, SessionId, SessionIoEvent,
    SessionIoRequest, SessionWorkerEngine, SessionWorkerRuntime, SnapshotReady,
    TerminalColorProfile, TerminalScreenEngine, TerminalScreenHook, TerminalScreenRuntime,
    TerminalScreenSize, TerminalSnapshotPayload,
};
use botster_core_test_support::fake::FakeTerminalScreenRuntime;

use botster_core::engine::terminal_screen::{
    PlainTerminalScreenRuntime, PLAIN_TERMINAL_SCREEN_MAX_BYTES,
};

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("session-terminal-screen".to_string())
}

fn terminal_engine() -> TerminalScreenEngine<FakeTerminalScreenRuntime> {
    TerminalScreenEngine::new(FakeTerminalScreenRuntime::new())
}

fn oversized_plain_bytes() -> Vec<u8> {
    let mut bytes = vec![b'a'; PLAIN_TERMINAL_SCREEN_MAX_BYTES];
    bytes.extend_from_slice(b"bounded-tail");
    bytes
}

#[test]
fn normalizes_output_without_losing_raw_bytes() {
    let mut engine = terminal_engine();
    let bytes = b"\x1b[31mhello\x00\xff".to_vec();

    let outcome = engine.normalize_output(&bytes);

    assert_eq!(
        outcome.hooks,
        vec![TerminalScreenHook::OutputNormalized { bytes: bytes.len() }]
    );
    assert!(matches!(
        outcome.output,
        Some(output) if output.bytes == bytes
    ));
    assert_eq!(engine.runtime().bytes(), bytes.as_slice());
}

#[test]
fn capture_and_replay_round_trips_opaque_snapshot_bytes() {
    let mut engine = terminal_engine();
    let bytes = b"\x00snapshot\xffbytes".to_vec();

    engine.resize(TerminalScreenSize::new(43, 132));
    engine.normalize_output(&bytes);
    let captured = engine.capture_snapshot();
    let snapshot = match captured.snapshot {
        Some(snapshot) => snapshot,
        None => panic!("snapshot should be captured"),
    };

    let mut replay_engine = terminal_engine();
    let replayed = replay_engine.replay_snapshot(snapshot.clone());

    assert_eq!(
        replayed.hooks,
        vec![TerminalScreenHook::SnapshotReplayed {
            size: TerminalScreenSize::new(43, 132)
        }]
    );
    assert_eq!(replay_engine.runtime().bytes(), bytes.as_slice());
    assert_eq!(
        replay_engine.capture_snapshot().snapshot,
        Some(TerminalSnapshotPayload::new(
            bytes,
            TerminalScreenSize::new(43, 132),
            snapshot.format,
        ))
    );
}

#[test]
fn screen_state_syncs_title_cwd_and_mode_without_pushed_change_events() {
    let mut engine = terminal_engine();
    let mode_flags = ModeFlags {
        cursor_visible: true,
        bracketed_paste: true,
        mouse_mode: 2,
        ..ModeFlags::default()
    };
    let mut colors = HashMap::new();
    colors.insert(1, botster_core::Rgb { r: 255, g: 0, b: 0 });

    engine.runtime_mut().set_synced_state(
        Some("shell".to_string()),
        Some("file:///tmp/project".to_string()),
        mode_flags.clone(),
        Some(TerminalColorProfile { colors }),
    );
    engine.normalize_output(b"prompt> ");
    let state_outcome = engine.screen_state();
    let state = match state_outcome.screen {
        Some(state) => state,
        None => panic!("screen state should be read"),
    };

    assert_eq!(
        state_outcome.hooks,
        vec![TerminalScreenHook::ScreenRead {
            size: TerminalScreenSize::new(24, 80)
        }]
    );
    assert_eq!(state.title, Some("shell".to_string()));
    assert_eq!(state.cwd, Some("file:///tmp/project".to_string()));
    assert_eq!(state.mode_flags, mode_flags);
    assert!(state.color_profile.is_some());
}

#[test]
fn plain_screen_read_uses_fake_runtime_without_terminal_backend_dependency() {
    let mut engine = terminal_engine();

    engine.normalize_output(b"one\r\ntwo");
    let state = match engine.screen_state().screen {
        Some(state) => state,
        None => panic!("screen state should be read"),
    };

    assert_eq!(state.plain_text, "one\r\ntwo");
    assert_eq!(state.size, TerminalScreenSize::new(24, 80));
}

#[test]
fn plain_runtime_bounds_snapshot_and_screen_state_without_truncating_output_chunk() {
    let mut engine = TerminalScreenEngine::new(PlainTerminalScreenRuntime::new(
        TerminalScreenSize::new(24, 80),
    ));
    let bytes = oversized_plain_bytes();

    let output = engine.normalize_output(&bytes);
    let snapshot = engine
        .capture_snapshot()
        .snapshot
        .expect("plain runtime captures snapshot");
    let screen = engine
        .screen_state()
        .screen
        .expect("plain runtime reads screen");

    assert!(matches!(
        output.output,
        Some(output) if output.bytes == bytes
    ));
    assert_eq!(snapshot.bytes.len(), PLAIN_TERMINAL_SCREEN_MAX_BYTES);
    assert_eq!(
        snapshot.bytes,
        bytes[bytes.len() - PLAIN_TERMINAL_SCREEN_MAX_BYTES..]
    );
    assert_eq!(screen.plain_text.len(), PLAIN_TERMINAL_SCREEN_MAX_BYTES);
    assert!(screen.plain_text.ends_with("bounded-tail"));
}

#[test]
fn plain_runtime_replay_snapshot_rebounds_oversized_payloads() {
    let mut engine = TerminalScreenEngine::new(PlainTerminalScreenRuntime::new(
        TerminalScreenSize::new(24, 80),
    ));
    let bytes = oversized_plain_bytes();

    engine.replay_snapshot(TerminalSnapshotPayload::new(
        bytes.clone(),
        TerminalScreenSize::new(40, 120),
        Some("plain-opaque-v1".to_string()),
    ));
    let snapshot = engine
        .capture_snapshot()
        .snapshot
        .expect("plain runtime captures replayed snapshot");
    let screen = engine
        .screen_state()
        .screen
        .expect("plain runtime reads replayed screen");

    assert_eq!(snapshot.bytes.len(), PLAIN_TERMINAL_SCREEN_MAX_BYTES);
    assert_eq!(
        snapshot.bytes,
        bytes[bytes.len() - PLAIN_TERMINAL_SCREEN_MAX_BYTES..]
    );
    assert_eq!(snapshot.size, TerminalScreenSize::new(40, 120));
    assert_eq!(screen.size, TerminalScreenSize::new(40, 120));
    assert!(screen.plain_text.ends_with("bounded-tail"));
}

#[test]
fn snapshot_payload_compatibility_matches_existing_session_protocol_shape() {
    let payload = TerminalSnapshotPayload::new(
        b"\x00opaque\xff".to_vec(),
        TerminalScreenSize::new(30, 100),
        Some("opaque-test".to_string()),
    );
    let snapshot_ready = payload
        .clone()
        .into_snapshot_ready(request_id("snapshot-1"), session_id());
    let payload_from_snapshot = TerminalSnapshotPayload::from_snapshot_ready(snapshot_ready);
    let prepared = payload.clone().into_prepared_snapshot_request(
        request_id("prepared-1"),
        session_id(),
        true,
    );

    assert_eq!(payload_from_snapshot.bytes, payload.bytes);
    assert_eq!(payload_from_snapshot.size, payload.size);
    assert_eq!(payload_from_snapshot.format, None);
    assert_eq!(prepared.snapshot, payload.bytes);
    assert!(prepared.recovery);
}

#[test]
fn terminal_screen_boundary_does_not_expose_renderer_policy() {
    let contract_source =
        std::fs::read_to_string("src/contract/terminal_screen.rs").expect("read contract source");
    let engine_source =
        std::fs::read_to_string("src/engine/terminal_screen.rs").expect("read engine source");
    let source = format!("{contract_source}\n{engine_source}");

    for forbidden in [
        "ModeChanged",
        "terminal-mode-delta",
        "TitleChanged",
        "CwdChanged",
        "ColorChanged",
        "title_delta",
        "cwd_delta",
        "color_delta",
        "BoundaryJson",
        "browser",
        "Browser",
        "tui",
        "Tui",
        "hub",
        "Hub",
        "renderer",
        "Renderer",
        "rendering",
        "Rendering",
    ] {
        assert!(
            !source.contains(forbidden),
            "terminal screen boundary must not expose {forbidden}"
        );
    }
}

#[derive(Debug, Clone)]
struct TerminalBackedSessionRuntime {
    engine: TerminalScreenEngine<FakeTerminalScreenRuntime>,
}

impl TerminalBackedSessionRuntime {
    fn new() -> Self {
        let mut engine = terminal_engine();
        engine.normalize_output(b"session screen");
        Self { engine }
    }
}

impl SessionWorkerRuntime for TerminalBackedSessionRuntime {
    fn write_input(&mut self, _session_id: &SessionId, data: &[u8]) {
        self.engine.normalize_output(data);
    }

    fn resize(
        &mut self,
        _session_id: &SessionId,
        rows: u16,
        cols: u16,
    ) -> Result<(), botster_core::SessionRuntimeError> {
        self.engine.resize(TerminalScreenSize::new(rows, cols));
        Ok(())
    }

    fn snapshot(&mut self, request_id: RequestId, session_id: SessionId) -> SnapshotReady {
        match self.engine.capture_snapshot().snapshot {
            Some(snapshot) => snapshot.into_snapshot_ready(request_id, session_id),
            None => TerminalSnapshotPayload::new(Vec::new(), TerminalScreenSize::new(0, 0), None)
                .into_snapshot_ready(request_id, session_id),
        }
    }

    fn request_initial_snapshot(
        &mut self,
        _request: botster_core::InitialSnapshotRequest,
    ) -> Result<(), botster_core::SessionRuntimeError> {
        Ok(())
    }

    fn send_file(&mut self, request: SendFileRequest) -> Result<SendFileWritten, SendFileFailed> {
        Ok(SendFileWritten {
            request_id: request.request_id,
            session_id: request.session_id,
            bytes: request.data.len(),
            storage_ref: None,
        })
    }

    fn prepare_snapshot(&mut self, request: PreparedSnapshotRequest) -> PreparedSnapshotReady {
        PreparedSnapshotReady {
            request_id: request.request_id,
            session_id: request.session_id,
            uncompressed_len: request.snapshot.len(),
            payload: request.snapshot,
            recovery: request.recovery,
        }
    }

    fn mode_flags(
        &mut self,
        request_id: RequestId,
        session_id: SessionId,
    ) -> Result<ModeFlagsReady, botster_core::SessionRuntimeError> {
        let state = match self.engine.screen_state().screen {
            Some(state) => state,
            None => {
                botster_core::TerminalScreenState::new(TerminalScreenSize::new(0, 0), String::new())
            }
        };
        Ok(ModeFlagsReady {
            request_id,
            session_id,
            mode_flags: state.mode_flags,
            mode_freshness: botster_core::ModeFreshnessToken::default(),
        })
    }

    fn screen(&mut self, request_id: RequestId, session_id: SessionId) -> ScreenReady {
        let state = match self.engine.screen_state().screen {
            Some(state) => state,
            None => {
                botster_core::TerminalScreenState::new(TerminalScreenSize::new(0, 0), String::new())
            }
        };
        ScreenReady {
            request_id,
            session_id,
            text: state.plain_text,
        }
    }

    fn set_color_profile(
        &mut self,
        _session_id: &SessionId,
        color_profile: TerminalColorProfile,
    ) -> Result<(), botster_core::SessionRuntimeError> {
        let state = self.engine.runtime().screen_state();
        self.engine.runtime_mut().set_synced_state(
            state.title,
            state.cwd,
            state.mode_flags,
            Some(color_profile),
        );
        Ok(())
    }

    fn shutdown(
        &mut self,
        _session_id: &SessionId,
        _reason: &str,
    ) -> Result<Vec<botster_core::SessionWorkerRuntimeEvent>, botster_core::SessionRuntimeError>
    {
        Ok(Vec::new())
    }
}

#[test]
fn terminal_screen_runtime_seam_feeds_existing_session_worker_contracts() {
    let runtime = TerminalBackedSessionRuntime::new();
    let mut worker = SessionWorkerEngine::new(runtime);

    worker
        .handle_request(SessionIoRequest::Resize {
            session_id: session_id(),
            rows: 50,
            cols: 120,
        })
        .expect("resize request succeeds");
    let snapshot = worker
        .handle_request(SessionIoRequest::GetSnapshot {
            request_id: request_id("snapshot-1"),
            session_id: session_id(),
        })
        .expect("snapshot request succeeds");
    let prepared = worker
        .handle_request(SessionIoRequest::PrepareSnapshot(PreparedSnapshotRequest {
            request_id: request_id("prepared-1"),
            session_id: session_id(),
            snapshot: b"prepared\xff".to_vec(),
            recovery: true,
        }))
        .expect("prepared snapshot request succeeds");
    let mode = worker
        .handle_request(SessionIoRequest::GetModeFlags {
            request_id: request_id("mode-1"),
            session_id: session_id(),
        })
        .expect("mode flags request succeeds");
    let screen = worker
        .handle_request(SessionIoRequest::GetScreen {
            request_id: request_id("screen-1"),
            session_id: session_id(),
        })
        .expect("screen request succeeds");

    assert!(matches!(
        &snapshot.events[0],
        SessionIoEvent::SnapshotReady(snapshot)
            if snapshot.data == b"session screen" && snapshot.rows == 50 && snapshot.cols == 120
    ));
    assert!(matches!(
        &prepared.events[0],
        SessionIoEvent::PreparedSnapshotReady(prepared)
            if prepared.payload == b"prepared\xff" && prepared.recovery
    ));
    assert!(matches!(mode.events[0], SessionIoEvent::ModeFlagsReady(_)));
    assert!(matches!(
        &screen.events[0],
        SessionIoEvent::ScreenReady(screen) if screen.text == "session screen"
    ));
}
