//! Local session worker process entrypoint.
//!
//! Hosted by `botster-core-daemon` so the worker can depend on
//! `botster-terminal-ghostty` without a Cargo cycle through package
//! `botster-core`. The worker owns the only server terminal parser for its
//! session: every PTY output byte is parsed by the worker Ghostty whether or
//! not a client is attached, and every client input operation is encoded here
//! against the current terminal modes.

use std::collections::{HashMap, VecDeque};
#[cfg(unix)]
use std::fs::DirBuilder;
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use botster_core::contract::terminal_screen::{TerminalKeyEvent, TerminalMouseEvent};
use botster_core::contract::terminal_wake::TerminalWakeSource;
use botster_core::engine::TerminalScreenRuntime;
use botster_core::{
    decode_worker_input_operation, encode_final_state, read_hello, write_startup_failure,
    write_welcome, Frame, LocalProcessRuntime, LocalProcessRuntimeOptions, ModeFlags,
    ModeFlagsPayload, ReservedSessionSpawnError, ResizePayload, ScreenPayload, SessionId,
    SessionMetadata, SessionReservation, SessionRuntime, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest, StartupFailureOutcome, StartupFailureReport, TerminalMetadataKind,
    TerminalMetadataLaneShaper, TerminalMetadataObservation, TerminalMetadataProducer,
    TerminalMetadataShapingObservation, TerminalMetadataShapingOutcome, TerminalScreenSize,
    TimeoutPayload, WorkerFinalState, WorkerHealth, WorkerInputKind, WorkerProbeRequest,
    WorkerSnapshotPhase, WorkerSnapshotRequest, WorkerSnapshotResult, FRAME_BELL,
    FRAME_CWD_CHANGED, FRAME_FINAL_STATE, FRAME_GET_MODE_FLAGS, FRAME_GET_SCREEN,
    FRAME_GET_SNAPSHOT, FRAME_INPUT_CANCEL, FRAME_INPUT_OPERATION, FRAME_INPUT_RESULT,
    FRAME_METADATA_SHAPING, FRAME_MODES_CHANGED, FRAME_MODE_FLAGS, FRAME_NOTIFICATION, FRAME_PING,
    FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT,
    FRAME_RESIZE, FRAME_RESIZE_APPLIED, FRAME_SCREEN, FRAME_SET_TIMEOUT, FRAME_SHUTDOWN,
    FRAME_SNAPSHOT, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED, PROTOCOL_VERSION,
};
use botster_terminal_ghostty::{
    GhosttyAdapterConfig, GhosttySnapshotFrameKind, GhosttyTerminal, GHOSTTY_SNAPSHOT_FORMAT,
};
use botster_terminal_protocol::{
    encode_input_result, encode_modes, InputOutcome, InputResultBody, ModesBody,
    MAX_ENCODED_INPUT_BYTES, MAX_INPUT_OPERATIONS_PER_SESSION, MAX_PASTE_BYTES,
    MAX_RETAINED_INPUT_BYTES_PER_SESSION,
};
use botster_terminal_protocol_client::{
    decode_input_body, TerminalInputCommand, TerminalInputKind,
};

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr(),
            "botster-session-worker {} failed: {error}",
            process::id()
        );
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = WorkerArgs::parse(std::env::args().skip(1).collect())?;
    let control = WorkerControl::open(&args)?;
    if args.control_socket.is_some() {
        writeln!(
            io::stdout(),
            "botster-session-worker-ready {}",
            process::id()
        )
        .and_then(|_| io::stdout().flush())
        .map_err(|error| format!("publish worker readiness failed: {error}"))?;
    }
    let shutdown_on_disconnect = control.shutdown_on_disconnect();
    let mut initial_control = control.accept_initial()?;

    let peer_version = read_hello(&mut initial_control).map_err(|error| error.to_string())?;
    if peer_version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported parent protocol version: {peer_version}"
        ));
    }
    let spawn_frame = read_frame(&mut initial_control)?;
    if spawn_frame.frame_type != FRAME_SPAWN_SESSION {
        return Err("worker expected FRAME_SPAWN_SESSION after hello".to_string());
    }
    let spawn_request: SessionSpawnRequest = match serde_json::from_slice(&spawn_frame.payload) {
        Ok(request) => request,
        Err(error) => return Err(error.to_string()),
    };
    let session_id = spawn_request.session_id.clone();
    let request_id = spawn_request.request_id.clone();

    let wakes = TerminalWakeSource::new();
    let runtime_options = LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(args.shutdown_grace_ms),
        pty_reader_chunk_capacity: args.pty_reader_chunk_capacity,
        test_hold_after_read_ms: args.test_hold_after_read_ms,
        test_pending_capacity: args.test_pending_capacity,
        test_hold_after_enqueue_ms: args.test_hold_after_enqueue_ms,
    };
    let mut runtime =
        LocalProcessRuntime::with_options(runtime_options).with_wake_source(wakes.clone());
    let initial_size = spawn_request
        .initial_pty_size
        .clone()
        .unwrap_or(ResizePayload { rows: 24, cols: 80 });
    let reservation = runtime
        .session_admission()
        .ok_or_else(|| "worker local runtime has no session admission".to_string())?
        .reserve(session_id.clone())
        .map_err(|error| format!("{error:?}"))?;
    let handle = match runtime.spawn_reserved(&reservation, spawn_request) {
        Ok(handle) => handle,
        Err(error) => {
            write_worker_startup_failure(
                &mut initial_control,
                &request_id.0,
                &session_id.0,
                &reservation,
                &error,
            )?;
            return Err(error.to_string());
        }
    };
    if args.test_fail_after_spawn {
        write_created_startup_failure(
            &mut initial_control,
            &request_id.0,
            &session_id.0,
            handle.process.pid,
            runtime
                .session_process_group(&session_id)
                .ok()
                .flatten()
                .filter(|group| *group > 1),
            "test fail after spawn",
        )?;
        return Err("test fail after spawn".to_string());
    }
    let initial_rows = initial_size.rows;
    let initial_cols = initial_size.cols;
    let mut ghostty = GhosttyTerminal::with_config(
        TerminalScreenSize::new(initial_rows, initial_cols),
        GhosttyAdapterConfig::with_max_scrollback_bytes(args.ghostty_max_scrollback_bytes),
    )
    .map_err(|error| {
        let _ = write_created_startup_failure(
            &mut initial_control,
            &request_id.0,
            &session_id.0,
            handle.process.pid,
            runtime
                .session_process_group(&session_id)
                .ok()
                .flatten()
                .filter(|group| *group > 1),
            &format!("worker Ghostty init failed: {error}"),
        );
        format!("worker Ghostty init failed: {error}")
    })?;
    if let Some(profile) = args.terminal_color_profile.as_ref() {
        ghostty.apply_color_profile(profile).map_err(|error| {
            let _ = write_created_startup_failure(
                &mut initial_control,
                &request_id.0,
                &session_id.0,
                handle.process.pid,
                runtime
                    .session_process_group(&session_id)
                    .ok()
                    .flatten()
                    .filter(|group| *group > 1),
                &format!("worker Ghostty color profile failed: {error}"),
            );
            format!("worker Ghostty color profile failed: {error}")
        })?;
    }
    let initial_flags = ghostty.read_mode_flags().map_err(|error| {
        let _ = write_created_startup_failure(
            &mut initial_control,
            &request_id.0,
            &session_id.0,
            handle.process.pid,
            runtime
                .session_process_group(&session_id)
                .ok()
                .flatten()
                .filter(|group| *group > 1),
            &format!("worker initial mode flags failed: {error}"),
        );
        format!("worker initial mode flags failed: {error}")
    })?;
    let metadata = SessionMetadata {
        session_uuid: session_id.0.clone(),
        pid: handle.process.pid.unwrap_or_else(process::id),
        rows: initial_rows,
        cols: initial_cols,
        last_output_at: 0,
        title: None,
        cwd: None,
        port: None,
        mode_flags: initial_flags.clone(),
        recovery_identity: Some(serde_json::json!({
            "session_uuid": session_id.0,
            "runtime_id": handle.process.runtime_id,
            "worker_pid": process::id(),
            "process_group_id": runtime.session_process_group(&session_id)
                .map_err(|error| error.to_string())?,
            "worker_control_socket": args.control_socket,
            "atomic_snapshot_boundary": true,
            "snapshot_delivery": "ready_then_history",
            "terminal_stream_scheme": 2,
        })),
    };
    write_welcome(&mut initial_control, &metadata).map_err(|error| error.to_string())?;

    let (egress, protected_receiver, metadata_receiver) =
        WorkerEgress::new(args.egress_capacity.max(1));
    let (frame_sender, frame_receiver) = mpsc::channel();
    let snapshot_barrier = Arc::new(SnapshotBarrierControl {
        egress_space: Arc::clone(&egress.space),
        ..SnapshotBarrierControl::default()
    });
    let control_connections = Arc::new(AtomicUsize::new(0));
    control.spawn_readers(
        initial_control,
        frame_sender,
        Arc::clone(&snapshot_barrier),
        metadata.clone(),
        wakes.clone(),
        session_id.clone(),
        Arc::clone(&control_connections),
    );

    let writer = control.spawn_writer(
        protected_receiver,
        metadata_receiver,
        Arc::clone(&egress.space),
    );
    let mut state = WorkerState {
        session_id: handle.session_id.clone(),
        ghostty,
        metadata_producer: TerminalMetadataProducer::new(),
        metadata_shaper: TerminalMetadataLaneShaper::new(
            (args.egress_capacity / 2).max(1),
            args.egress_capacity.saturating_mul(4).max(1),
        ),
        egress,
        last_modes: ModesBody {
            mode_bits: initial_flags.to_mode_bits(),
            rows: initial_rows,
            cols: initial_cols,
        },
        pending_writes: VecDeque::new(),
        pending_ops: 0,
        pending_bytes: 0,
        exited: false,
        deferring_exit: false,
        deferred_exit: None,
    };
    let mut reconnect_timeout_seconds = None;
    let mut lifecycle = WorkerLifecycle::default();

    while lifecycle.should_continue() {
        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => match frame.frame_type {
                    FRAME_PTY_INPUT => {
                        state.queue_keyless_write(frame.payload);
                    }
                    FRAME_INPUT_OPERATION => {
                        // Apply every drained byte before encoding so modes are current.
                        state.drain_and_apply_pty_output(&mut runtime)?;
                        state.admit_input_operation(&mut runtime, &frame.payload);
                    }
                    FRAME_INPUT_CANCEL => {
                        if let Ok((key, _)) =
                            botster_core::split_worker_operation_key(&frame.payload)
                        {
                            state.cancel_operation(key);
                        }
                    }
                    FRAME_GET_MODE_FLAGS => {
                        let request_id = probe_request_id(&frame.payload);
                        state.drain_and_apply_pty_output(&mut runtime)?;
                        let size = state.ghostty.size();
                        let payload = match state.ghostty.read_mode_flags() {
                            Ok(mode_flags) => ModeFlagsPayload {
                                request_id,
                                mode_flags,
                                rows: size.rows,
                                cols: size.cols,
                                error_kind: None,
                            },
                            Err(error) => ModeFlagsPayload {
                                request_id,
                                mode_flags: ModeFlags::default(),
                                rows: size.rows,
                                cols: size.cols,
                                error_kind: Some(error.to_string()),
                            },
                        };
                        state.egress.send_protected_json(FRAME_MODE_FLAGS, &payload);
                    }
                    FRAME_GET_SCREEN => {
                        let request_id = probe_request_id(&frame.payload);
                        state.drain_and_apply_pty_output(&mut runtime)?;
                        let payload = match state.ghostty.plain_text() {
                            Ok(text) => ScreenPayload {
                                request_id,
                                text,
                                error_kind: None,
                            },
                            Err(error) => ScreenPayload {
                                request_id,
                                text: String::new(),
                                error_kind: Some(error.to_string()),
                            },
                        };
                        state.egress.send_protected_json(FRAME_SCREEN, &payload);
                    }
                    FRAME_RESIZE => {
                        let size: ResizePayload = serde_json::from_slice(&frame.payload)
                            .map_err(|error| error.to_string())?;
                        if state.exited {
                            // No PTY is left; the final model takes the size.
                            state
                                .ghostty
                                .resize(TerminalScreenSize::new(size.rows, size.cols));
                            state.publish_modes_if_changed();
                        } else {
                            state.apply_resize(&mut runtime, size.rows, size.cols, None)?;
                        }
                        state
                            .egress
                            .send_protected_json(FRAME_RESIZE_APPLIED, &size);
                    }
                    FRAME_GET_SNAPSHOT => {
                        state.handle_snapshot_request(
                            &mut runtime,
                            &snapshot_barrier,
                            &frame.payload,
                        );
                    }
                    FRAME_PING => {
                        let health = WorkerHealth {
                            session_id: state.session_id.clone(),
                            worker_pid: process::id(),
                            reconnect_timeout_seconds,
                        };
                        state.egress.send_protected_json(FRAME_PONG, &health);
                    }
                    FRAME_SET_TIMEOUT => {
                        let timeout: TimeoutPayload = serde_json::from_slice(&frame.payload)
                            .map_err(|error| error.to_string())?;
                        reconnect_timeout_seconds = Some(timeout.seconds);
                    }
                    FRAME_SHUTDOWN => {
                        if !state.exited {
                            runtime
                                .send_input(SessionRuntimeInput::Shutdown {
                                    session_id: state.session_id.clone(),
                                })
                                .map_err(|error| error.to_string())?;
                        }
                        lifecycle.request_shutdown();
                    }
                    _ => {}
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if shutdown_on_disconnect {
                        if !state.exited {
                            runtime
                                .send_input(SessionRuntimeInput::Shutdown {
                                    session_id: state.session_id.clone(),
                                })
                                .map_err(|error| error.to_string())?;
                        }
                        lifecycle.request_shutdown();
                    }
                    break;
                }
            }
        }

        state.drain_and_apply_pty_output(&mut runtime)?;
        if state.exited {
            lifecycle.observe_process_exit();
            // A socket worker whose every control connection has closed has
            // no parent left to serve.
            if !shutdown_on_disconnect && control_connections.load(Ordering::SeqCst) == 0 {
                lifecycle.request_shutdown();
            }
            if !lifecycle.should_continue() {
                break;
            }
            // Control frames and each control connection's end post session
            // wakes, so this wait needs no timer.
            let _ = wakes.wait_wakes_untimed();
            continue;
        }
        if state.progress_pending_writes(&runtime) {
            runtime
                .wake_when_writable(&state.session_id)
                .map_err(|error| error.to_string())?;
        }
        // Control frames, PTY output, child exit, and PTY writability all
        // post session wakes, so the loop needs no timer.
        let _ = wakes.wait_wakes_untimed();
    }

    if let Some(hold_ms) = args.test_hold_before_exit_ms {
        thread::sleep(Duration::from_millis(hold_ms));
    }

    drop(state);
    writer
        .join()
        .map_err(|_| "worker egress writer panicked".to_string())??;
    if let Some(exit_code) = args.test_exit_code {
        process::exit(exit_code);
    }
    Ok(())
}

