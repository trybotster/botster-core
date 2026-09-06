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
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use botster_core::contract::terminal_screen::{TerminalKeyEvent, TerminalMouseEvent};
use botster_core::contract::terminal_wake::TerminalWakeSource;
use botster_core::engine::TerminalScreenRuntime;
use botster_core::{
    decode_worker_input_operation, encode_final_state, read_hello, write_welcome, Frame,
    LocalProcessRuntime, LocalProcessRuntimeOptions, ModeFlags, ModeFlagsPayload, ResizePayload,
    ScreenPayload, SessionId, SessionMetadata, SessionRuntime, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, TerminalMetadataKind, TerminalMetadataLaneShaper,
    TerminalMetadataObservation, TerminalMetadataProducer, TerminalMetadataShapingObservation,
    TerminalMetadataShapingOutcome, TerminalScreenSize, TimeoutPayload, WorkerFinalState,
    WorkerHealth, WorkerInputKind, WorkerProbeRequest, WorkerSnapshotPhase, WorkerSnapshotRequest,
    WorkerSnapshotResult, FRAME_BELL, FRAME_CWD_CHANGED, FRAME_FINAL_STATE, FRAME_GET_MODE_FLAGS,
    FRAME_GET_SCREEN, FRAME_GET_SNAPSHOT, FRAME_INPUT_CANCEL, FRAME_INPUT_OPERATION,
    FRAME_INPUT_RESULT, FRAME_METADATA_SHAPING, FRAME_MODES_CHANGED, FRAME_MODE_FLAGS,
    FRAME_NOTIFICATION, FRAME_PING, FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK,
    FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE, FRAME_RESIZE_APPLIED, FRAME_SCREEN,
    FRAME_SET_TIMEOUT, FRAME_SHUTDOWN, FRAME_SNAPSHOT, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED,
    PROTOCOL_VERSION,
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

/// Longest idle wait when no PTY write is pending. Wakes end it early.
const IDLE_WAIT: Duration = Duration::from_secs(5);
/// Retry interval while the PTY would block on a pending write.
const BLOCKED_WRITE_WAIT: Duration = Duration::from_millis(2);

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
    let spawn_request: SessionSpawnRequest =
        serde_json::from_slice(&spawn_frame.payload).map_err(|error| error.to_string())?;
    let session_id = spawn_request.session_id.clone();

    let wakes = TerminalWakeSource::new();
    let runtime_options = LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(args.shutdown_grace_ms),
        poll_interval: Duration::from_millis(args.poll_interval_ms),
        pty_reader_chunk_capacity: args.pty_reader_chunk_capacity,
        test_hold_after_read_ms: args.test_hold_after_read_ms,
        test_write_block_until_unix_ms: args.test_write_block_until_unix_ms,
        test_write_max_chunk: args.test_write_max_chunk,
        test_pending_capacity: args.test_pending_capacity,
        test_hold_after_enqueue_ms: args.test_hold_after_enqueue_ms,
    };
    let mut runtime =
        LocalProcessRuntime::with_options(runtime_options).with_wake_source(wakes.clone());
    let initial_size = spawn_request
        .initial_pty_size
        .clone()
        .unwrap_or(ResizePayload { rows: 24, cols: 80 });
    let handle = runtime
        .spawn_session(spawn_request)
        .map_err(|error| error.to_string())?;
    let initial_rows = initial_size.rows;
    let initial_cols = initial_size.cols;
    let mut ghostty = GhosttyTerminal::with_config(
        TerminalScreenSize::new(initial_rows, initial_cols),
        GhosttyAdapterConfig::with_max_scrollback_bytes(args.ghostty_max_scrollback_bytes),
    )
    .map_err(|error| format!("worker Ghostty init failed: {error}"))?;
    if let Some(profile) = args.terminal_color_profile.as_ref() {
        ghostty
            .apply_color_profile(profile)
            .map_err(|error| format!("worker Ghostty color profile failed: {error}"))?;
    }
    let initial_flags = ghostty
        .read_mode_flags()
        .map_err(|error| format!("worker initial mode flags failed: {error}"))?;
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
            "worker_control_socket": args.control_socket,
            "atomic_snapshot_boundary": true,
            "snapshot_delivery": "ready_then_history",
            "terminal_stream_scheme": 2,
        })),
    };
    write_welcome(&mut initial_control, &metadata).map_err(|error| error.to_string())?;

    let (frame_sender, frame_receiver) = mpsc::channel();
    let snapshot_barrier = Arc::new(SnapshotBarrierControl::default());
    control.spawn_readers(
        initial_control,
        frame_sender,
        Arc::clone(&snapshot_barrier),
        metadata.clone(),
        wakes.clone(),
        session_id.clone(),
    );

    let (egress, protected_receiver, metadata_receiver) =
        WorkerEgress::new(args.egress_capacity.max(1));
    let writer = control.spawn_writer(protected_receiver, metadata_receiver);
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
                        state.apply_resize(&mut runtime, size.rows, size.cols, None)?;
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
                        runtime
                            .send_input(SessionRuntimeInput::Shutdown {
                                session_id: state.session_id.clone(),
                            })
                            .map_err(|error| error.to_string())?;
                        lifecycle.request_shutdown();
                    }
                    _ => {}
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if shutdown_on_disconnect {
                        runtime
                            .send_input(SessionRuntimeInput::Shutdown {
                                session_id: state.session_id.clone(),
                            })
                            .map_err(|error| error.to_string())?;
                        lifecycle.request_shutdown();
                    }
                    break;
                }
            }
        }

        state.drain_and_apply_pty_output(&mut runtime)?;
        if state.exited {
            lifecycle.observe_process_exit();
            break;
        }
        let write_blocked = state.progress_pending_writes(&runtime);
        let timeout = if write_blocked {
            BLOCKED_WRITE_WAIT
        } else {
            IDLE_WAIT
        };
        let _ = wakes.wait_wakes(timeout);
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
        match encode_input_result(result) {
            Ok(frame) => {
                let payload = botster_core::encode_worker_operation(key, frame.body());
                self.egress
                    .send_protected_frame(FRAME_INPUT_RESULT, payload);
            }
            Err(_) => {
                // A body that cannot encode is a programming error in this
                // process; the parent reports OutcomeUnknown on link loss.
            }
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
            if let Ok(frame) = encode_modes(modes) {
                self.egress
                    .send_protected_frame(FRAME_MODES_CHANGED, frame.body().to_vec());
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
                    for observation in self.metadata_shaper.drain() {
                        send_metadata_observation(&self.egress, observation);
                    }
                    self.fail_pending_on_exit();
                    self.send_final_state();
                    self.egress
                        .send_protected_json(FRAME_PROCESS_EXITED, &payload);
                    self.exited = true;
                }
                SessionRuntimeOutput::Backpressure(_)
                | SessionRuntimeOutput::TitleChanged { .. }
                | SessionRuntimeOutput::CwdChanged { .. }
                | SessionRuntimeOutput::PromptMark { .. }
                | SessionRuntimeOutput::Bell { .. }
                | SessionRuntimeOutput::Notification { .. }
                | SessionRuntimeOutput::MetadataShaping(_) => {}
            }
        }
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
        let result = runtime.with_pty_io_barrier(&session_id, |barrier| {
            let encoded = (|| {
                let outputs = barrier.drain_output()?;
                self.apply_outputs(outputs);
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
                                request_id: request_id.clone(),
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
                            || barrier_control.is_cancelled(&request_id),
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
                        request_id: request_id.clone(),
                        snapshot: None,
                        phase: None,
                        error_kind: Some(error.to_string()),
                        barrier_released: false,
                        color_profile: None,
                    },
                    || barrier_control.is_cancelled(&request_id),
                );
            }
            match barrier_control.wait_for_release(&request_id) {
                SnapshotBarrierRelease::Cancel => return Ok(()),
                SnapshotBarrierRelease::Complete(resize) => {
                    let release_error = if let Some(size) = resize {
                        self.ghostty
                            .resize(TerminalScreenSize::new(size.rows, size.cols));
                        barrier.resize(size).err().map(|error| error.to_string())
                    } else {
                        None
                    };
                    let _ = self.egress.send_protected_json(
                        FRAME_SNAPSHOT,
                        &WorkerSnapshotResult {
                            request_id: request_id.clone(),
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
        });
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
}

impl SnapshotBarrierControl {
    fn begin(&self, request_id: String) {
        if let Ok(mut state) = self.state.lock() {
            state.active_request = Some(request_id);
            state.staged_resize = None;
            state.release = None;
            self.wake.notify_all();
        }
    }

    fn cancel_active(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.active_request.is_some() {
                state.release = Some(SnapshotBarrierRelease::Cancel);
                self.wake.notify_all();
            }
        }
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

#[derive(Default)]
enum WorkerLifecycle {
    #[default]
    Running,
    Stopping,
    Exited,
}

impl WorkerLifecycle {
    fn request_shutdown(&mut self) {
        if matches!(self, Self::Running) {
            *self = Self::Stopping;
        }
    }

    fn observe_process_exit(&mut self) {
        *self = Self::Exited;
    }

    fn should_continue(&self) -> bool {
        !matches!(self, Self::Exited)
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
) {
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
            if sender.send(frame).is_err() {
                break;
            }
            wakes.notify_session(&session_id);
        }
        snapshot_barrier.cancel_active();
        wakes.notify_session(&session_id);
    });
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

/// Write protected frames, then pending metadata. `FRAME_PROCESS_EXITED` is
/// terminal: drain metadata first so queued observations still precede it,
/// write the exit frame, then stop so no later frame follows it.
fn write_egress_lanes(
    mut write_frame: impl FnMut(&[u8]) -> Result<(), String>,
    protected: Receiver<Vec<u8>>,
    metadata: Receiver<Vec<u8>>,
) -> Result<(), String> {
    while let Ok(frame) = protected.recv() {
        if write_one_protected_frame(&mut write_frame, &metadata, frame)? {
            return Ok(());
        }
        while let Ok(frame) = protected.try_recv() {
            if write_one_protected_frame(&mut write_frame, &metadata, frame)? {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn write_one_protected_frame(
    write_frame: &mut impl FnMut(&[u8]) -> Result<(), String>,
    metadata: &Receiver<Vec<u8>>,
    frame: Vec<u8>,
) -> Result<bool, String> {
    if is_process_exited_frame(&frame) {
        drain_metadata_lane(metadata, |queued| write_frame(queued))?;
        write_frame(&frame)?;
        return Ok(true);
    }
    write_frame(&frame)?;
    drain_metadata_lane(metadata, |queued| write_frame(queued))?;
    Ok(false)
}

fn write_egress(
    mut stdout: impl Write,
    protected: Receiver<Vec<u8>>,
    metadata: Receiver<Vec<u8>>,
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

    fn spawn_readers(
        &self,
        initial: Box<dyn ReadWrite + Send>,
        sender: mpsc::Sender<Frame>,
        snapshot_barrier: Arc<SnapshotBarrierControl>,
        metadata: SessionMetadata,
        wakes: TerminalWakeSource,
        session_id: SessionId,
    ) {
        spawn_control_reader(
            initial,
            sender.clone(),
            Arc::clone(&snapshot_barrier),
            wakes.clone(),
            session_id.clone(),
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
                    );
                }
            });
        }
    }

    fn spawn_writer(
        &self,
        protected: Receiver<Vec<u8>>,
        metadata: Receiver<Vec<u8>>,
    ) -> thread::JoinHandle<Result<(), String>> {
        match self {
            Self::Stdio => thread::spawn(move || write_egress(io::stdout(), protected, metadata)),
            #[cfg(unix)]
            Self::Socket { writer, .. } => {
                let writer = Arc::clone(writer);
                thread::spawn(move || {
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
            if cancelled() {
                return false;
            }
            match self.protected_sender.try_send(frame) {
                Ok(()) => return true,
                Err(TrySendError::Full(returned)) => {
                    frame = returned;
                    thread::sleep(Duration::from_millis(1));
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

struct WorkerArgs {
    egress_capacity: usize,
    pty_reader_chunk_capacity: usize,
    shutdown_grace_ms: u64,
    poll_interval_ms: u64,
    control_socket: Option<PathBuf>,
    test_hold_after_read_ms: Option<u64>,
    test_write_block_until_unix_ms: Option<u64>,
    test_write_max_chunk: Option<usize>,
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
        let mut poll_interval_ms = 10;
        let mut control_socket = None;
        let mut test_hold_after_read_ms = None;
        let mut test_write_block_until_unix_ms = None;
        let mut test_write_max_chunk = None;
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
                "--poll-interval-ms" => {
                    index += 1;
                    poll_interval_ms = parse_arg(&args, index, "--poll-interval-ms")?;
                }
                "--control-socket" => {
                    index += 1;
                    control_socket = Some(PathBuf::from(parse_string_arg(
                        &args,
                        index,
                        "--control-socket",
                    )?));
                }
                "--test-hold-after-read-ms" => {
                    index += 1;
                    test_hold_after_read_ms =
                        Some(parse_arg(&args, index, "--test-hold-after-read-ms")?);
                }
                "--test-write-block-until-unix-ms" => {
                    index += 1;
                    test_write_block_until_unix_ms =
                        Some(parse_arg(&args, index, "--test-write-block-until-unix-ms")?);
                }
                "--test-write-max-chunk" => {
                    index += 1;
                    test_write_max_chunk = Some(parse_arg(&args, index, "--test-write-max-chunk")?);
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
            poll_interval_ms,
            control_socket,
            test_hold_after_read_ms,
            test_write_block_until_unix_ms,
            test_write_max_chunk,
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
        super::write_egress(&mut stdout, protected_rx, metadata_rx).expect("stdio writer");
        assert_eq!(
            decode_frame_types(&stdout),
            vec![super::FRAME_TITLE_CHANGED, super::FRAME_PROCESS_EXITED]
        );
    }

    #[test]
    fn writer_emits_queued_metadata_then_process_exited_and_drops_later_protected_frames() {
        let (protected_tx, protected_rx) = std::sync::mpsc::sync_channel(8);
        let (metadata_tx, metadata_rx) = std::sync::mpsc::sync_channel(8);
        let late_pty =
            botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"after-exit").expect("pty");
        let late_title =
            botster_core::encode_string(super::FRAME_TITLE_CHANGED, "after-exit-title")
                .expect("title");
        protected_tx
            .send(process_exited_frame())
            .expect("queue process-exited");
        protected_tx.send(late_pty).expect("queue late pty");
        metadata_tx.send(late_title).expect("queue late title");
        drop(protected_tx);
        drop(metadata_tx);

        let mut stdout = Vec::new();
        super::write_egress(&mut stdout, protected_rx, metadata_rx).expect("stdio writer");
        assert_eq!(
            decode_frame_types(&stdout),
            vec![super::FRAME_TITLE_CHANGED, super::FRAME_PROCESS_EXITED]
        );
    }

    #[test]
    fn socket_writer_path_is_terminal_after_process_exited() {
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
    fn shutdown_keeps_worker_loop_alive_until_process_exit_is_observed() {
        let mut lifecycle = WorkerLifecycle::default();

        lifecycle.request_shutdown();
        assert!(lifecycle.should_continue());

        lifecycle.observe_process_exit();
        assert!(!lifecycle.should_continue());
    }

    #[test]
    fn control_eof_releases_an_active_snapshot_barrier() {
        let control = Arc::new(SnapshotBarrierControl::default());
        control.begin("snapshot-eof".to_string());
        let waiter = Arc::clone(&control);
        let joined = std::thread::spawn(move || waiter.wait_for_release("snapshot-eof"));

        control.cancel_active();

        assert!(matches!(
            joined.join().expect("barrier waiter"),
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
