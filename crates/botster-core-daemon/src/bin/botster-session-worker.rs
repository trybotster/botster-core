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
use std::time::Duration;

use botster_core::contract::terminal_screen::{TerminalKeyEvent, TerminalMouseEvent};
use botster_core::engine::TerminalScreenRuntime;
use botster_core::{
    decode_worker_input_operation, encode_final_state, read_hello, write_startup_failure,
    write_welcome, Frame, LocalProcessRuntime, LocalProcessRuntimeOptions, ModeFlags,
    ModeFlagsPayload, PtyPollFds, PtyRead, ReservedSessionSpawnError, ResizePayload, ScreenPayload,
    SessionId, SessionMetadata, SessionReservation, SessionRuntime, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, StartupFailureOutcome, StartupFailureReport,
    TerminalMetadataKind, TerminalMetadataLaneShaper, TerminalMetadataObservation,
    TerminalMetadataProducer, TerminalMetadataShapingObservation, TerminalMetadataShapingOutcome,
    TerminalScreenSize, TimeoutPayload, WorkerFinalState, WorkerHealth, WorkerInputKind,
    WorkerProbeRequest, WorkerSnapshotPhase, WorkerSnapshotRequest, WorkerSnapshotResult,
    FRAME_BELL, FRAME_CWD_CHANGED, FRAME_FINAL_STATE, FRAME_GET_MODE_FLAGS, FRAME_GET_SCREEN,
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
    if std::env::args().skip(1).eq(["--probe".to_string()]) {
        // Identity only: no sockets, PTYs, or environment-dependent setup.
        // A host runs this once so the OS's first-exec cost for a newly
        // installed binary is paid before the first worker spawn.
        println!(
            "botster-session-worker {} protocol {}",
            env!("CARGO_PKG_VERSION"),
            PROTOCOL_VERSION
        );
        return;
    }
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
    let initial = control.accept_initial()?;
    let mut initial_control = initial.handshake_stream()?;

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

    let runtime_options = LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(args.shutdown_grace_ms),
        pty_reader_chunk_capacity: args.pty_reader_chunk_capacity,
        test_pending_capacity: args.test_pending_capacity,
    };
    let mut runtime = LocalProcessRuntime::with_options(runtime_options).with_polled_pty();
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

    // One thread from here on: the PTY, the child exit, every control
    // connection and the egress are descriptors in one poll.
    drop(initial_control);
    let mut io = WorkerIo::start(control, initial, metadata)?;
    let pty_fds = runtime
        .poll_fds(&session_id)
        .map_err(|error| error.to_string())?;
    let state = WorkerState {
        session_id: handle.session_id.clone(),
        ghostty,
        metadata_producer: TerminalMetadataProducer::new(),
        metadata_shaper: TerminalMetadataLaneShaper::new(
            (args.egress_capacity / 2).max(1),
            args.egress_capacity.saturating_mul(4).max(1),
        ),
        egress: Egress::new(args.egress_capacity.max(1)),
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
        pty_fds: Some(pty_fds),
        pty_reading: true,
    };
    let mut worker = WorkerLoop {
        state,
        runtime,
        snapshot: SnapshotGate::default(),
        lifecycle: WorkerLifecycle::default(),
        reconnect_timeout_seconds: None,
        shutdown_on_disconnect: io.shutdown_on_disconnect(),
        egress_full_signal: args.test_egress_full_signal.clone(),
        control_held_signal: args.test_control_held_signal.clone(),
    };

    while worker.lifecycle.should_continue() {
        worker.turn(&mut io)?;
        if !worker.lifecycle.should_continue() {
            break;
        }
        // A flush can free room for work that no descriptor will announce:
        // take it now instead of waiting.
        if worker.can_progress(&io) {
            io.ready.clear();
            continue;
        }
        io.wait(&worker)?;
    }

    // Everything queued reaches the parent before the worker exits.
    io.flush_all(&mut worker.state.egress)?;
    if let Some(gate) = &args.test_hold_before_exit_gate {
        // Test-only: block reading the gate pipe with stdout still open,
        // until the test writes it or ends the worker.
        let _ = std::fs::read(gate);
    }
    if let Some(exit_code) = args.test_exit_code {
        process::exit(exit_code);
    }
    Ok(())
}

/// The worker's state and runtime, driven one poll turn at a time.
struct WorkerLoop {
    state: WorkerState,
    runtime: LocalProcessRuntime,
    snapshot: SnapshotGate,
    lifecycle: WorkerLifecycle,
    reconnect_timeout_seconds: Option<u64>,
    shutdown_on_disconnect: bool,
    /// Test-only: written once the output egress stays full after a flush.
    egress_full_signal: Option<PathBuf>,
    /// Test-only: written once control intake stops for lack of reply room,
    /// with the queued replies and the reserved results.
    control_held_signal: Option<PathBuf>,
}

impl WorkerLoop {
    /// Work that can progress without a new event: snapshot frames or
    /// buffered control frames that the reply class now has room for.
    fn can_progress(&self, io: &WorkerIo) -> bool {
        self.state.control_has_room() && (self.snapshot.has_unsent() || io.has_buffered_control())
    }

    /// Everything that can progress now, without waiting: readiness found by
    /// the last poll, control frames already buffered, snapshot frames the
    /// egress has room for, PTY writes, the egress flush.
    fn turn(&mut self, io: &mut WorkerIo) -> Result<(), String> {
        // Control first: input is applied even while output is held.
        self.state.egress.produce(EgressClass::Reply);
        io.accept_ready(&mut self.state.egress);
        io.read_ready_connections();
        while self.state.control_has_room() {
            match io.next_control_event() {
                Some(ControlEvent::Frame(frame)) => self.handle_control_frame(frame)?,
                Some(ControlEvent::Ended { stdio }) => self.handle_control_end(stdio)?,
                None => break,
            }
        }
        self.progress_snapshot()?;

        // The PTY: one chunk while the output class has room, then the
        // runtime's output (chunks, and the exit once the reader ended and
        // the child can be reaped). Nothing moves while a capture holds it.
        self.state.egress.produce(EgressClass::Output);
        if !self.snapshot.is_active() {
            if let Some(fds) = self.state.pty_fds {
                if io.readable(fds.pty)
                    && self.state.pty_reading
                    && self.state.egress.has_room(EgressClass::Output)
                {
                    let read = self
                        .runtime
                        .read_ready(&self.state.session_id)
                        .map_err(|error| error.to_string())?;
                    if read == PtyRead::Closed {
                        self.state.pty_reading = false;
                    }
                }
                if fds.exit.is_some_and(|fd| io.readable(fd))
                    && self
                        .runtime
                        .poll_exit(&self.state.session_id)
                        .map_err(|error| error.to_string())?
                {
                    // Reapable: the drain collects it. Stop watching.
                    if let Some(fds) = self.state.pty_fds.as_mut() {
                        fds.exit = None;
                    }
                }
            }
            self.state.drain_and_apply_pty_output(&mut self.runtime)?;
        }
        if self.state.exited {
            self.lifecycle.observe_process_exit();
            // A socket worker whose every control connection has closed has
            // no parent left to serve.
            if !self.shutdown_on_disconnect && io.connections() == 0 {
                self.lifecycle.request_shutdown();
            }
        }

        if self.state.pty_fds.is_some() {
            self.state.progress_pending_writes(&self.runtime);
            // A shutdown whose grace has passed escalates now.
            self.runtime
                .advance_shutdown(&self.state.session_id)
                .map_err(|error| error.to_string())?;
        }
        io.flush(&mut self.state.egress);
        // A flush writes until the descriptor refuses, so output still full
        // here means the parent has stopped reading.
        if !self.state.egress.has_room(EgressClass::Output) {
            if let Some(signal) = self.egress_full_signal.take() {
                let _ = std::fs::write(signal, b"full\n");
            }
        }
        if !self.state.control_has_room() {
            if let Some(signal) = self.control_held_signal.take() {
                let report = format!(
                    "held {} {}\n",
                    self.state.egress.queued(EgressClass::Reply),
                    self.state.pending_ops
                );
                let _ = std::fs::write(signal, report);
            }
        }
        Ok(())
    }