/// `FRAME_INPUT_RESULT` payload: `[u64 LE key][complete INPUT_RESULT TerminalBody]`.
///
/// The parent splits the key and validates the full TerminalBody header, so
/// the body is sent with its header, never as bare body bytes.
fn input_result_payload(key: u64, result: &InputResultBody) -> Option<Vec<u8>> {
    let frame = encode_input_result(result).ok()?;
    Some(botster_core::encode_worker_operation(key, frame.as_bytes()))
}

/// `FRAME_MODES_CHANGED` payload: one complete MODES TerminalBody.
fn modes_changed_payload(modes: ModesBody) -> Option<Vec<u8>> {
    Some(encode_modes(modes).ok()?.as_bytes().to_vec())
}

fn probe_request_id(payload: &[u8]) -> String {
    serde_json::from_slice::<WorkerProbeRequest>(payload)
        .map(|request| request.request_id)
        .unwrap_or_default()
}

/// One queued PTY write. Keyless writes come from `FRAME_PTY_INPUT` and from
/// Ghostty `write_pty` query replies; keyed writes are client operations.
struct PendingWrite {
    key: Option<u64>,
    operation_id: u64,
    accepted_payload_bytes: u64,
    bytes: Vec<u8>,
    written: usize,
}

struct WorkerState {
    session_id: SessionId,
    ghostty: GhosttyTerminal,
    metadata_producer: TerminalMetadataProducer,
    metadata_shaper: TerminalMetadataLaneShaper,
    egress: WorkerEgress,
    last_modes: ModesBody,
    pending_writes: VecDeque<PendingWrite>,
    /// Keyed operations admitted and not yet reported.
    pending_ops: usize,
    /// Encoded bytes retained across keyed operations.
    pending_bytes: usize,
    exited: bool,
    /// A snapshot request is being served: an exit met while draining under
    /// its barrier waits here, so PROCESS_EXITED (terminal on the egress)
    /// cannot close the egress before the snapshot frames are sent.
    deferring_exit: bool,
    deferred_exit: Option<botster_core::ProcessExitedPayload>,
}

impl WorkerState {
    fn current_mode_bits(&self) -> u32 {
        self.ghostty
            .read_mode_flags()
            .map(|flags| flags.to_mode_bits())
            .unwrap_or(self.last_modes.mode_bits)
    }

    fn queue_keyless_write(&mut self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        self.pending_writes.push_back(PendingWrite {
            key: None,
            operation_id: 0,
            accepted_payload_bytes: bytes.len() as u64,
            bytes,
            written: 0,
        });
    }

    fn send_result(&self, key: u64, result: &InputResultBody) {
        // A body that cannot encode is a programming error in this process;
        // the parent reports OutcomeUnknown on link loss.
        if let Some(payload) = input_result_payload(key, result) {
            self.egress
                .send_protected_frame(FRAME_INPUT_RESULT, payload);
        }
    }

    fn reject(&self, key: u64, operation_id: u64, outcome: InputOutcome, detail: &str) {
        self.send_result(
            key,
            &InputResultBody {
                operation_id,
                outcome,
                accepted_payload_bytes: Some(0),
                written_pty_bytes: Some(0),
                mode_bits: self.current_mode_bits(),
                detail: detail.to_owned(),
            },
        );
    }

    /// Admit one parent-forwarded operation: decode, bound, encode, queue.
    fn admit_input_operation(&mut self, runtime: &mut LocalProcessRuntime, payload: &[u8]) {
        let operation = match decode_worker_input_operation(payload) {
            Ok(operation) => operation,
            Err(_) => return,
        };
        let key = operation.key;
        let operation_id = operation.operation_id;
        if self.exited {
            self.reject(
                key,
                operation_id,
                InputOutcome::SessionEnded,
                "session ended",
            );
            return;
        }
        if self.pending_ops >= MAX_INPUT_OPERATIONS_PER_SESSION
            || self.pending_bytes >= MAX_RETAINED_INPUT_BYTES_PER_SESSION
        {
            self.reject(
                key,
                operation_id,
                InputOutcome::RejectedLaneFull,
                "worker input lane is full",
            );
            return;
        }
        let mut encoded = Vec::new();
        let accepted_payload_bytes;
        match operation.kind {
            WorkerInputKind::RawBytes => {
                accepted_payload_bytes = operation.body.len();
                encoded.extend_from_slice(operation.body);
            }
            WorkerInputKind::Key => {
                let command =
                    match decode_input_body(TerminalInputKind::Key, operation_id, operation.body) {
                        Ok(command) => command,
                        Err(error) => {
                            self.reject(
                                key,
                                operation_id,
                                InputOutcome::RejectedProtocol,
                                &error.to_string(),
                            );
                            return;
                        }
                    };
                let TerminalInputCommand::Key {
                    action,
                    key: physical,
                    mods,
                    consumed_mods,
                    composing,
                    unshifted_codepoint,
                    text,
                    ..
                } = command
                else {
                    return;
                };
                accepted_payload_bytes = text.len();
                let event = TerminalKeyEvent {
                    action,
                    key: physical,
                    mods,
                    consumed_mods,
                    composing,
                    unshifted_codepoint,
                    text: &text,
                };
                if let Err(error) =
                    TerminalScreenRuntime::encode_key(&mut self.ghostty, &event, &mut encoded)
                {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::WriteFailed,
                        &error.to_string(),
                    );
                    return;
                }
            }
            WorkerInputKind::Mouse => {
                let command =
                    match decode_input_body(TerminalInputKind::Mouse, operation_id, operation.body)
                    {
                        Ok(command) => command,
                        Err(error) => {
                            self.reject(
                                key,
                                operation_id,
                                InputOutcome::RejectedProtocol,
                                &error.to_string(),
                            );
                            return;
                        }
                    };
                let TerminalInputCommand::Mouse {
                    action,
                    button,
                    mods,
                    col,
                    row,
                    x_px,
                    y_px,
                    ..
                } = command
                else {
                    return;
                };
                accepted_payload_bytes = 0;
                let event = TerminalMouseEvent {
                    action,
                    button,
                    mods,
                    col,
                    row,
                    x_px,
                    y_px,
                };
                if let Err(error) =
                    TerminalScreenRuntime::encode_mouse(&mut self.ghostty, &event, &mut encoded)
                {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::WriteFailed,
                        &error.to_string(),
                    );
                    return;
                }
            }
            WorkerInputKind::Focus => {
                let command =
                    match decode_input_body(TerminalInputKind::Focus, operation_id, operation.body)
                    {
                        Ok(command) => command,
                        Err(error) => {
                            self.reject(
                                key,
                                operation_id,
                                InputOutcome::RejectedProtocol,
                                &error.to_string(),
                            );
                            return;
                        }
                    };
                let TerminalInputCommand::Focus { focused, .. } = command else {
                    return;
                };
                accepted_payload_bytes = 0;
                if let Err(error) =
                    TerminalScreenRuntime::encode_focus(&mut self.ghostty, focused, &mut encoded)
                {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::WriteFailed,
                        &error.to_string(),
                    );
                    return;
                }
            }
            WorkerInputKind::Resize => {
                let command = match decode_input_body(
                    TerminalInputKind::Resize,
                    operation_id,
                    operation.body,
                ) {
                    Ok(command) => command,
                    Err(error) => {
                        self.reject(
                            key,
                            operation_id,
                            InputOutcome::RejectedProtocol,
                            &error.to_string(),
                        );
                        return;
                    }
                };
                let TerminalInputCommand::Resize {
                    rows,
                    cols,
                    width_px,
                    height_px,
                    ..
                } = command
                else {
                    return;
                };
                match self.apply_resize(runtime, rows, cols, Some((width_px, height_px))) {
                    Ok(()) => {
                        self.egress.send_protected_json(
                            FRAME_RESIZE_APPLIED,
                            &ResizePayload { rows, cols },
                        );
                        self.send_result(
                            key,
                            &InputResultBody {
                                operation_id,
                                outcome: InputOutcome::Written,
                                accepted_payload_bytes: Some(0),
                                written_pty_bytes: Some(0),
                                mode_bits: self.current_mode_bits(),
                                detail: String::new(),
                            },
                        );
                    }
                    Err(error) => {
                        self.reject(key, operation_id, InputOutcome::WriteFailed, &error);
                    }
                }
                return;
            }
            WorkerInputKind::Paste => {
                let Some((allow_unsafe, data)) = operation.body.split_first() else {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedProtocol,
                        "paste body is empty",
                    );
                    return;
                };
                if data.len() > MAX_PASTE_BYTES {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedTooLarge,
                        "paste exceeds the paste ceiling",
                    );
                    return;
                }
                if *allow_unsafe == 0 && !GhosttyTerminal::paste_is_safe(data) {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedUnsafePaste,
                        "paste contains newlines or a bracketed paste end marker",
                    );
                    return;
                }
                accepted_payload_bytes = data.len();
                let mut scratch = data.to_vec();
                if let Err(error) = self.ghostty.encode_paste(&mut scratch, &mut encoded) {
                    self.reject(
                        key,
                        operation_id,
                        InputOutcome::WriteFailed,
                        &error.to_string(),
                    );
                    return;
                }
            }
        }
        if encoded.len() > MAX_ENCODED_INPUT_BYTES {
            self.reject(
                key,
                operation_id,
                InputOutcome::RejectedTooLarge,
                "encoded input exceeds the ceiling",
            );
            return;
        }
        if encoded.is_empty() {
            self.send_result(
                key,
                &InputResultBody {
                    operation_id,
                    outcome: InputOutcome::Written,
                    accepted_payload_bytes: Some(accepted_payload_bytes as u64),
                    written_pty_bytes: Some(0),
                    mode_bits: self.current_mode_bits(),
                    detail: String::new(),
                },
            );
            return;
        }
        self.pending_ops += 1;
        self.pending_bytes += encoded.len();
        self.pending_writes.push_back(PendingWrite {
            key: Some(key),
            operation_id,
            accepted_payload_bytes: accepted_payload_bytes as u64,
            bytes: encoded,
            written: 0,
        });
    }

    fn apply_resize(
        &mut self,
        runtime: &mut LocalProcessRuntime,
        rows: u16,
        cols: u16,
        pixels: Option<(u32, u32)>,
    ) -> Result<(), String> {
        if let Some((width_px, height_px)) = pixels {
            self.ghostty.set_surface_pixels(width_px, height_px);
        }
        self.ghostty.resize(TerminalScreenSize::new(rows, cols));
        runtime
            .send_input(SessionRuntimeInput::Resize {
                session_id: self.session_id.clone(),
                size: ResizePayload { rows, cols },
            })
            .map_err(|error| error.to_string())?;
        self.publish_modes_if_changed();
        Ok(())
    }

    /// Abandon the unwritten remainder of one keyed operation.
    fn cancel_operation(&mut self, key: u64) {
        let Some(position) = self
            .pending_writes
            .iter()
            .position(|write| write.key == Some(key))
        else {
            return;
        };
        let Some(write) = self.pending_writes.remove(position) else {
            return;
        };
        self.finish_keyed(&write);
        self.send_result(
            key,
            &InputResultBody {
                operation_id: write.operation_id,
                outcome: InputOutcome::Cancelled,
                accepted_payload_bytes: Some(write.accepted_payload_bytes),
                written_pty_bytes: Some(write.written as u64),
                mode_bits: self.current_mode_bits(),
                detail: String::new(),
            },
        );
    }

    fn finish_keyed(&mut self, write: &PendingWrite) {
        if write.key.is_some() {
            self.pending_ops = self.pending_ops.saturating_sub(1);
            self.pending_bytes = self.pending_bytes.saturating_sub(write.bytes.len());
        }
    }

    /// Write queued bytes without waiting. Returns `true` when the PTY would
    /// block and bytes remain.
    fn progress_pending_writes(&mut self, runtime: &LocalProcessRuntime) -> bool {
        while let Some(head) = self.pending_writes.front_mut() {
            let remaining = &head.bytes[head.written..];
            match runtime.try_write_input(&self.session_id, remaining) {
                Ok(written) => {
                    head.written += written;
                    if head.written < head.bytes.len() {
                        return true;
                    }
                    let write = self.pending_writes.pop_front().expect("front write exists");
                    self.finish_keyed(&write);
                    if let Some(key) = write.key {
                        self.send_result(
                            key,
                            &InputResultBody {
                                operation_id: write.operation_id,
                                outcome: InputOutcome::Written,
                                accepted_payload_bytes: Some(write.accepted_payload_bytes),
                                written_pty_bytes: Some(write.written as u64),
                                mode_bits: self.current_mode_bits(),
                                detail: String::new(),
                            },
                        );
                    }
                }
                Err(failure) => {
                    let mut write = self.pending_writes.pop_front().expect("front write exists");
                    write.written += failure.bytes_written;
                    self.finish_keyed(&write);
                    if let Some(key) = write.key {
                        let outcome = if write.written > 0 {
                            InputOutcome::PartialWrite
                        } else {
                            InputOutcome::WriteFailed
                        };
                        self.send_result(
                            key,
                            &InputResultBody {
                                operation_id: write.operation_id,
                                outcome,
                                accepted_payload_bytes: Some(write.accepted_payload_bytes),
                                written_pty_bytes: Some(write.written as u64),
                                mode_bits: self.current_mode_bits(),
                                detail: failure.message.clone(),
                            },
                        );
                    }
                }
            }
        }
        false
    }

    /// Report every unfinished keyed operation as ended, before the exit frame.
    fn fail_pending_on_exit(&mut self) {
        let writes: Vec<_> = self.pending_writes.drain(..).collect();
        for write in writes {
            self.finish_keyed(&write);
            if let Some(key) = write.key {
                self.send_result(
                    key,
                    &InputResultBody {
                        operation_id: write.operation_id,
                        outcome: InputOutcome::SessionEnded,
                        accepted_payload_bytes: Some(write.accepted_payload_bytes),
                        written_pty_bytes: Some(write.written as u64),
                        mode_bits: self.last_modes.mode_bits,
                        detail: String::new(),
                    },
                );
            }
        }
    }

    fn publish_modes_if_changed(&mut self) {
        let Ok(flags) = self.ghostty.read_mode_flags() else {
            return;
        };
        let size = self.ghostty.size();
        let modes = ModesBody {
            mode_bits: flags.to_mode_bits(),
            rows: size.rows,
            cols: size.cols,
        };
        if modes != self.last_modes {
            self.last_modes = modes;
            if let Some(payload) = modes_changed_payload(modes) {
                self.egress
                    .send_protected_frame(FRAME_MODES_CHANGED, payload);
            }
        }
    }

    fn apply_pty_output_chunk(&mut self, data: Vec<u8>) {
        let observations = self.metadata_producer.observe(&data);
        self.ghostty.write_output(&data);
        // The worker is the only parser: inject query replies into the PTY here.
        let replies = self.ghostty.drain_pty_writes();
        self.queue_keyless_write(replies);
        self.publish_modes_if_changed();
        self.egress.send_protected_frame(FRAME_PTY_OUTPUT, data);
        let mut shaping_reports = MetadataShapingReportAccumulator::default();
        for observation in observations {
            for shaping in self.metadata_shaper.push(observation) {
                shaping_reports.record(shaping);
            }
        }
        for observation in self.metadata_shaper.drain() {
            send_metadata_observation(&self.egress, observation);
        }
        for shaping in shaping_reports.into_reports() {
            self.egress
                .send_protected_json(FRAME_METADATA_SHAPING, &shaping);
        }
    }

    fn apply_outputs(&mut self, outputs: Vec<SessionRuntimeOutput>) {
        for output in outputs {
            match output {
                SessionRuntimeOutput::PtyOutput { data, .. } => self.apply_pty_output_chunk(data),
                SessionRuntimeOutput::ProcessExited { payload, .. } => {
                    if self.deferring_exit {
                        self.deferred_exit = Some(payload);
                    } else {
                        self.emit_process_exit(&payload);
                    }
                }
                SessionRuntimeOutput::Backpressure(_)
                // The worker's own local runtime never loses a worker.
                | SessionRuntimeOutput::WorkerLost { .. }
                | SessionRuntimeOutput::TitleChanged { .. }
                | SessionRuntimeOutput::CwdChanged { .. }
                | SessionRuntimeOutput::PromptMark { .. }
                | SessionRuntimeOutput::Bell { .. }
                | SessionRuntimeOutput::Notification { .. }
                | SessionRuntimeOutput::MetadataShaping(_)
                // The worker's own Ghostty is the mode source here.
                | SessionRuntimeOutput::ModesChanged { .. } => {}
            }
        }
    }

    /// Emit the exit: pending metadata, failed keyed operations, the final
    /// state, then PROCESS_EXITED, which ends the egress.
    fn emit_process_exit(&mut self, payload: &botster_core::ProcessExitedPayload) {
        for observation in self.metadata_shaper.drain() {
            send_metadata_observation(&self.egress, observation);
        }
        self.fail_pending_on_exit();
        self.send_final_state();
        self.egress
            .send_protected_json(FRAME_PROCESS_EXITED, payload);
        self.exited = true;
    }

    fn send_final_state(&mut self) {
        let screen_text = self.ghostty.plain_text().unwrap_or_default();
        let (snapshot, error) = match self.ghostty.export_snapshot_bytes() {
            Ok(bytes) => (Some(bytes), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mode_bits = self.current_mode_bits();
        let size = self.ghostty.size();
        let color_profile = self.ghostty.read_color_profile().unwrap_or_default();
        let state = WorkerFinalState {
            screen_text,
            has_snapshot: snapshot.is_some(),
            mode_bits,
            rows: size.rows,
            cols: size.cols,
            color_profile,
            error,
        };
        if let Ok(payload) = encode_final_state(&state, snapshot.as_deref()) {
            self.egress.send_protected_frame(FRAME_FINAL_STATE, payload);
        }
    }

    fn drain_and_apply_pty_output(
        &mut self,
        runtime: &mut LocalProcessRuntime,
    ) -> Result<(), String> {
        if self.exited {
            return Ok(());
        }
        let outputs = runtime
            .drain_output(&self.session_id)
            .map_err(|error| error.to_string())?;
        self.apply_outputs(outputs);
        Ok(())
    }

    /// Send one capture's snapshot frames and complete its release
    /// handshake. Under `barrier` the PTY is held; with none, the child has
    /// exited and the final terminal model is served.
    fn serve_snapshot(
        &mut self,
        mut barrier: Option<&mut botster_core::PtyIoBarrier<'_>>,
        request_id: &str,
        barrier_control: &SnapshotBarrierControl,
    ) -> Result<(), botster_core::SessionRuntimeError> {
        let encoded = (|| {
            if let Some(barrier) = barrier.as_mut() {
                let outputs = barrier.drain_output()?;
                self.apply_outputs(outputs);
            }
            let size = self.ghostty.size();
            let color_profile = self.ghostty.read_color_profile().unwrap_or_default();
            let egress = &self.egress;
            self.ghostty
                .export_snapshot_frames(|frame| {
                    let phase = match frame.kind {
                        GhosttySnapshotFrameKind::Ready => WorkerSnapshotPhase::Ready,
                        GhosttySnapshotFrameKind::History => WorkerSnapshotPhase::History,
                        GhosttySnapshotFrameKind::Finish => WorkerSnapshotPhase::Finish,
                    };
                    egress.send_protected_json_cancellable(
                        FRAME_SNAPSHOT,
                        &WorkerSnapshotResult {
                            request_id: request_id.to_owned(),
                            snapshot: Some(botster_core::TerminalSnapshotPayload::new(
                                frame.bytes,
                                size,
                                Some(GHOSTTY_SNAPSHOT_FORMAT.to_owned()),
                            )),
                            phase: Some(phase),
                            error_kind: None,
                            barrier_released: false,
                            color_profile: (frame.kind == GhosttySnapshotFrameKind::Finish)
                                .then(|| color_profile.clone()),
                        },
                        || barrier_control.is_cancelled(request_id),
                    )
                })
                .map_err(|error| {
                    botster_core::SessionRuntimeError::new(
                        botster_core::SessionRuntimeErrorKind::OutputFailed,
                        error.to_string(),
                    )
                })
        })();
        if let Err(error) = encoded {
            let _ = self.egress.send_protected_json_cancellable(
                FRAME_SNAPSHOT,
                &WorkerSnapshotResult {
                    request_id: request_id.to_owned(),
                    snapshot: None,
                    phase: None,
                    error_kind: Some(error.to_string()),
                    barrier_released: false,
                    color_profile: None,
                },
                || barrier_control.is_cancelled(request_id),
            );
        }
        match barrier_control.wait_for_release(request_id) {
            SnapshotBarrierRelease::Cancel => return Ok(()),
            SnapshotBarrierRelease::Complete(resize) => {
                let release_error = if let Some(size) = resize {
                    self.ghostty
                        .resize(TerminalScreenSize::new(size.rows, size.cols));
                    barrier
                        .as_mut()
                        .and_then(|barrier| barrier.resize(size).err())
                        .map(|error| error.to_string())
                } else {
                    None
                };
                let _ = self.egress.send_protected_json(
                    FRAME_SNAPSHOT,
                    &WorkerSnapshotResult {
                        request_id: request_id.to_owned(),
                        snapshot: None,
                        phase: None,
                        error_kind: release_error,
                        barrier_released: true,
                        color_profile: None,
                    },
                );
            }
        }
        Ok(())
    }

    fn handle_snapshot_request(
        &mut self,
        runtime: &mut LocalProcessRuntime,
        snapshot_barrier: &Arc<SnapshotBarrierControl>,
        payload: &[u8],
    ) {
        let request = match serde_json::from_slice::<WorkerSnapshotRequest>(payload) {
            Ok(request) => request,
            Err(error) => {
                self.egress.send_protected_json(
                    FRAME_SNAPSHOT,
                    &WorkerSnapshotResult {
                        request_id: String::new(),
                        snapshot: None,
                        phase: None,
                        error_kind: Some(format!("malformed snapshot request: {error}")),
                        barrier_released: false,
                        color_profile: None,
                    },
                );
                return;
            }
        };
        let request_id = request.request_id;
        let barrier_control = Arc::clone(snapshot_barrier);
        let session_id = self.session_id.clone();
        self.deferring_exit = true;
        let result = if self.exited {
            // The child has exited and its PTY is gone. The final terminal
            // model still answers, with the same release handshake.
            self.serve_snapshot(None, &request_id, &barrier_control)
        } else {
            runtime.with_pty_io_barrier(&session_id, |barrier| {
                self.serve_snapshot(Some(barrier), &request_id, &barrier_control)
            })
        };
        if let Err(error) = result {
            let _ = self.egress.send_protected_json(
                FRAME_SNAPSHOT,
                &WorkerSnapshotResult {
                    request_id: request_id.clone(),
                    snapshot: None,
                    phase: None,
                    error_kind: Some(error.to_string()),
                    barrier_released: false,
                    color_profile: None,
                },
            );
        }
        snapshot_barrier.clear(&request_id);
        self.publish_modes_if_changed();
        self.deferring_exit = false;
        if let Some(payload) = self.deferred_exit.take() {
            self.emit_process_exit(&payload);
        }
    }
}

#[derive(Default)]
struct SnapshotBarrierState {
    active_request: Option<String>,
    staged_resize: Option<ResizePayload>,
    release: Option<SnapshotBarrierRelease>,
}

#[derive(Clone)]
enum SnapshotBarrierRelease {
    Cancel,
    Complete(Option<ResizePayload>),
}

#[derive(Default)]
struct SnapshotBarrierControl {
    state: Mutex<SnapshotBarrierState>,
    wake: Condvar,
    /// Notified after a cancel so a snapshot send blocked on a full egress
    /// lane rechecks its cancellation.
    egress_space: Arc<EgressSpace>,
}

impl SnapshotBarrierControl {
    fn begin(&self, request_id: String) {
        if let Ok(mut state) = self.state.lock() {
            state.active_request = Some(request_id);
            state.staged_resize = None;
            state.release = None;
            self.wake.notify_all();
        }
        // After the state lock is released: a blocked sender holds the egress
        // lock while it checks cancellation under the state lock.
        self.egress_space.notify();
    }

    fn cancel_active(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.active_request.is_some() {
                state.release = Some(SnapshotBarrierRelease::Cancel);
                self.wake.notify_all();
            }
        }
        self.egress_space.notify();
    }

    fn stage_resize(&self, size: ResizePayload) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.active_request.is_none() {
            return false;
        }
        state.staged_resize = Some(size);
        true
    }

    fn request_cancel(&self, request_id: String) {
        if let Ok(mut state) = self.state.lock() {
            if state.active_request.as_deref() == Some(request_id.as_str()) {
                state.release = Some(SnapshotBarrierRelease::Cancel);
                self.wake.notify_all();
            }
        }
        self.egress_space.notify();
    }

    fn request_complete(&self, request_id: String) {
        if let Ok(mut state) = self.state.lock() {
            if state.active_request.as_deref() == Some(request_id.as_str()) {
                let resize = state.staged_resize.take();
                state.release = Some(SnapshotBarrierRelease::Complete(resize));
                self.wake.notify_all();
            }
        }
    }

    fn is_cancelled(&self, request_id: &str) -> bool {
        self.state.lock().map_or(true, |state| {
            state.active_request.as_deref() != Some(request_id)
                || matches!(state.release, Some(SnapshotBarrierRelease::Cancel))
        })
    }

    fn wait_for_release(&self, request_id: &str) -> SnapshotBarrierRelease {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            if state.active_request.as_deref() != Some(request_id) {
                return SnapshotBarrierRelease::Cancel;
            }
            if let Some(release) = state.release.take() {
                return release;
            }
            state = self
                .wake
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    fn clear(&self, request_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            if state.active_request.as_deref() == Some(request_id) {
                *state = SnapshotBarrierState::default();
            }
        }
    }
}