    fn handle_control_end(&mut self, stdio: bool) -> Result<(), String> {
        // A link that ends releases an active capture: nothing will
        // complete it on this link.
        self.snapshot.cancel_active();
        if stdio && self.shutdown_on_disconnect {
            self.request_shutdown()?;
        }
        Ok(())
    }

    fn request_shutdown(&mut self) -> Result<(), String> {
        if !self.state.exited && self.state.pty_fds.is_some() {
            self.runtime
                .send_input(SessionRuntimeInput::Shutdown {
                    session_id: self.state.session_id.clone(),
                })
                .map_err(|error| error.to_string())?;
        }
        self.lifecycle.request_shutdown();
        Ok(())
    }

    fn handle_control_frame(&mut self, frame: Frame) -> Result<(), String> {
        let state = &mut self.state;
        let runtime = &mut self.runtime;
        match frame.frame_type {
            FRAME_PTY_INPUT => {
                state.queue_keyless_write(frame.payload);
            }
            FRAME_INPUT_OPERATION => {
                // Apply every drained byte before encoding so modes are current.
                state.drain_and_apply_pty_output(runtime)?;
                state.admit_input_operation(runtime, &frame.payload);
            }
            FRAME_INPUT_CANCEL => {
                if let Ok((key, _)) = botster_core::split_worker_operation_key(&frame.payload) {
                    state.cancel_operation(key);
                }
            }
            FRAME_GET_MODE_FLAGS => {
                let request_id = probe_request_id(&frame.payload);
                state.drain_and_apply_pty_output(runtime)?;
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
                state.drain_and_apply_pty_output(runtime)?;
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
                let size: ResizePayload =
                    serde_json::from_slice(&frame.payload).map_err(|error| error.to_string())?;
                // During a capture the resize waits for the release.
                if self.snapshot.stage_resize(size.clone()) {
                    return Ok(());
                }
                if state.exited || state.pty_fds.is_none() {
                    // No PTY is left; the final model takes the size.
                    state
                        .ghostty
                        .resize(TerminalScreenSize::new(size.rows, size.cols));
                    state.publish_modes_if_changed();
                } else {
                    state.apply_resize(runtime, size.rows, size.cols, None)?;
                }
                state
                    .egress
                    .send_protected_json(FRAME_RESIZE_APPLIED, &size);
            }
            FRAME_GET_SNAPSHOT => {
                if let Ok(request) = serde_json::from_slice::<WorkerSnapshotRequest>(&frame.payload)
                {
                    if request.cancel {
                        self.snapshot.request_cancel(&request.request_id);
                        return Ok(());
                    }
                    if request.complete {
                        self.snapshot.request_complete(&request.request_id);
                        return Ok(());
                    }
                }
                self.begin_snapshot(&frame.payload);
            }
            FRAME_PING => {
                let health = WorkerHealth {
                    session_id: state.session_id.clone(),
                    worker_pid: process::id(),
                    reconnect_timeout_seconds: self.reconnect_timeout_seconds,
                };
                state.egress.send_protected_json(FRAME_PONG, &health);
            }
            FRAME_SET_TIMEOUT => {
                let timeout: TimeoutPayload =
                    serde_json::from_slice(&frame.payload).map_err(|error| error.to_string())?;
                self.reconnect_timeout_seconds = Some(timeout.seconds);
            }
            FRAME_SHUTDOWN => {
                // A shutdown ends an active capture, then is acted on.
                self.snapshot.cancel_active();
                self.request_shutdown()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Take the capture boundary and queue the snapshot frames. The frames
    /// reach the egress as the reply class has room; the release follows.
    fn begin_snapshot(&mut self, payload: &[u8]) {
        let request = match serde_json::from_slice::<WorkerSnapshotRequest>(payload) {
            Ok(request) => request,
            Err(error) => {
                self.state.egress.send_protected_json(
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
        // A capture already active ends as a cancel; this one replaces it.
        self.snapshot.begin(request_id.clone());
        self.state.deferring_exit = true;
        let session_id = self.state.session_id.clone();
        let state = &mut self.state;
        let snapshot = &mut self.snapshot;
        let result = if state.exited || state.pty_fds.is_none() {
            // The child has exited and its PTY is gone. The final terminal
            // model still answers, with the same release handshake.
            state.capture_snapshot(None, &request_id, snapshot);
            Ok(())
        } else {
            self.runtime.with_pty_io_barrier(&session_id, |barrier| {
                state.capture_snapshot(Some(barrier), &request_id, snapshot);
                Ok(())
            })
        };
        if let Err(error) = result {
            self.state.egress.send_protected_json(
                FRAME_SNAPSHOT,
                &WorkerSnapshotResult {
                    request_id,
                    snapshot: None,
                    phase: None,
                    error_kind: Some(error.to_string()),
                    barrier_released: false,
                    color_profile: None,
                },
            );
            self.snapshot = SnapshotGate::default();
            self.end_capture();
        }
    }

    /// Feed queued snapshot frames, and act on a decided release.
    fn progress_snapshot(&mut self) -> Result<(), String> {
        // Snapshot frames share the reply bound with pending results.
        let pending = self.state.pending_ops;
        let egress = &mut self.state.egress;
        let room = egress.capacity;
        let mut queued = egress.queued(EgressClass::Reply) + pending;
        let mut fed = Vec::new();
        self.snapshot.feed(
            || {
                let has_room = queued < room;
                queued += 1;
                has_room
            },
            |frame| fed.push(frame),
        );
        egress.produce(EgressClass::Reply);
        for frame in fed {
            egress.push_protected(frame);
        }
        let Some((request_id, release)) = self.snapshot.take_release() else {
            return Ok(());
        };
        if let SnapshotRelease::Complete(resize) = release {
            let release_error = match resize {
                Some(size) => {
                    self.state
                        .ghostty
                        .resize(TerminalScreenSize::new(size.rows, size.cols));
                    if self.state.pty_fds.is_some() {
                        self.runtime
                            .send_input(SessionRuntimeInput::Resize {
                                session_id: self.state.session_id.clone(),
                                size,
                            })
                            .err()
                            .map(|error| error.to_string())
                    } else {
                        None
                    }
                }
                None => None,
            };
            self.state.egress.send_protected_json(
                FRAME_SNAPSHOT,
                &WorkerSnapshotResult {
                    request_id,
                    snapshot: None,
                    phase: None,
                    error_kind: release_error,
                    barrier_released: true,
                    color_profile: None,
                },
            );
        }
        self.end_capture();
        Ok(())
    }

    /// After a capture: publish a mode change it held back, then an exit
    /// met during it (PROCESS_EXITED follows the capture's frames).
    fn end_capture(&mut self) {
        self.state.publish_modes_if_changed();
        self.state.deferring_exit = false;
        if let Some(payload) = self.state.deferred_exit.take() {
            self.state.egress.produce(EgressClass::Output);
            self.state.emit_process_exit(&payload);
        }
    }
}

enum ControlEvent {
    Frame(Frame),
    /// A connection ended (EOF, a read error, or a malformed frame).
    Ended {
        stdio: bool,
    },
}

/// One control connection: the stdio pipes, or one accepted socket.
struct ControlConnection {
    /// Readable end; for stdio, standard input.
    read: std::fs::File,
    /// Writable end for the egress; `None` for stdio (standard output is the
    /// egress descriptor itself).
    #[cfg(unix)]
    stream: Option<UnixStream>,
    decoder: FrameDecoder,
    /// An accepted socket reads its hello first; its welcome goes out before
    /// any frame. `false` once handshaken.
    awaiting_hello: bool,
    stdio: bool,
    /// Set once its end has been reported.
    finished: bool,
}

/// Where egress frames are written.
enum EgressTarget {
    Stdout(std::mem::ManuallyDrop<std::fs::File>),
    #[cfg(unix)]
    Socket(UnixStream),
    /// A socket worker between connections: frames are discarded.
    None,
}

/// The worker's descriptors and the readiness the last poll found.
struct WorkerIo {
    control: WorkerControl,
    connections: Vec<ControlConnection>,
    target: EgressTarget,
    /// Welcome bytes for a newly accepted connection.
    metadata: SessionMetadata,
    /// Descriptors the last poll reported ready, with their events.
    ready: Vec<(std::os::fd::RawFd, libc::c_short)>,
    /// A newly accepted connection's welcome, written before any frame.
    pending_preamble: Option<Vec<u8>>,
}

/// Bytes read from one control connection per readiness turn.
const CONTROL_READ_BYTES: usize = 8192;

fn set_nonblocking(fd: std::os::fd::RawFd) -> Result<(), String> {
    // SAFETY: fcntl on a live descriptor this process owns.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(format!(
            "read descriptor flags failed: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: as above; only O_NONBLOCK is added.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(format!(
            "set non-blocking failed: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

impl WorkerIo {
    fn start(
        control: WorkerControl,
        initial: InitialControl,
        metadata: SessionMetadata,
    ) -> Result<Self, String> {
        use std::os::fd::AsRawFd;
        let (connection, target) = match initial {
            InitialControl::Stdio => {
                set_nonblocking(0)?;
                set_nonblocking(1)?;
                (
                    ControlConnection {
                        read: stdio_file(0),
                        #[cfg(unix)]
                        stream: None,
                        decoder: FrameDecoder::default(),
                        awaiting_hello: false,
                        stdio: true,
                        finished: false,
                    },
                    EgressTarget::Stdout(std::mem::ManuallyDrop::new(stdio_file(1))),
                )
            }
            #[cfg(unix)]
            InitialControl::Socket(stream) => {
                stream
                    .set_nonblocking(true)
                    .map_err(|error| error.to_string())?;
                let read = stream.try_clone().map_err(|error| error.to_string())?;
                let write = stream.try_clone().map_err(|error| error.to_string())?;
                (
                    ControlConnection {
                        read: std::fs::File::from(std::os::fd::OwnedFd::from(read)),
                        stream: Some(stream),
                        decoder: FrameDecoder::default(),
                        awaiting_hello: false,
                        stdio: false,
                        finished: false,
                    },
                    EgressTarget::Socket(write),
                )
            }
        };
        #[cfg(unix)]
        if let WorkerControl::Socket { listener, .. } = &control {
            listener
                .set_nonblocking(true)
                .map_err(|error| error.to_string())?;
            let _ = listener.as_raw_fd();
        }
        Ok(Self {
            control,
            connections: vec![connection],
            target,
            metadata,
            ready: Vec::new(),
            pending_preamble: None,
        })
    }

    fn shutdown_on_disconnect(&self) -> bool {
        self.control.shutdown_on_disconnect()
    }

    /// A connection holds a complete frame, a pending hello, or its end.
    fn has_buffered_control(&self) -> bool {
        self.connections.iter().any(|connection| {
            !connection.finished
                && (connection.decoder.has_frame()
                    || (connection.awaiting_hello
                        && connection.decoder.buffer.len()
                            >= botster_core::encode_hello(PROTOCOL_VERSION).len()))
        })
    }

    /// Handshaken connections still open.
    fn connections(&self) -> usize {
        self.connections
            .iter()
            .filter(|connection| !connection.finished && !connection.awaiting_hello)
            .count()
    }

    /// Readable, hung up or failed: a read will not block.
    fn readable(&self, fd: std::os::fd::RawFd) -> bool {
        self.ready.iter().any(|(ready, events)| {
            *ready == fd && events & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
        })
    }

    /// Accept every pending connection; each reads its hello first.
    fn accept_ready(&mut self, egress: &mut Egress) {
        let _ = egress;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let WorkerControl::Socket { listener, .. } = &self.control else {
                return;
            };
            if !self.readable(listener.as_raw_fd()) {
                return;
            }
            while let Ok((stream, _)) = listener.accept() {
                if stream.set_nonblocking(true).is_err() {
                    continue;
                }
                let Ok(read) = stream.try_clone() else {
                    continue;
                };
                self.connections.push(ControlConnection {
                    read: std::fs::File::from(std::os::fd::OwnedFd::from(read)),
                    stream: Some(stream),
                    decoder: FrameDecoder::default(),
                    awaiting_hello: true,
                    stdio: false,
                    finished: false,
                });
            }
        }
    }

    /// One read from each ready connection whose frames are all taken.
    fn read_ready_connections(&mut self) {
        use std::os::fd::AsRawFd;
        let mut buffer = [0; CONTROL_READ_BYTES];
        for connection in &mut self.connections {
            if connection.finished || (!connection.awaiting_hello && connection.decoder.has_frame())
            {
                continue;
            }
            let fd = connection.read.as_raw_fd();
            let ready = self.ready.iter().any(|(ready, events)| {
                *ready == fd && events & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
            });
            if !ready {
                continue;
            }
            match connection.read.read(&mut buffer) {
                Ok(0) => connection.decoder.end(),
                Ok(read) => connection.decoder.push(&buffer[..read]),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => connection.decoder.end(),
            }
        }
    }

    /// The next control frame or connection end, oldest connection first.
    /// An accepted connection's hello is checked here; a good one becomes
    /// the egress target, with its welcome ahead of any frame.
    fn next_control_event(&mut self) -> Option<ControlEvent> {
        for index in 0..self.connections.len() {
            if self.connections[index].finished {
                continue;
            }
            if self.connections[index].awaiting_hello {
                match self.take_hello(index) {
                    HelloStep::Waiting => continue,
                    HelloStep::Rejected => {
                        // Never counted as a connection; nothing to report.
                        self.connections[index].finished = true;
                        continue;
                    }
                    HelloStep::Accepted => {}
                }
            }
            match self.connections[index].decoder.next_frame() {
                Ok(Some(frame)) => return Some(ControlEvent::Frame(frame)),
                Ok(None) => {}
                Err(()) => {
                    let connection = &mut self.connections[index];
                    connection.finished = true;
                    let stdio = connection.stdio;
                    self.retire_target_of(index);
                    return Some(ControlEvent::Ended { stdio });
                }
            }
        }
        self.connections.retain(|connection| !connection.finished);
        None
    }

    fn take_hello(&mut self, index: usize) -> HelloStep {
        let hello_len = botster_core::encode_hello(PROTOCOL_VERSION).len();
        let connection = &mut self.connections[index];
        let buffered = connection.decoder.buffer.len();
        if buffered < hello_len {
            return if connection.decoder.ended {
                HelloStep::Rejected
            } else {
                HelloStep::Waiting
            };
        }
        let hello: Vec<u8> = connection.decoder.buffer.drain(..hello_len).collect();
        if botster_core::decode_hello(&hello).ok() != Some(PROTOCOL_VERSION) {
            return HelloStep::Rejected;
        }
        let Ok(welcome) = botster_core::encode_welcome(PROTOCOL_VERSION, &self.metadata) else {
            return HelloStep::Rejected;
        };
        #[cfg(unix)]
        {
            let Some(write) = connection
                .stream
                .as_ref()
                .and_then(|stream| stream.try_clone().ok())
            else {
                return HelloStep::Rejected;
            };
            connection.awaiting_hello = false;
            self.target = EgressTarget::Socket(write);
            self.pending_preamble = Some(welcome);
        }
        HelloStep::Accepted
    }

    /// A connection that ended no longer receives the egress.
    fn retire_target_of(&mut self, index: usize) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let Some(stream) = self.connections[index].stream.as_ref() else {
                return;
            };
            let ended = stream.as_raw_fd();
            if let EgressTarget::Socket(target) = &self.target {
                // The target is a clone of the connection's stream: compare
                // the socket, not the descriptor number.
                if same_socket(target.as_raw_fd(), ended) {
                    self.target = EgressTarget::None;
                }
            }
        }
    }

    fn target_fd(&self) -> Option<std::os::fd::RawFd> {
        use std::os::fd::AsRawFd;
        match &self.target {
            EgressTarget::Stdout(file) => Some(file.as_raw_fd()),
            #[cfg(unix)]
            EgressTarget::Socket(stream) => Some(stream.as_raw_fd()),
            EgressTarget::None => None,
        }
    }

    /// Write what the egress descriptor accepts now.
    fn flush(&mut self, egress: &mut Egress) {
        if let Some(welcome) = self.pending_preamble.take() {
            egress.retarget(welcome);
        }
        let result = match &mut self.target {
            EgressTarget::Stdout(file) => egress.flush(|bytes| file.write(bytes)),
            #[cfg(unix)]
            EgressTarget::Socket(stream) => egress.flush(|bytes| stream.write(bytes)),
            EgressTarget::None => {
                egress.discard();
                Ok(())
            }
        };
        if result.is_err() {
            match self.target {
                // Standard output failed: nothing reaches the parent again.
                EgressTarget::Stdout(_) => egress.close(),
                // The socket failed: frames wait for no one until a new
                // connection takes over.
                #[cfg(unix)]
                EgressTarget::Socket(_) => {
                    self.target = EgressTarget::None;
                    egress.discard();
                }
                EgressTarget::None => {}
            }
        }
    }

    /// Flush everything, waiting for the descriptor, before the worker exits.
    fn flush_all(&mut self, egress: &mut Egress) -> Result<(), String> {
        loop {
            self.flush(egress);
            if !egress.wants_write() {
                return Ok(());
            }
            let Some(fd) = self.target_fd() else {
                return Ok(());
            };
            let mut poll_fd = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            // SAFETY: poll_fd points at one valid pollfd; -1 waits for the event.
            let count = unsafe { libc::poll(&mut poll_fd, 1, -1) };
            if count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(format!(
                    "egress poll failed: {}",
                    io::Error::last_os_error()
                ));
            }
        }
    }

    /// Wait for the next event. There is no timer: every source of progress
    /// is a descriptor in this poll.
    fn wait(&mut self, worker: &WorkerLoop) -> Result<(), String> {
        use std::os::fd::AsRawFd;
        let state = &worker.state;
        let mut fds: Vec<libc::pollfd> = Vec::new();
        let mut add = |fd: std::os::fd::RawFd, events: libc::c_short| {
            if events != 0 {
                fds.push(libc::pollfd {
                    fd,
                    events,
                    revents: 0,
                });
            }
        };
        if let Some(pty) = state.pty_fds {
            let mut events = 0;
            if state.pty_reading
                && !worker.snapshot.is_active()
                && state.egress.has_room(EgressClass::Output)
            {
                events |= libc::POLLIN;
            }
            if !state.pending_writes.is_empty() {
                events |= libc::POLLOUT;
            }
            add(pty.pty, events);
            if let Some(exit) = pty.exit {
                if !worker.snapshot.is_active() {
                    add(exit, libc::POLLIN);
                }
            }
        }
        let reply_room = state.control_has_room();
        for connection in &self.connections {
            if !connection.finished
                && (connection.awaiting_hello || (reply_room && !connection.decoder.has_frame()))
            {
                add(connection.read.as_raw_fd(), libc::POLLIN);
            }
        }
        #[cfg(unix)]
        if let WorkerControl::Socket { listener, .. } = &self.control {
            add(listener.as_raw_fd(), libc::POLLIN);
        }
        if state.egress.wants_write() {
            if let Some(fd) = self.target_fd() {
                add(fd, libc::POLLOUT);
            }
        }
        // Only a running shutdown has a deadline; otherwise every source of
        // progress is a descriptor in this poll, and it waits with no timer.
        let deadline = match state.pty_fds {
            Some(_) => worker
                .runtime
                .shutdown_deadline(&state.session_id)
                .map_err(|error| error.to_string())?,
            None => None,
        };
        loop {
            let timeout_ms = deadline.map_or(-1, |deadline| {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                libc::c_int::try_from(left.as_nanos().div_ceil(1_000_000))
                    .unwrap_or(libc::c_int::MAX)
            });
            // SAFETY: fds is a live array of valid pollfd records.
            let count = unsafe {
                // timer: deadline — shutdown grace; expiry escalates to the group kill, then fails the worker
                libc::poll(
                    fds.as_mut_ptr(),
                    libc::nfds_t::try_from(fds.len()).map_err(|error| error.to_string())?,
                    timeout_ms,
                )
            };
            if count >= 0 {
                break;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(format!("worker poll failed: {error}"));
            }
        }
        self.ready = fds
            .iter()
            .filter(|fd| fd.revents != 0)
            .map(|fd| (fd.fd, fd.revents))
            .collect();
        Ok(())
    }
}

enum HelloStep {
    Waiting,
    Accepted,
    Rejected,
}

/// Standard input or output as a file that this process must not close.
fn stdio_file(fd: std::os::fd::RawFd) -> std::fs::File {
    // SAFETY: fd 0 and fd 1 stay open for the process lifetime; the caller
    // wraps the output file in ManuallyDrop, and standard input is only
    // closed at exit.
    unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) }
}

/// Whether two descriptors refer to the same open socket.
#[cfg(unix)]
fn same_socket(left: std::os::fd::RawFd, right: std::os::fd::RawFd) -> bool {
    // SAFETY: fstat on live descriptors into zeroed records.
    let (mut a, mut b): (libc::stat, libc::stat) =
        unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
    // SAFETY: as above.
    let ok = unsafe { libc::fstat(left, &mut a) == 0 && libc::fstat(right, &mut b) == 0 };
    ok && a.st_dev == b.st_dev && a.st_ino == b.st_ino
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
    egress: Egress,
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
    /// The session's PTY and exit descriptors. `None` once the drain that
    /// reported the exit removed the session and closed them, even while
    /// that exit is held back behind a capture.
    pty_fds: Option<PtyPollFds>,
    /// The PTY still has output to read.
    pty_reading: bool,
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

    fn send_result(&mut self, key: u64, result: &InputResultBody) {
        // A body that cannot encode is a programming error in this process;
        // the parent reports OutcomeUnknown on link loss.
        if let Some(payload) = input_result_payload(key, result) {
            // A result answers a control frame whenever its write finishes,
            // so it is always a reply, never output.
            let producing = self.egress.producing;
            self.egress.produce(EgressClass::Reply);
            self.egress
                .send_protected_frame(FRAME_INPUT_RESULT, payload);
            self.egress.produce(producing);
        }
    }

    /// Room to take another control frame: the reply class holds the
    /// replies already queued, and one slot is reserved for each admitted
    /// operation whose result is still to come. Results published later by
    /// PTY writes therefore stay within the bound.
    fn control_has_room(&self) -> bool {
        self.egress.queued(EgressClass::Reply) + self.pending_ops < self.egress.capacity
    }

    fn reject(&mut self, key: u64, operation_id: u64, outcome: InputOutcome, detail: &str) {
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
            send_metadata_observation(&mut self.egress, observation);
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
                    // The drain that reports the exit removed the session:
                    // its descriptors are closed now.
                    self.pty_fds = None;
                    self.pty_reading = false;
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
            send_metadata_observation(&mut self.egress, observation);
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
        // After the exit's drain the runtime no longer has the session.
        if self.exited || self.pty_fds.is_none() {
            return Ok(());
        }
        let outputs = runtime
            .drain_output(&self.session_id)
            .map_err(|error| error.to_string())?;
        self.apply_outputs(outputs);
        Ok(())
    }

    /// Take a capture's boundary and queue its snapshot frames on `gate`.
    /// Under `barrier` the PTY output read so far (and what the PTY still
    /// holds) is applied first; with none, the child has exited and the
    /// final terminal model is served. Errors become an error frame.
    fn capture_snapshot(
        &mut self,
        mut barrier: Option<&mut botster_core::PtyIoBarrier<'_>>,
        request_id: &str,
        gate: &mut SnapshotGate,
    ) {
        let producing = self.egress.producing;
        let encoded = (|| {
            if let Some(barrier) = barrier.as_mut() {
                let outputs = barrier.drain_output()?;
                // Output applied at the boundary is output, not a reply.
                self.egress.produce(EgressClass::Output);
                self.apply_outputs(outputs);
                self.egress.produce(producing);
            }
            let size = self.ghostty.size();
            let color_profile = self.ghostty.read_color_profile().unwrap_or_default();
            self.ghostty
                .export_snapshot_frames(|frame| {
                    let phase = match frame.kind {
                        GhosttySnapshotFrameKind::Ready => WorkerSnapshotPhase::Ready,
                        GhosttySnapshotFrameKind::History => WorkerSnapshotPhase::History,
                        GhosttySnapshotFrameKind::Finish => WorkerSnapshotPhase::Finish,
                    };
                    let result = WorkerSnapshotResult {
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
                    };
                    if let Ok(encoded) = botster_core::encode_json(FRAME_SNAPSHOT, &result) {
                        gate.queue_frame(encoded);
                    }
                    true
                })
                .map_err(|error| {
                    botster_core::SessionRuntimeError::new(
                        botster_core::SessionRuntimeErrorKind::OutputFailed,
                        error.to_string(),
                    )
                })
        })();
        if let Err(error) = encoded {
            let result = WorkerSnapshotResult {
                request_id: request_id.to_owned(),
                snapshot: None,
                phase: None,
                error_kind: Some(error.to_string()),
                barrier_released: false,
                color_profile: None,
            };
            if let Ok(encoded) = botster_core::encode_json(FRAME_SNAPSHOT, &result) {
                gate.queue_frame(encoded);
            }
        }
    }
}

/// What caused an egress frame. Each class has its own bound, the worker's
/// egress capacity, in one FIFO, so the wire order stays as produced.
///
/// PTY reads stop while the output class is full; control reads, and the
/// feeding of snapshot frames, stop while the reply class is full. A flood
/// fills only the output class, so control input is still read and applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EgressClass {
    /// PTY output and what it causes: modes, metadata shaping, the exit.
    Output,
    /// What a control frame causes: results, replies, snapshot frames.
    Reply,
}