/// The worker outlives its child: after the exit it keeps serving control
/// requests (captures from the final terminal model) until the parent asks
/// it to stop or its control closes.
#[derive(Default)]
enum WorkerLifecycle {
    #[default]
    Running,
    /// Shutdown requested; the child has not exited yet.
    Stopping,
    /// The child exited; control requests are still served.
    Exited,
    /// The child exited and the parent asked to stop, or is gone.
    Done,
}

impl WorkerLifecycle {
    fn request_shutdown(&mut self) {
        *self = match self {
            Self::Running => Self::Stopping,
            Self::Exited | Self::Done => Self::Done,
            Self::Stopping => Self::Stopping,
        };
    }

    fn observe_process_exit(&mut self) {
        *self = match self {
            Self::Running | Self::Exited => Self::Exited,
            Self::Stopping | Self::Done => Self::Done,
        };
    }

    fn should_continue(&self) -> bool {
        !matches!(self, Self::Done)
    }
}

fn send_metadata_observation(egress: &WorkerEgress, observation: TerminalMetadataObservation) {
    match observation {
        TerminalMetadataObservation::TitleChanged(title) => {
            egress.send_metadata_string(FRAME_TITLE_CHANGED, &title, TerminalMetadataKind::Title);
        }
        TerminalMetadataObservation::CwdChanged(cwd) => {
            egress.send_metadata_string(FRAME_CWD_CHANGED, &cwd, TerminalMetadataKind::Cwd);
        }
        TerminalMetadataObservation::PromptMark(payload) => {
            egress.send_metadata_json(
                FRAME_PROMPT_MARK,
                &payload,
                TerminalMetadataKind::PromptMark,
            );
        }
        TerminalMetadataObservation::Bell => {
            egress.send_metadata_frame(FRAME_BELL, Vec::new(), TerminalMetadataKind::Bell);
        }
        TerminalMetadataObservation::Notification(payload) => {
            egress.send_metadata_json(
                FRAME_NOTIFICATION,
                &payload,
                TerminalMetadataKind::Notification,
            );
        }
    }
}