/// The worker's egress: the protected lane and the metadata lane, written
/// without blocking when the egress descriptor is writable.
///
/// Write order: after each protected frame, the queued metadata; before
/// `FRAME_PROCESS_EXITED`, the queued metadata first, so every observation
/// queued before the exit precedes it. The exit does not end the egress: the
/// worker keeps answering control requests (captures from its final model).
struct Egress {
    capacity: usize,
    /// The class of the frames produced now; the loop sets it per event.
    producing: EgressClass,
    protected: VecDeque<(EgressClass, Vec<u8>)>,
    output_queued: usize,
    reply_queued: usize,
    metadata: VecDeque<Vec<u8>>,
    /// Bytes to write before any frame: a new connection's welcome.
    preamble: Vec<u8>,
    /// The frame being written, and how much of it was written.
    writing: Option<(Option<EgressClass>, Vec<u8>, usize)>,
    /// After a protected frame, the queued metadata goes next.
    metadata_turn: bool,
    /// The descriptor failed (stdio). Nothing is written again.
    closed: bool,
}

impl Egress {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            producing: EgressClass::Output,
            protected: VecDeque::new(),
            output_queued: 0,
            reply_queued: 0,
            metadata: VecDeque::new(),
            preamble: Vec::new(),
            writing: None,
            metadata_turn: false,
            closed: false,
        }
    }

    fn produce(&mut self, class: EgressClass) {
        self.producing = class;
    }

    fn queued(&self, class: EgressClass) -> usize {
        match class {
            EgressClass::Output => self.output_queued,
            EgressClass::Reply => self.reply_queued,
        }
    }

    fn has_room(&self, class: EgressClass) -> bool {
        self.queued(class) < self.capacity
    }

    fn count(&mut self, class: EgressClass, added: bool) {
        let slot = match class {
            EgressClass::Output => &mut self.output_queued,
            EgressClass::Reply => &mut self.reply_queued,
        };
        if added {
            *slot += 1;
        } else {
            *slot = slot.saturating_sub(1);
        }
    }

    fn push_protected(&mut self, frame: Vec<u8>) {
        if self.closed {
            return;
        }
        let class = self.producing;
        self.count(class, true);
        self.protected.push_back((class, frame));
    }

    fn send_protected_frame(&mut self, frame_type: u8, payload: Vec<u8>) -> bool {
        match botster_core::encode_frame(frame_type, &payload) {
            Ok(frame) => {
                self.push_protected(frame);
                true
            }
            Err(_) => false,
        }
    }

    fn send_protected_json<T: serde::Serialize>(&mut self, frame_type: u8, payload: &T) -> bool {
        match botster_core::encode_json(frame_type, payload) {
            Ok(frame) => {
                self.push_protected(frame);
                true
            }
            Err(_) => false,
        }
    }

    fn send_metadata_frame(
        &mut self,
        frame_type: u8,
        payload: Vec<u8>,
        kind: TerminalMetadataKind,
    ) {
        if let Ok(frame) = botster_core::encode_frame(frame_type, &payload) {
            self.try_send_metadata(frame, kind);
        }
    }

    fn send_metadata_json<T: serde::Serialize>(
        &mut self,
        frame_type: u8,
        payload: &T,
        kind: TerminalMetadataKind,
    ) {
        if let Ok(frame) = botster_core::encode_json(frame_type, payload) {
            self.try_send_metadata(frame, kind);
        }
    }

    fn send_metadata_string(&mut self, frame_type: u8, payload: &str, kind: TerminalMetadataKind) {
        if let Ok(frame) = botster_core::encode_string(frame_type, payload) {
            self.try_send_metadata(frame, kind);
        }
    }

    /// The metadata lane holds at most the egress capacity; an observation
    /// that finds it full is dropped and reported on the protected lane.
    fn try_send_metadata(&mut self, frame: Vec<u8>, kind: TerminalMetadataKind) {
        if self.closed {
            return;
        }
        if self.metadata.len() < self.capacity {
            self.metadata.push_back(frame);
            return;
        }
        self.send_protected_json(
            FRAME_METADATA_SHAPING,
            &TerminalMetadataShapingObservation {
                kind: Some(kind),
                outcome: TerminalMetadataShapingOutcome::Dropped,
                count: 1,
            },
        );
    }

    /// A new egress descriptor: the half-written frame (meant for the old
    /// one) is dropped, and `preamble` is written first.
    fn retarget(&mut self, preamble: Vec<u8>) {
        if let Some((Some(class), _, _)) = self.writing.take() {
            self.count(class, false);
        }
        self.preamble = preamble;
    }

    /// Whether anything waits to be written.
    fn wants_write(&self) -> bool {
        !self.closed
            && (!self.preamble.is_empty()
                || self.writing.is_some()
                || !self.protected.is_empty()
                || !self.metadata.is_empty())
    }

    fn next_frame(&mut self) -> Option<(Option<EgressClass>, Vec<u8>)> {
        let exit_next = self
            .protected
            .front()
            .is_some_and(|(_, frame)| is_process_exited_frame(frame));
        if (self.metadata_turn || exit_next || self.protected.is_empty())
            && !self.metadata.is_empty()
        {
            return self.metadata.pop_front().map(|frame| (None, frame));
        }
        self.metadata_turn = false;
        let (class, frame) = self.protected.pop_front()?;
        self.metadata_turn = true;
        Some((Some(class), frame))
    }

    /// Write what the descriptor accepts now. `write` is a non-blocking
    /// write: `WouldBlock` stops the flush, and any other error is returned
    /// (the caller decides whether the egress closes or loses its target).
    fn flush(&mut self, mut write: impl FnMut(&[u8]) -> io::Result<usize>) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        while !self.preamble.is_empty() {
            match write(&self.preamble) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(written) => {
                    self.preamble.drain(..written);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        loop {
            if self.writing.is_none() {
                let Some((class, frame)) = self.next_frame() else {
                    return Ok(());
                };
                self.writing = Some((class, frame, 0));
            }
            let (class, frame, offset) = self.writing.as_mut().expect("a frame is being written");
            match write(&frame[*offset..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(written) => {
                    *offset += written;
                    if *offset == frame.len() {
                        let class = *class;
                        self.writing = None;
                        if let Some(class) = class {
                            self.count(class, false);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    /// With no descriptor to write to (a socket worker between
    /// connections), queued frames are discarded, as the link has no reader.
    fn discard(&mut self) {
        self.writing = None;
        self.protected.clear();
        self.metadata.clear();
        self.preamble.clear();
        self.output_queued = 0;
        self.reply_queued = 0;
        self.metadata_turn = false;
    }

    /// The descriptor failed for good: drop everything, and every later frame.
    fn close(&mut self) {
        self.discard();
        self.closed = true;
    }
}

/// Decodes length-prefixed control frames from non-blocking reads.
///
/// A read of zero bytes is the end of the connection. An end inside a frame,
/// or an invalid length, is also the end: nothing after it can be trusted.
#[derive(Default)]
struct FrameDecoder {
    buffer: Vec<u8>,
    ended: bool,
}

impl FrameDecoder {
    fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    fn end(&mut self) {
        self.ended = true;
    }

    /// The next complete frame, or `Err(())` once the connection has ended.
    fn next_frame(&mut self) -> Result<Option<Frame>, ()> {
        if self.buffer.len() >= 4 {
            let len = u32::from_le_bytes(self.buffer[..4].try_into().expect("four bytes")) as usize;
            if len == 0 || len > botster_core::MAX_FRAME_LEN {
                self.buffer.clear();
                self.ended = true;
                return Err(());
            }
            if self.buffer.len() >= 4 + len {
                let body: Vec<u8> = self.buffer.drain(..4 + len).skip(4).collect();
                return Ok(Some(Frame {
                    frame_type: body[0],
                    payload: body[1..].to_vec(),
                }));
            }
        }
        if self.ended {
            self.buffer.clear();
            return Err(());
        }
        Ok(None)
    }

    /// Whether the buffer holds a complete frame (or the end) not yet taken.
    fn has_frame(&self) -> bool {
        if self.ended {
            return true;
        }
        if self.buffer.len() < 4 {
            return false;
        }
        let len = u32::from_le_bytes(self.buffer[..4].try_into().expect("four bytes")) as usize;
        len == 0 || len > botster_core::MAX_FRAME_LEN || self.buffer.len() >= 4 + len
    }
}

/// How a capture's barrier ends.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SnapshotRelease {
    Cancel,
    Complete(Option<ResizePayload>),
}

/// The capture barrier as loop state (one capture at a time).
///
/// While a capture is active, the worker neither reads nor drains its PTY:
/// its terminal model stays at the capture boundary until the release. The
/// parent's cancel, complete, shutdown or link end releases it; a resize that
/// arrives meanwhile is staged and applied at the release.
#[derive(Default)]
struct SnapshotGate {
    active: Option<ActiveSnapshot>,
}

struct ActiveSnapshot {
    request_id: String,
    /// Encoded frames not yet in the egress; fed while the reply class has room.
    unsent: VecDeque<Vec<u8>>,
    staged_resize: Option<ResizePayload>,
    release: Option<SnapshotRelease>,
}

impl SnapshotGate {
    fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Frames wait to be fed, or a decided release waits to be taken.
    fn has_unsent(&self) -> bool {
        self.active.as_ref().is_some_and(|active| {
            !active.unsent.is_empty() || matches!(active.release, Some(SnapshotRelease::Cancel))
        })
    }

    /// Start a capture. An active one ends first, as a cancel: its unsent
    /// frames are dropped and it gets no release frame.
    fn begin(&mut self, request_id: String) {
        self.active = Some(ActiveSnapshot {
            request_id,
            unsent: VecDeque::new(),
            staged_resize: None,
            release: None,
        });
    }

    fn queue_frame(&mut self, frame: Vec<u8>) {
        if let Some(active) = self.active.as_mut() {
            active.unsent.push_back(frame);
        }
    }

    /// Stage a resize for the release; `false` when no capture is active.
    fn stage_resize(&mut self, size: ResizePayload) -> bool {
        match self.active.as_mut() {
            Some(active) => {
                active.staged_resize = Some(size);
                true
            }
            None => false,
        }
    }

    fn request_cancel(&mut self, request_id: &str) {
        if let Some(active) = self.active.as_mut() {
            if active.request_id == request_id {
                active.release = Some(SnapshotRelease::Cancel);
            }
        }
    }

    fn request_complete(&mut self, request_id: &str) {
        if let Some(active) = self.active.as_mut() {
            if active.request_id == request_id && active.release.is_none() {
                let resize = active.staged_resize.take();
                active.release = Some(SnapshotRelease::Complete(resize));
            }
        }
    }

    /// Shutdown or a control link's end: cancel whatever is active.
    fn cancel_active(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.release = Some(SnapshotRelease::Cancel);
        }
    }

    /// Move unsent frames into `egress` while `room` allows. A cancelled
    /// capture sends nothing more.
    fn feed(&mut self, mut room: impl FnMut() -> bool, mut send: impl FnMut(Vec<u8>)) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if matches!(active.release, Some(SnapshotRelease::Cancel)) {
            active.unsent.clear();
            return;
        }
        while room() {
            let Some(frame) = active.unsent.pop_front() else {
                return;
            };
            send(frame);
        }
    }

    /// The release, once decided and (for a completion) once every frame
    /// has been fed. The capture then ends.
    fn take_release(&mut self) -> Option<(String, SnapshotRelease)> {
        let active = self.active.as_ref()?;
        let ready = match &active.release {
            Some(SnapshotRelease::Cancel) => true,
            Some(SnapshotRelease::Complete(_)) => active.unsent.is_empty(),
            None => false,
        };
        if !ready {
            return None;
        }
        let active = self.active.take().expect("an active capture");
        Some((
            active.request_id,
            active.release.expect("a decided release"),
        ))
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

fn send_metadata_observation(egress: &mut Egress, observation: TerminalMetadataObservation) {
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

fn encoded_frame_type(frame: &[u8]) -> Option<u8> {
    frame.get(4).copied()
}

fn is_process_exited_frame(frame: &[u8]) -> bool {
    encoded_frame_type(frame) == Some(FRAME_PROCESS_EXITED)
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

/// The stdio control link over the raw descriptors. Reads take exactly what
/// the handshake needs: a buffered reader could hold bytes that follow the
/// spawn frame, and the poll loop would never see them.
struct StdioControl {
    stdin: std::mem::ManuallyDrop<std::fs::File>,
    stdout: std::mem::ManuallyDrop<std::fs::File>,
}

impl StdioControl {
    fn new() -> Self {
        Self {
            stdin: std::mem::ManuallyDrop::new(stdio_file(0)),
            stdout: std::mem::ManuallyDrop::new(stdio_file(1)),
        }
    }
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

/// The first control link, which carries the handshake and the spawn.
enum InitialControl {
    Stdio,
    #[cfg(unix)]
    Socket(UnixStream),
}

impl InitialControl {
    /// A blocking stream for the handshake, before the poll loop starts.
    fn handshake_stream(&self) -> Result<Box<dyn ReadWrite + Send>, String> {
        match self {
            Self::Stdio => Ok(Box::new(StdioControl::new())),
            #[cfg(unix)]
            Self::Socket(stream) => Ok(Box::new(
                stream.try_clone().map_err(|error| error.to_string())?,
            )),
        }
    }
}

enum WorkerControl {
    Stdio,
    #[cfg(unix)]
    Socket {
        listener: UnixListener,
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

    fn accept_initial(&self) -> Result<InitialControl, String> {
        match self {
            Self::Stdio => Ok(InitialControl::Stdio),
            #[cfg(unix)]
            Self::Socket { listener, .. } => {
                let stream = listener.accept().map_err(|error| error.to_string())?.0;
                Ok(InitialControl::Socket(stream))
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
    test_pending_capacity: Option<usize>,
    test_hold_before_exit_gate: Option<PathBuf>,
    test_egress_full_signal: Option<PathBuf>,
    test_control_held_signal: Option<PathBuf>,
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
        let mut test_pending_capacity = None;
        let mut test_hold_before_exit_gate = None;
        let mut test_egress_full_signal = None;
        let mut test_control_held_signal = None;
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
                "--test-pending-capacity" => {
                    index += 1;
                    test_pending_capacity =
                        Some(parse_arg(&args, index, "--test-pending-capacity")?);
                }
                "--test-hold-before-exit-gate" => {
                    index += 1;
                    test_hold_before_exit_gate = Some(PathBuf::from(parse_string_arg(
                        &args,
                        index,
                        "--test-hold-before-exit-gate",
                    )?));
                }
                "--test-egress-full-signal" => {
                    index += 1;
                    test_egress_full_signal = Some(PathBuf::from(parse_string_arg(
                        &args,
                        index,
                        "--test-egress-full-signal",
                    )?));
                }
                "--test-control-held-signal" => {
                    index += 1;
                    test_control_held_signal = Some(PathBuf::from(parse_string_arg(
                        &args,
                        index,
                        "--test-control-held-signal",
                    )?));
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
            test_pending_capacity,
            test_hold_before_exit_gate,
            test_egress_full_signal,
            test_control_held_signal,
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
    use super::{
        Egress, EgressClass, FrameDecoder, SnapshotGate, SnapshotRelease, WorkerLifecycle,
    };

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

    fn title(text: &str) -> Vec<u8> {
        botster_core::encode_string(super::FRAME_TITLE_CHANGED, text).expect("title")
    }

    fn written(egress: &mut Egress) -> Vec<u8> {
        let mut out = Vec::new();
        egress
            .flush(|bytes| {
                out.extend_from_slice(bytes);
                Ok(bytes.len())
            })
            .expect("flush");
        out
    }

    #[test]
    fn egress_writes_queued_metadata_before_process_exited() {
        let mut egress = Egress::new(8);
        egress.metadata.push_back(title("late-title"));
        egress.push_protected(process_exited_frame());
        assert_eq!(
            decode_frame_types(&written(&mut egress)),
            vec![super::FRAME_TITLE_CHANGED, super::FRAME_PROCESS_EXITED]
        );
    }

    /// The exit does not end the egress: a capture answered from the final
    /// model after the exit is still written.
    #[test]
    fn egress_writes_metadata_then_the_exit_then_later_frames() {
        let mut egress = Egress::new(8);
        egress.push_protected(process_exited_frame());
        egress.push_protected(
            botster_core::encode_frame(super::FRAME_SNAPSHOT, b"after-exit").expect("snapshot"),
        );
        egress.metadata.push_back(title("after-exit-title"));
        assert_eq!(
            decode_frame_types(&written(&mut egress)),
            vec![
                super::FRAME_TITLE_CHANGED,
                super::FRAME_PROCESS_EXITED,
                super::FRAME_SNAPSHOT
            ]
        );
    }

    #[test]
    fn egress_writes_metadata_after_the_protected_frame_before_it() {
        let mut egress = Egress::new(8);
        egress.push_protected(
            botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"pty").expect("pty"),
        );
        egress.metadata.push_back(title("socket-title"));
        egress.push_protected(process_exited_frame());
        assert_eq!(
            decode_frame_types(&written(&mut egress)),
            vec![
                super::FRAME_PTY_OUTPUT,
                super::FRAME_TITLE_CHANGED,
                super::FRAME_PROCESS_EXITED
            ]
        );
    }

    /// A descriptor that takes a few bytes, then would block: the flush
    /// stops, and the next one resumes at the byte where it stopped.
    #[test]
    fn a_partial_write_resumes_where_it_stopped() {
        let mut egress = Egress::new(8);
        let first = botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"first").expect("frame");
        let second = botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"second").expect("frame");
        egress.push_protected(first.clone());
        egress.push_protected(second.clone());
        let mut out = Vec::new();
        let mut flushes = 0;
        while egress.wants_write() {
            flushes += 1;
            let mut budget = 3;
            egress
                .flush(|bytes| {
                    if budget == 0 {
                        return Err(std::io::ErrorKind::WouldBlock.into());
                    }
                    let take = bytes.len().min(budget);
                    budget -= take;
                    out.extend_from_slice(&bytes[..take]);
                    Ok(take)
                })
                .expect("flush");
        }
        assert!(flushes > 1, "the writes were split");
        assert_eq!(out, [first, second].concat());
        assert_eq!(egress.queued(EgressClass::Output), 0);
    }

    /// A new connection gets its welcome first, and never the rest of a
    /// frame that was half written to the old one.
    #[test]
    fn a_new_target_gets_its_welcome_and_no_half_frame() {
        let mut egress = Egress::new(8);
        let half = botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"half").expect("frame");
        let next = botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"next").expect("frame");
        egress.push_protected(half);
        egress.push_protected(next.clone());
        let mut written_once = false;
        egress
            .flush(|bytes| {
                if written_once {
                    return Err(std::io::ErrorKind::WouldBlock.into());
                }
                written_once = true;
                Ok(bytes.len() / 2)
            })
            .expect("flush");

        egress.retarget(b"WELCOME".to_vec());
        let out = written(&mut egress);
        assert_eq!(out, [b"WELCOME".to_vec(), next].concat());
        assert_eq!(egress.queued(EgressClass::Output), 0);
    }

    /// Each class has its own bound: output that fills the egress leaves
    /// room for replies, so control input is still read during a flood.
    #[test]
    fn full_output_leaves_room_for_replies() {
        let mut egress = Egress::new(2);
        egress.produce(EgressClass::Output);
        for _ in 0..2 {
            egress.push_protected(
                botster_core::encode_frame(super::FRAME_PTY_OUTPUT, b"flood").expect("frame"),
            );
        }
        assert!(!egress.has_room(EgressClass::Output), "output is held");
        assert!(
            egress.has_room(EgressClass::Reply),
            "a flood must not stop control input"
        );
        egress.produce(EgressClass::Reply);
        egress.push_protected(botster_core::encode_frame(super::FRAME_PONG, b"{}").expect("frame"));
        egress.push_protected(botster_core::encode_frame(super::FRAME_PONG, b"{}").expect("frame"));
        assert!(
            !egress.has_room(EgressClass::Reply),
            "replies are bounded too"
        );
        let _ = written(&mut egress);
        assert!(egress.has_room(EgressClass::Output) && egress.has_room(EgressClass::Reply));
    }

    #[test]
    fn a_full_metadata_lane_drops_and_reports_on_the_protected_lane() {
        let mut egress = Egress::new(1);
        egress.send_metadata_string(
            super::FRAME_TITLE_CHANGED,
            "kept",
            botster_core::TerminalMetadataKind::Title,
        );
        egress.send_metadata_string(
            super::FRAME_TITLE_CHANGED,
            "dropped",
            botster_core::TerminalMetadataKind::Title,
        );
        assert_eq!(egress.metadata.len(), 1);
        assert_eq!(
            decode_frame_types(&written(&mut egress)),
            vec![super::FRAME_METADATA_SHAPING, super::FRAME_TITLE_CHANGED]
        );
    }

    #[test]
    fn frames_split_across_reads_decode_whole() {
        let ping = botster_core::encode_frame(super::FRAME_PING, &[7u8; 40]).expect("frame");
        let mut decoder = FrameDecoder::default();
        decoder.push(&ping[..5]);
        assert!(matches!(decoder.next_frame(), Ok(None)));
        decoder.push(&ping[5..]);
        let frame = decoder.next_frame().expect("live").expect("a frame");
        assert_eq!(frame.frame_type, super::FRAME_PING);
        assert_eq!(frame.payload, vec![7u8; 40]);
        assert!(matches!(decoder.next_frame(), Ok(None)));
    }

    /// The parent's writer dies mid-frame, then the link ends: the half
    /// frame is the end, not a frame to wait for.
    #[test]
    fn a_truncated_frame_then_the_end_ends_the_link() {
        let ping = botster_core::encode_frame(super::FRAME_PING, &[0u8; 64]).expect("frame");
        let mut decoder = FrameDecoder::default();
        decoder.push(&ping[..ping.len() / 2]);
        assert!(matches!(decoder.next_frame(), Ok(None)));
        decoder.end();
        assert!(decoder.has_frame(), "the end is ready to take");
        assert!(decoder.next_frame().is_err());
    }

    #[test]
    fn an_invalid_frame_length_ends_the_link() {
        let mut decoder = FrameDecoder::default();
        decoder.push(&0u32.to_le_bytes());
        assert!(decoder.next_frame().is_err());
    }

    #[test]
    fn a_shutdown_or_link_end_cancels_an_active_capture() {
        let mut gate = SnapshotGate::default();
        gate.begin("snapshot-shutdown".to_string());
        gate.queue_frame(b"unsent".to_vec());
        gate.cancel_active();
        let mut fed = Vec::new();
        gate.feed(|| true, |frame| fed.push(frame));
        assert!(fed.is_empty(), "a cancelled capture sends nothing more");
        assert_eq!(
            gate.take_release(),
            Some(("snapshot-shutdown".to_string(), SnapshotRelease::Cancel))
        );
        assert!(!gate.is_active());
    }

    #[test]
    fn a_completion_waits_for_every_frame_and_carries_the_staged_resize() {
        let mut gate = SnapshotGate::default();
        assert!(
            !gate.stage_resize(botster_core::ResizePayload { rows: 1, cols: 1 }),
            "no capture: the resize applies at once"
        );
        gate.begin("snapshot".to_string());
        gate.queue_frame(b"one".to_vec());
        gate.queue_frame(b"two".to_vec());
        let size = botster_core::ResizePayload {
            rows: 40,
            cols: 120,
        };
        assert!(gate.stage_resize(size.clone()));
        gate.request_complete("snapshot");

        let mut fed = Vec::new();
        let mut room = 1;
        gate.feed(
            || {
                let has_room = room > 0;
                room -= 1;
                has_room
            },
            |frame| fed.push(frame),
        );
        assert_eq!(fed, vec![b"one".to_vec()]);
        assert_eq!(gate.take_release(), None, "a frame is still unsent");
        gate.feed(|| true, |frame| fed.push(frame));
        assert_eq!(
            gate.take_release(),
            Some((
                "snapshot".to_string(),
                SnapshotRelease::Complete(Some(size))
            ))
        );
    }

    #[test]
    fn a_new_capture_replaces_the_active_one_and_other_ids_are_ignored() {
        let mut gate = SnapshotGate::default();
        gate.begin("old".to_string());
        gate.queue_frame(b"old-frame".to_vec());
        gate.begin("new".to_string());
        gate.request_complete("old");
        gate.request_cancel("old");
        let mut fed = Vec::new();
        gate.feed(|| true, |frame| fed.push(frame));
        assert!(fed.is_empty(), "the replaced capture's frames are gone");
        assert_eq!(gate.take_release(), None);
        gate.request_complete("new");
        assert_eq!(
            gate.take_release(),
            Some(("new".to_string(), SnapshotRelease::Complete(None)))
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

    #[test]
    fn shutdown_keeps_worker_loop_alive_until_process_exit_is_observed() {
        let mut lifecycle = WorkerLifecycle::default();

        lifecycle.request_shutdown();
        assert!(lifecycle.should_continue());

        lifecycle.observe_process_exit();
        assert!(!lifecycle.should_continue());
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