fn spawn_control_reader(
    mut control: Box<dyn ReadWrite + Send>,
    sender: mpsc::Sender<Frame>,
    snapshot_barrier: Arc<SnapshotBarrierControl>,
    wakes: TerminalWakeSource,
    session_id: SessionId,
    connections: Arc<AtomicUsize>,
) {
    connections.fetch_add(1, Ordering::SeqCst);
    thread::spawn(move || {
        while let Ok(frame) = read_frame(&mut control) {
            if frame.frame_type == FRAME_GET_SNAPSHOT {
                if let Ok(request) = serde_json::from_slice::<WorkerSnapshotRequest>(&frame.payload)
                {
                    if request.cancel {
                        snapshot_barrier.request_cancel(request.request_id);
                        continue;
                    }
                    if request.complete {
                        snapshot_barrier.request_complete(request.request_id);
                        continue;
                    }
                    snapshot_barrier.begin(request.request_id);
                }
            }
            if frame.frame_type == FRAME_RESIZE {
                if let Ok(size) = serde_json::from_slice::<ResizePayload>(&frame.payload) {
                    if snapshot_barrier.stage_resize(size) {
                        continue;
                    }
                }
            }
            if frame.frame_type == FRAME_SHUTDOWN {
                // The main loop handles shutdown, but it may be parked in
                // wait_for_release for an active barrier. Release it here so
                // the shutdown frame is acted on; the barrier is over.
                snapshot_barrier.cancel_active();
            }
            if sender.send(frame).is_err() {
                break;
            }
            wakes.notify_session(&session_id);
        }
        snapshot_barrier.cancel_active();
        // Drop the sender before the wake: the main loop reads disconnect as
        // TryRecvError::Disconnected, and a wake that lands while the sender
        // is alive reads as Empty and leaves the loop parked with no wake.
        drop(sender);
        connections.fetch_sub(1, Ordering::SeqCst);
        wakes.notify_session(&session_id);
        #[cfg(test)]
        hold_after_eof_wake(&session_id);
    });
}

/// Test seam: a gate that parks a control reader right after its final wake,
/// so a test can observe what the main loop sees at that wake.
#[cfg(test)]
static EOF_WAKE_GATES: Mutex<Vec<(SessionId, Receiver<()>)>> = Mutex::new(Vec::new());

#[cfg(test)]
fn hold_after_eof_wake(session_id: &SessionId) {
    let gate = {
        let mut gates = EOF_WAKE_GATES.lock().expect("eof wake gates");
        gates
            .iter()
            .position(|(id, _)| id == session_id)
            .map(|index| gates.swap_remove(index).1)
    };
    if let Some(gate) = gate {
        let _ = gate.recv();
    }
}

fn encoded_frame_type(frame: &[u8]) -> Option<u8> {
    frame.get(4).copied()
}

fn is_process_exited_frame(frame: &[u8]) -> bool {
    encoded_frame_type(frame) == Some(FRAME_PROCESS_EXITED)
}

fn drain_metadata_lane(
    metadata: &Receiver<Vec<u8>>,
    mut write_frame: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    while let Ok(frame) = metadata.try_recv() {
        write_frame(&frame)?;
    }
    Ok(())
}

/// Write protected frames, then pending metadata. Before
/// `FRAME_PROCESS_EXITED`, drain metadata first so queued observations still
/// precede it. The exit frame does not end the egress: the worker keeps
/// answering control requests (captures from its final model) after it.
fn write_egress_lanes(
    mut write_frame: impl FnMut(&[u8]) -> Result<(), String>,
    protected: Receiver<Vec<u8>>,
    metadata: Receiver<Vec<u8>>,
    space: &EgressSpace,
) -> Result<(), String> {
    while let Ok(frame) = protected.recv() {
        space.record_taken();
        write_one_protected_frame(&mut write_frame, &metadata, frame)?;
        while let Ok(frame) = protected.try_recv() {
            space.record_taken();
            write_one_protected_frame(&mut write_frame, &metadata, frame)?;
        }
    }
    Ok(())
}

fn write_one_protected_frame(
    write_frame: &mut impl FnMut(&[u8]) -> Result<(), String>,
    metadata: &Receiver<Vec<u8>>,
    frame: Vec<u8>,
) -> Result<(), String> {
    if is_process_exited_frame(&frame) {
        drain_metadata_lane(metadata, |queued| write_frame(queued))?;
        return write_frame(&frame);
    }
    write_frame(&frame)?;
    drain_metadata_lane(metadata, |queued| write_frame(queued))
}

fn write_egress(
    mut stdout: impl Write,
    protected: Receiver<Vec<u8>>,
    metadata: Receiver<Vec<u8>>,
    space: &EgressSpace,
) -> Result<(), String> {
    write_egress_lanes(
        |frame| {
            stdout
                .write_all(frame)
                .and_then(|_| stdout.flush())
                .map_err(|error| error.to_string())
        },
        protected,
        metadata,
        space,
    )
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

struct StdioControl {
    stdin: io::Stdin,
    stdout: io::Stdout,
}

impl Read for StdioControl {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stdin.read(buffer)
    }
}

impl Write for StdioControl {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stdout.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

enum WorkerControl {
    Stdio,
    #[cfg(unix)]
    Socket {
        listener: UnixListener,
        writer: Arc<Mutex<Option<UnixStream>>>,
        _endpoint: WorkerSocketEndpoint,
    },
}

impl WorkerControl {
    fn open(args: &WorkerArgs) -> Result<Self, String> {
        match &args.control_socket {
            Some(path) => {
                #[cfg(unix)]
                {
                    let (listener, endpoint) = bind_worker_socket(path)?;
                    Ok(Self::Socket {
                        listener,
                        writer: Arc::new(Mutex::new(None)),
                        _endpoint: endpoint,
                    })
                }
                #[cfg(not(unix))]
                {
                    let _ = path;
                    Err("control sockets are only supported on unix".to_string())
                }
            }
            None => Ok(Self::Stdio),
        }
    }

    fn accept_initial(&self) -> Result<Box<dyn ReadWrite + Send>, String> {
        match self {
            Self::Stdio => Ok(Box::new(StdioControl {
                stdin: io::stdin(),
                stdout: io::stdout(),
            })),
            #[cfg(unix)]
            Self::Socket {
                listener, writer, ..
            } => {
                let stream = listener.accept().map_err(|error| error.to_string())?.0;
                *writer
                    .lock()
                    .map_err(|_| "writer lock poisoned".to_string())? =
                    Some(stream.try_clone().map_err(|error| error.to_string())?);
                Ok(Box::new(stream))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_readers(
        &self,
        initial: Box<dyn ReadWrite + Send>,
        sender: mpsc::Sender<Frame>,
        snapshot_barrier: Arc<SnapshotBarrierControl>,
        metadata: SessionMetadata,
        wakes: TerminalWakeSource,
        session_id: SessionId,
        connections: Arc<AtomicUsize>,
    ) {
        spawn_control_reader(
            initial,
            sender.clone(),
            Arc::clone(&snapshot_barrier),
            wakes.clone(),
            session_id.clone(),
            Arc::clone(&connections),
        );
        #[cfg(unix)]
        if let Self::Socket {
            listener, writer, ..
        } = self
        {
            let listener = listener.try_clone().expect("clone worker listener");
            let writer = Arc::clone(writer);
            thread::spawn(move || {
                while let Ok((mut stream, _)) = listener.accept() {
                    if read_hello(&mut stream).ok() != Some(PROTOCOL_VERSION) {
                        continue;
                    }
                    if write_welcome(&mut stream, &metadata).is_err() {
                        continue;
                    }
                    if let Ok(clone) = stream.try_clone() {
                        if let Ok(mut slot) = writer.lock() {
                            *slot = Some(clone);
                        }
                    }
                    spawn_control_reader(
                        Box::new(stream),
                        sender.clone(),
                        Arc::clone(&snapshot_barrier),
                        wakes.clone(),
                        session_id.clone(),
                        Arc::clone(&connections),
                    );
                }
            });
        }
    }

    fn spawn_writer(
        &self,
        protected: Receiver<Vec<u8>>,
        metadata: Receiver<Vec<u8>>,
        space: Arc<EgressSpace>,
    ) -> thread::JoinHandle<Result<(), String>> {
        match self {
            Self::Stdio => thread::spawn(move || {
                let _close = CloseEgressOnExit(Arc::clone(&space));
                write_egress(io::stdout(), protected, metadata, &space)
            }),
            #[cfg(unix)]
            Self::Socket { writer, .. } => {
                let writer = Arc::clone(writer);
                thread::spawn(move || {
                    let _close = CloseEgressOnExit(Arc::clone(&space));
                    write_egress_lanes(
                        |frame| {
                            if let Ok(mut slot) = writer.lock() {
                                if let Some(stream) = slot.as_mut() {
                                    if stream
                                        .write_all(frame)
                                        .and_then(|_| stream.flush())
                                        .is_err()
                                    {
                                        *slot = None;
                                    }
                                }
                            }
                            Ok(())
                        },
                        protected,
                        metadata,
                        &space,
                    )
                })
            }
        }
    }

    fn shutdown_on_disconnect(&self) -> bool {
        matches!(self, Self::Stdio)
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SocketIdentity {
    device: u64,
    inode: u64,
    ctime: i64,
    ctime_nsec: i64,
}

#[cfg(unix)]
fn socket_identity(path: &std::path::Path) -> io::Result<SocketIdentity> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "worker control endpoint is not a socket",
        ));
    }
    Ok(SocketIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
    })
}

#[cfg(unix)]
fn remove_socket_if_unchanged(
    path: &std::path::Path,
    expected: &SocketIdentity,
) -> io::Result<bool> {
    let current = match socket_identity(path) {
        Ok(identity) => identity,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if &current != expected {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    Ok(true)
}

#[cfg(unix)]
fn bind_worker_socket(
    path: &std::path::Path,
) -> Result<(UnixListener, WorkerSocketEndpoint), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "worker control socket has no parent directory".to_string())?;
    let parent_existed = match std::fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.is_dir() => true,
        Ok(_) => return Err("worker control socket parent is not a directory".to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "inspect worker control socket parent failed: {error}"
            ))
        }
    };
    if !parent_existed {
        create_private_socket_parent(parent)?;
    }
    let parent_metadata = std::fs::symlink_metadata(parent)
        .map_err(|error| format!("inspect worker control socket parent failed: {error}"))?;
    if !parent_metadata.is_dir() {
        return Err("worker control socket parent is not a directory".to_string());
    }
    if parent_metadata.uid() != effective_user_id() || parent_metadata.mode() & 0o077 != 0 {
        return Err(
            "worker control socket parent must be owned by the effective user with private permissions"
                .to_string(),
        );
    }

    match socket_identity(path) {
        Ok(before) => match UnixStream::connect(path) {
            Ok(_) => return Err("worker control socket is already active".to_string()),
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                if !remove_socket_if_unchanged(path, &before).map_err(|error| error.to_string())? {
                    return Err("worker control socket changed during stale cleanup".to_string());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("probe worker control socket failed: {error}")),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        let parent_state = std::fs::symlink_metadata(parent)
            .map(|metadata| {
                format!(
                    "present uid={} mode={:o}",
                    metadata.uid(),
                    metadata.mode() & 0o777
                )
            })
            .unwrap_or_else(|parent_error| format!("unavailable: {parent_error}"));
        format!(
            "bind worker control socket {:?} failed: {error}; parent is {parent_state}",
            path
        )
    })?;
    let identity = socket_identity(path)
        .map_err(|error| format!("inspect bound worker control socket failed: {error}"))?;
    Ok((
        listener,
        WorkerSocketEndpoint {
            path: path.to_path_buf(),
            identity,
        },
    ))
}

#[cfg(unix)]
fn create_private_socket_parent(parent: &std::path::Path) -> Result<(), String> {
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(0o700);
    match builder.create(parent) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(format!(
            "create worker control socket parent failed: {error}"
        )),
    }
}

#[cfg(unix)]
fn effective_user_id() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }

    // SAFETY: geteuid takes no arguments and returns the effective POSIX user id.
    unsafe { geteuid() }
}

#[cfg(unix)]
#[derive(Debug)]
struct WorkerSocketEndpoint {
    path: PathBuf,
    identity: SocketIdentity,
}

#[cfg(unix)]
impl Drop for WorkerSocketEndpoint {
    fn drop(&mut self) {
        let _ = remove_socket_if_unchanged(&self.path, &self.identity);
    }
}

struct WorkerEgress {
    protected_sender: SyncSender<Vec<u8>>,
    metadata_sender: SyncSender<Vec<u8>>,
    space: Arc<EgressSpace>,
}

/// Protected egress frames the writer has taken off its lane. A sender
/// blocked on a full lane waits here for the writer, a cancel, or the writer
/// exit.
#[derive(Default)]
struct EgressSpace {
    state: Mutex<EgressSpaceState>,
    changed: Condvar,
}

#[derive(Default)]
struct EgressSpaceState {
    taken: u64,
    /// The writer thread exited; nothing will take another frame.
    closed: bool,
}

/// Closes the egress space when the writer thread ends, a panic included.
struct CloseEgressOnExit(Arc<EgressSpace>);

impl Drop for CloseEgressOnExit {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl EgressSpace {
    fn lock(&self) -> std::sync::MutexGuard<'_, EgressSpaceState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn taken(&self) -> u64 {
        self.lock().taken
    }

    fn is_closed(&self) -> bool {
        self.lock().closed
    }

    fn record_taken(&self) {
        self.lock().taken += 1;
        self.changed.notify_all();
    }

    /// The writer exited. Wake blocked senders so they give up.
    fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }

    /// Wake blocked senders so they recheck cancellation.
    fn notify(&self) {
        let _state = self.lock();
        self.changed.notify_all();
    }

    /// Wait until the writer takes a frame after `seen`, the writer exits,
    /// or `cancelled`.
    fn wait_after(&self, seen: u64, cancelled: &mut impl FnMut() -> bool) {
        let mut state = self.lock();
        while state.taken == seen && !state.closed && !cancelled() {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

#[derive(Default)]
struct MetadataShapingReportAccumulator {
    counts: HashMap<(Option<TerminalMetadataKind>, TerminalMetadataShapingOutcome), usize>,
}

impl MetadataShapingReportAccumulator {
    fn record(&mut self, observation: TerminalMetadataShapingObservation) {
        *self
            .counts
            .entry((observation.kind, observation.outcome))
            .or_insert(0) += observation.count;
    }

    fn into_reports(self) -> Vec<TerminalMetadataShapingObservation> {
        self.counts
            .into_iter()
            .map(
                |((kind, outcome), count)| TerminalMetadataShapingObservation {
                    kind,
                    outcome,
                    count,
                },
            )
            .collect()
    }
}

impl WorkerEgress {
    fn new(capacity: usize) -> (Self, Receiver<Vec<u8>>, Receiver<Vec<u8>>) {
        let (protected_sender, protected_receiver) = mpsc::sync_channel(capacity.max(1));
        let (metadata_sender, metadata_receiver) = mpsc::sync_channel(capacity.max(1));
        (
            Self {
                protected_sender,
                metadata_sender,
                space: Arc::new(EgressSpace::default()),
            },
            protected_receiver,
            metadata_receiver,
        )
    }

    fn send_protected_frame(&self, frame_type: u8, payload: Vec<u8>) -> bool {
        if let Ok(frame) = botster_core::encode_frame(frame_type, &payload) {
            return self.protected_sender.send(frame).is_ok();
        }
        false
    }

    fn send_protected_json<T: serde::Serialize>(&self, frame_type: u8, payload: &T) -> bool {
        if let Ok(frame) = botster_core::encode_json(frame_type, payload) {
            return self.protected_sender.send(frame).is_ok();
        }
        false
    }

    fn send_protected_json_cancellable<T, F>(
        &self,
        frame_type: u8,
        payload: &T,
        mut cancelled: F,
    ) -> bool
    where
        T: serde::Serialize,
        F: FnMut() -> bool,
    {
        let Ok(mut frame) = botster_core::encode_json(frame_type, payload) else {
            return false;
        };
        loop {
            if cancelled() || self.space.is_closed() {
                return false;
            }
            let seen = self.space.taken();
            match self.protected_sender.try_send(frame) {
                Ok(()) => return true,
                Err(TrySendError::Full(returned)) => {
                    frame = returned;
                    self.space.wait_after(seen, &mut cancelled);
                }
                Err(TrySendError::Disconnected(_)) => return false,
            }
        }
    }

    fn send_metadata_frame(&self, frame_type: u8, payload: Vec<u8>, kind: TerminalMetadataKind) {
        if let Ok(frame) = botster_core::encode_frame(frame_type, &payload) {
            self.try_send_metadata(frame, kind);
        }
    }

    fn send_metadata_json<T: serde::Serialize>(
        &self,
        frame_type: u8,
        payload: &T,
        kind: TerminalMetadataKind,
    ) {
        if let Ok(frame) = botster_core::encode_json(frame_type, payload) {
            self.try_send_metadata(frame, kind);
        }
    }

    fn send_metadata_string(&self, frame_type: u8, payload: &str, kind: TerminalMetadataKind) {
        if let Ok(frame) = botster_core::encode_string(frame_type, payload) {
            self.try_send_metadata(frame, kind);
        }
    }

    fn try_send_metadata(&self, frame: Vec<u8>, kind: TerminalMetadataKind) {
        match self.metadata_sender.try_send(frame) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.send_protected_json(
                    FRAME_METADATA_SHAPING,
                    &TerminalMetadataShapingObservation {
                        kind: Some(kind),
                        outcome: TerminalMetadataShapingOutcome::Dropped,
                        count: 1,
                    },
                );
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

fn read_frame(stream: &mut impl Read) -> Result<Frame, String> {
    let mut len_buf = [0; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|error| error.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > botster_core::MAX_FRAME_LEN {
        return Err("invalid frame length".to_string());
    }
    let mut body = vec![0; len];
    stream
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(Frame {
        frame_type: body[0],
        payload: body[1..].to_vec(),
    })
}

fn write_worker_startup_failure(
    stream: &mut impl Write,
    request_id: &str,
    session_id: &str,
    reservation: &SessionReservation,
    error: &ReservedSessionSpawnError,
) -> Result<(), String> {
    let outcome = if reservation.startup_created_child() {
        StartupFailureOutcome::Created {
            child_pid: None,
            process_group_id: None,
        }
    } else {
        StartupFailureOutcome::NotCreated
    };
    write_created_or_not(stream, request_id, session_id, outcome, &error.to_string())
}

fn write_created_startup_failure(
    stream: &mut impl Write,
    request_id: &str,
    session_id: &str,
    child_pid: Option<u32>,
    process_group_id: Option<i32>,
    message: &str,
) -> Result<(), String> {
    write_created_or_not(
        stream,
        request_id,
        session_id,
        StartupFailureOutcome::Created {
            child_pid,
            process_group_id,
        },
        message,
    )
}

fn write_created_or_not(
    stream: &mut impl Write,
    request_id: &str,
    session_id: &str,
    outcome: StartupFailureOutcome,
    message: &str,
) -> Result<(), String> {
    write_startup_failure(
        stream,
        &StartupFailureReport {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            worker_pid: process::id(),
            message: message.to_string(),
            outcome,
        },
    )
    .map_err(|error| error.to_string())
}

struct WorkerArgs {
    egress_capacity: usize,
    pty_reader_chunk_capacity: usize,
    shutdown_grace_ms: u64,
    control_socket: Option<PathBuf>,
    test_fail_after_spawn: bool,
    test_hold_after_read_ms: Option<u64>,
    test_pending_capacity: Option<usize>,
    test_hold_after_enqueue_ms: Option<u64>,
    test_hold_before_exit_ms: Option<u64>,
    test_exit_code: Option<i32>,
    ghostty_max_scrollback_bytes: usize,
    terminal_color_profile: Option<botster_core::TerminalColorProfile>,
}

impl WorkerArgs {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut egress_capacity = 64;
        let mut pty_reader_chunk_capacity = botster_core::DEFAULT_PTY_READER_CHUNK_CAPACITY;
        let mut shutdown_grace_ms = 500;
        let mut control_socket = None;
        let mut test_fail_after_spawn = false;
        let mut test_hold_after_read_ms = None;
        let mut test_pending_capacity = None;
        let mut test_hold_after_enqueue_ms = None;
        let mut test_hold_before_exit_ms = None;
        let mut test_exit_code = None;
        let mut ghostty_max_scrollback_bytes = 10_000_000;
        let mut terminal_color_profile = None;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--egress-capacity" => {
                    index += 1;
                    egress_capacity = parse_arg(&args, index, "--egress-capacity")?;
                }
                "--pty-reader-capacity" => {
                    index += 1;
                    pty_reader_chunk_capacity = parse_arg(&args, index, "--pty-reader-capacity")?;
                }
                "--shutdown-grace-ms" => {
                    index += 1;
                    shutdown_grace_ms = parse_arg(&args, index, "--shutdown-grace-ms")?;
                }
                "--control-socket" => {
                    index += 1;
                    control_socket = Some(PathBuf::from(parse_string_arg(
                        &args,
                        index,
                        "--control-socket",
                    )?));
                }
                "--test-fail-after-spawn" => {
                    test_fail_after_spawn = true;
                }
                "--test-hold-after-read-ms" => {
                    index += 1;
                    test_hold_after_read_ms =
                        Some(parse_arg(&args, index, "--test-hold-after-read-ms")?);
                }
                "--test-pending-capacity" => {
                    index += 1;
                    test_pending_capacity =
                        Some(parse_arg(&args, index, "--test-pending-capacity")?);
                }
                "--test-hold-after-enqueue-ms" => {
                    index += 1;
                    test_hold_after_enqueue_ms =
                        Some(parse_arg(&args, index, "--test-hold-after-enqueue-ms")?);
                }
                "--test-hold-before-exit-ms" => {
                    index += 1;
                    test_hold_before_exit_ms =
                        Some(parse_arg(&args, index, "--test-hold-before-exit-ms")?);
                }
                "--test-exit-code" => {
                    index += 1;
                    test_exit_code = Some(parse_arg(&args, index, "--test-exit-code")?);
                }
                "--ghostty-max-scrollback-bytes" => {
                    index += 1;
                    ghostty_max_scrollback_bytes =
                        parse_arg(&args, index, "--ghostty-max-scrollback-bytes")?;
                }
                "--terminal-color-profile" => {
                    index += 1;
                    terminal_color_profile = Some(
                        serde_json::from_str(&parse_string_arg(
                            &args,
                            index,
                            "--terminal-color-profile",
                        )?)
                        .map_err(|error| error.to_string())?,
                    );
                }
                other => return Err(format!("unknown worker argument: {other}")),
            }
            index += 1;
        }

        Ok(Self {
            egress_capacity,
            pty_reader_chunk_capacity,
            shutdown_grace_ms,
            control_socket,
            test_fail_after_spawn,
            test_hold_after_read_ms,
            test_pending_capacity,
            test_hold_after_enqueue_ms,
            test_hold_before_exit_ms,
            test_exit_code,
            ghostty_max_scrollback_bytes,
            terminal_color_profile,
        })
    }
}

fn parse_string_arg(args: &[String], index: usize, name: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_arg<T>(args: &[String], index: usize, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    args.get(index)
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse()
        .map_err(|error| format!("parse {name}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{SnapshotBarrierControl, SnapshotBarrierRelease, WorkerLifecycle};

    fn decode_frame_types(bytes: &[u8]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(bytes);
        let mut types = Vec::new();
        while cursor.position() < bytes.len() as u64 {
            types.push(
                super::read_frame(&mut cursor)
                    .expect("decode test frame")
                    .frame_type,
            );
        }
        types
    }

    fn process_exited_frame() -> Vec<u8> {
        botster_core::encode_json(
            super::FRAME_PROCESS_EXITED,
            &botster_core::ProcessExitedPayload {
                exit_code: Some(0),
                signal: None,
            },
        )
        .expect("encode process-exited test frame")
    }

    #[test]
    fn stdio_writer_emits_queued_metadata_before_process_exited_and_nothing_after() {
        let (protected_tx, protected_rx) = std::sync::mpsc::sync_channel(8);
        let (metadata_tx, metadata_rx) = std::sync::mpsc::sync_channel(8);
        let title =
            botster_core::encode_string(super::FRAME_TITLE_CHANGED, "late-title").expect("title");
        metadata_tx.send(title).expect("queue metadata");
        protected_tx
            .send(process_exited_frame())
            .expect("queue process-exited");
        drop(protected_tx);
        drop(metadata_tx);

        let mut stdout = Vec::new();
        super::write_egress(
            &mut stdout,
            protected_rx,
            metadata_rx,
            &super::EgressSpace::default(),
        )
        .expect("stdio writer");
        assert_eq!(
            decode_frame_types(&stdout),
            vec![super::FRAME_TITLE_CHANGED, super::FRAME_PROCESS_EXITED]
        );
    }

    /// Metadata queued before the exit still precedes it, and the exit no
    /// longer ends the egress: a capture answered from the final model after
    /// the exit is still written.
    #[test]
    fn writer_emits_queued_metadata_then_process_exited_and_serves_later_frames() {
        let (protected_tx, protected_rx) = std::sync::mpsc::sync_channel(8);
        let (metadata_tx, metadata_rx) = std::sync::mpsc::sync_channel(8);
        let late_snapshot =
            botster_core::encode_frame(super::FRAME_SNAPSHOT, b"after-exit").expect("snapshot");
        let late_title =
            botster_core::encode_string(super::FRAME_TITLE_CHANGED, "after-exit-title")
                .expect("title");
        protected_tx
            .send(process_exited_frame())
            .expect("queue process-exited");
        protected_tx
            .send(late_snapshot)
            .expect("queue late snapshot");
        metadata_tx.send(late_title).expect("queue late title");
        drop(protected_tx);
        drop(metadata_tx);

        let mut stdout = Vec::new();
        super::write_egress(
            &mut stdout,
            protected_rx,
            metadata_rx,
            &super::EgressSpace::default(),
        )
        .expect("stdio writer");
        assert_eq!(
            decode_frame_types(&stdout),
            vec![
                super::FRAME_TITLE_CHANGED,
                super::FRAME_PROCESS_EXITED,
                super::FRAME_SNAPSHOT
            ]
        );
    }

    #[test]
    fn a_snapshot_send_blocked_on_a_full_lane_ends_when_the_writer_exits() {
        let (egress, protected_rx, _metadata_rx) = super::WorkerEgress::new(1);
        assert!(egress.send_protected_frame(super::FRAME_PTY_OUTPUT, b"fills the lane".to_vec()));
        let space = std::sync::Arc::clone(&egress.space);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let sender = std::thread::spawn(move || {
            let sent =
                egress
                    .send_protected_json_cancellable(super::FRAME_PTY_OUTPUT, &"blocked", || false);
            let _ = done_tx.send(sent);
        });
        // The writer exits without taking the frame.
        drop(protected_rx);
        drop(super::CloseEgressOnExit(space));
        // timer: deadline — the writer exit must release the blocked sender; expiry fails the test
        let sent = done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("blocked sender released by the writer exit");
        assert!(!sent);
        sender.join().expect("sender thread");
    }

    #[test]
    fn socket_writer_path_emits_metadata_before_process_exited() {
        let (protected_tx, protected_rx) = std::sync::mpsc::sync_channel(8);
        let (metadata_tx, metadata_rx) = std::sync::mpsc::sync_channel(8);
        let title =
            botster_core::encode_string(super::FRAME_TITLE_CHANGED, "socket-title").expect("title");
        let pty = botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"pty").expect("pty");
        protected_tx.send(pty).expect("queue pty");
        metadata_tx.send(title).expect("queue metadata");
        protected_tx
            .send(process_exited_frame())
            .expect("queue process-exited");
        drop(protected_tx);
        drop(metadata_tx);

        let mut written = Vec::new();
        super::write_egress_lanes(
            |frame| {
                written.extend_from_slice(frame);
                Ok(())
            },
            protected_rx,
            metadata_rx,
            &super::EgressSpace::default(),
        )
        .expect("socket-style writer");
        assert_eq!(
            decode_frame_types(&written),
            vec![
                super::FRAME_PTY_OUTPUT,
                super::FRAME_TITLE_CHANGED,
                super::FRAME_PROCESS_EXITED
            ]
        );
    }

    #[test]
    fn input_result_frames_carry_the_full_terminal_body_the_parent_decodes() {
        let result = botster_terminal_protocol::InputResultBody {
            operation_id: 1,
            outcome: botster_terminal_protocol::InputOutcome::Written,
            accepted_payload_bytes: Some(4),
            written_pty_bytes: Some(4),
            mode_bits: 2,
            detail: String::new(),
        };
        // The same producer helper the worker uses for every result.
        let payload = super::input_result_payload(7, &result).expect("payload");

        let (key, body) = botster_core::split_worker_operation_key(&payload).expect("key");
        let decoded = botster_terminal_protocol::TerminalFrame::from_bytes(body)
            .expect("parent validates the TerminalBody header");
        assert_eq!(key, 7);
        assert_eq!(
            botster_terminal_protocol::decode_input_result(&decoded).expect("result"),
            result
        );
    }

    #[test]
    fn modes_changed_frames_carry_the_full_terminal_body_the_parent_decodes() {
        let modes = botster_terminal_protocol::ModesBody {
            mode_bits: 5,
            rows: 30,
            cols: 100,
        };
        // The same producer helper the worker uses on every mode change.
        let payload = super::modes_changed_payload(modes).expect("payload");

        let decoded = botster_terminal_protocol::TerminalFrame::from_bytes(&payload)
            .expect("parent validates the TerminalBody header");
        assert_eq!(
            botster_terminal_protocol::decode_modes(&decoded).expect("modes"),
            modes
        );
    }

    /// Wait for a barrier waiter with a bound so a release regression fails
    /// instead of hanging the test.
    fn release_within(
        control: &Arc<SnapshotBarrierControl>,
        request_id: &'static str,
        timeout: std::time::Duration,
    ) -> std::sync::mpsc::Receiver<SnapshotBarrierRelease> {
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = Arc::clone(control);
        std::thread::spawn(move || {
            let _ = tx.send(waiter.wait_for_release(request_id));
        });
        let _ = timeout;
        rx
    }

    #[test]
    fn shutdown_keeps_worker_loop_alive_until_process_exit_is_observed() {
        let mut lifecycle = WorkerLifecycle::default();

        lifecycle.request_shutdown();
        assert!(lifecycle.should_continue());

        lifecycle.observe_process_exit();
        assert!(!lifecycle.should_continue());
    }

    #[test]
    fn a_shutdown_frame_on_the_control_reader_releases_an_active_barrier() {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let control = Arc::new(SnapshotBarrierControl::default());
        control.begin("snapshot-shutdown".to_string());
        let released = release_within(
            &control,
            "snapshot-shutdown",
            std::time::Duration::from_secs(5),
        );
        let (frames_tx, frames_rx) = std::sync::mpsc::channel();
        let (mut parent, worker) = UnixStream::pair().expect("socket pair");
        super::spawn_control_reader(
            Box::new(worker),
            frames_tx,
            Arc::clone(&control),
            super::TerminalWakeSource::new(),
            super::SessionId("shutdown-reader".to_string()),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        );

        let shutdown =
            botster_core::encode_frame(super::FRAME_SHUTDOWN, &[]).expect("shutdown frame");
        parent.write_all(&shutdown).expect("write shutdown");

        assert!(matches!(
            released
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("barrier released within the bound"),
            SnapshotBarrierRelease::Cancel
        ));
        let forwarded = frames_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("shutdown still reaches the main loop");
        assert_eq!(forwarded.frame_type, super::FRAME_SHUTDOWN);
        drop(parent);
    }

    #[test]
    fn control_eof_wake_finds_the_frame_channel_disconnected() {
        use std::os::unix::net::UnixStream;

        let session = super::SessionId("eof-wake-reader".to_string());
        let (release, gate) = std::sync::mpsc::channel::<()>();
        super::EOF_WAKE_GATES
            .lock()
            .expect("eof wake gates")
            .push((session.clone(), gate));
        let wakes = super::TerminalWakeSource::new();
        let _registered = wakes.session_handle(session.clone());
        let (frames_tx, frames_rx) = std::sync::mpsc::channel();
        let (parent, worker) = UnixStream::pair().expect("socket pair");
        super::spawn_control_reader(
            Box::new(worker),
            frames_tx,
            Arc::new(SnapshotBarrierControl::default()),
            wakes.clone(),
            session,
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        );
        drop(parent);

        // timer: deadline — bounds the wait for the reader's EOF wake.
        let batch = wakes.wait_wakes(std::time::Duration::from_secs(5));
        assert_eq!(
            batch.ingress_sessions,
            vec![super::SessionId("eof-wake-reader".to_string())],
            "the reader posts a wake on control EOF"
        );
        // The reader is parked at the gate after its wake. The main loop
        // shuts down only on Disconnected, so Empty here would park it forever.
        let observed = frames_rx.try_recv().map(|_| ());
        drop(release);
        assert_eq!(observed, Err(super::TryRecvError::Disconnected));
    }

    #[test]
    fn a_truncated_frame_followed_by_link_shutdown_releases_the_barrier() {
        use std::io::Write;
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;

        let control = Arc::new(SnapshotBarrierControl::default());
        let (frames_tx, frames_rx) = std::sync::mpsc::channel();
        let (mut parent, worker) = UnixStream::pair().expect("socket pair");
        super::spawn_control_reader(
            Box::new(worker),
            frames_tx,
            Arc::clone(&control),
            super::TerminalWakeSource::new(),
            super::SessionId("truncated-reader".to_string()),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        );

        // The reader itself begins the barrier from a real begin frame.
        let begin = serde_json::to_vec(&super::WorkerSnapshotRequest {
            request_id: "snapshot-truncated".to_string(),
            cancel: false,
            complete: false,
        })
        .expect("begin json");
        let begin = botster_core::encode_frame(super::FRAME_GET_SNAPSHOT, &begin).expect("frame");
        parent.write_all(&begin).expect("write begin");
        let forwarded = frames_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("begin frame reaches the main loop");
        assert_eq!(forwarded.frame_type, super::FRAME_GET_SNAPSHOT);
        let released = release_within(
            &control,
            "snapshot-truncated",
            std::time::Duration::from_secs(5),
        );

        // The parent's writer dies mid-frame, then the parent shuts the link
        // as fail_control_plane does. The half frame must not park the reader.
        let ping = botster_core::encode_frame(super::FRAME_PING, &[0u8; 64]).expect("frame");
        parent
            .write_all(&ping[..ping.len() / 2])
            .expect("write half a frame");
        parent.shutdown(Shutdown::Write).expect("shut write half");

        // The release is one-shot: the parked waiter consumed it here. A page
        // loop observes the same cancel through is_cancelled only while no
        // waiter is parked, so it is not re-checked after this take.
        assert!(matches!(
            released
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("barrier released within the bound"),
            SnapshotBarrierRelease::Cancel
        ));
        drop(parent);
    }

    #[test]
    fn control_eof_releases_an_active_snapshot_barrier() {
        let control = Arc::new(SnapshotBarrierControl::default());
        control.begin("snapshot-eof".to_string());
        let released = release_within(&control, "snapshot-eof", std::time::Duration::from_secs(5));

        control.cancel_active();

        assert!(matches!(
            released
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("barrier released within the bound"),
            SnapshotBarrierRelease::Cancel
        ));
    }

    #[cfg(unix)]
    mod unix_socket {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::time::{SystemTime, UNIX_EPOCH};

        use super::super::{bind_worker_socket, remove_socket_if_unchanged, socket_identity};

        fn temp_path(label: &str) -> std::path::PathBuf {
            std::env::temp_dir().join(format!(
                "bsw-{label}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock")
                    .as_nanos()
            ))
        }

        fn create_private_root(root: &std::path::Path) {
            std::fs::create_dir_all(root).expect("create root");
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
                .expect("make root private");
        }

        #[test]
        fn bind_creates_a_missing_private_parent() {
            let root = temp_path("missing-parent");
            let path = root.join("worker.sock");

            let (_listener, endpoint) =
                bind_worker_socket(&path).expect("create private parent and bind");

            let metadata = std::fs::symlink_metadata(&root).expect("root metadata");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
            drop(endpoint);
            let _ = std::fs::remove_dir(root);
        }

        #[test]
        fn bind_refuses_a_connectable_socket_without_unlinking_it() {
            let root = temp_path("live");
            create_private_root(&root);
            let path = root.join("worker.sock");
            let listener = UnixListener::bind(&path).expect("bind live socket");
            let identity = socket_identity(&path).expect("live identity");

            let error = bind_worker_socket(&path).expect_err("live socket must be preserved");

            assert!(error.contains("already active"));
            assert_eq!(
                socket_identity(&path).expect("preserved identity"),
                identity
            );
            drop(listener);
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir(root);
        }

        #[test]
        fn bind_reclaims_a_refused_stale_socket() {
            let root = temp_path("stale");
            create_private_root(&root);
            let path = root.join("worker.sock");
            let stale = UnixListener::bind(&path).expect("bind stale socket");
            let stale_identity = socket_identity(&path).expect("stale identity");
            drop(stale);

            let (_listener, endpoint) =
                bind_worker_socket(&path).expect("reclaim stale socket and bind");

            assert_ne!(
                socket_identity(&path).expect("replacement identity"),
                stale_identity
            );
            drop(endpoint);
            let _ = std::fs::remove_dir(root);
        }

        #[test]
        fn cleanup_preserves_a_replaced_socket_object() {
            let root = temp_path("changed");
            create_private_root(&root);
            let path = root.join("worker.sock");
            let first = UnixListener::bind(&path).expect("bind first socket");
            let first_identity = socket_identity(&path).expect("first identity");
            drop(first);
            std::fs::remove_file(&path).expect("remove first socket");
            let replacement = UnixListener::bind(&path).expect("bind replacement socket");

            assert!(
                !remove_socket_if_unchanged(&path, &first_identity).expect("changed socket check")
            );
            assert!(path.exists());
            drop(replacement);
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir(root);
        }

        #[test]
        fn cleanup_identity_includes_socket_lifetime_metadata() {
            let root = temp_path("identity-lifetime");
            create_private_root(&root);
            let path = root.join("worker.sock");
            let listener = UnixListener::bind(&path).expect("bind socket");
            let mut earlier_lifetime = socket_identity(&path).expect("socket identity");
            earlier_lifetime.ctime_nsec ^= 1;

            assert!(!remove_socket_if_unchanged(&path, &earlier_lifetime)
                .expect("mismatched lifetime must be preserved"));
            assert!(path.exists());

            drop(listener);
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir(root);
        }

        #[test]
        fn bind_preserves_a_non_socket_entry() {
            let root = temp_path("file");
            create_private_root(&root);
            let path = root.join("worker.sock");
            let mut file = std::fs::File::create(&path).expect("create non-socket");
            writeln!(file, "keep").expect("write non-socket");

            bind_worker_socket(&path).expect_err("non-socket must fail");

            assert_eq!(
                std::fs::read_to_string(&path).expect("read preserved file"),
                "keep\n"
            );
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir(root);
        }

        #[test]
        fn bind_refuses_an_existing_non_private_parent() {
            let root = temp_path("public-parent");
            std::fs::create_dir_all(&root).expect("create root");
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))
                .expect("make root public");
            let path = root.join("worker.sock");

            let error = bind_worker_socket(&path).expect_err("public parent must be rejected");

            assert!(error.contains("owned by the effective user with private permissions"));
            assert!(!path.exists());
            let _ = std::fs::remove_dir(root);
        }
    }
}
