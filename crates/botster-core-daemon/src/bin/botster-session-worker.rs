//! Local session worker process entrypoint.
//!
//! Hosted by `botster-core-daemon` so the worker can depend on
//! `botster-terminal-ghostty` without a Cargo cycle through package
//! `botster-core`. This process owns worker-local Ghostty mode state and the
//! atomic mode-gated PTY input admit barrier.

use std::collections::HashMap;
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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use botster_core::engine::TerminalScreenRuntime;
use botster_core::{
    read_hello, write_welcome, Frame, LocalProcessRuntime, LocalProcessRuntimeOptions, ModeFlags,
    ModeFlagsPayload, ModeFreshnessToken, ModeGatedCancelRequest, ModeGatedPtyInputRequest,
    ModeGatedPtyInputResult, ResizePayload, SessionMetadata, SessionRuntime, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, TerminalMetadataKind, TerminalMetadataLaneShaper,
    TerminalMetadataObservation, TerminalMetadataProducer, TerminalMetadataShapingObservation,
    TerminalMetadataShapingOutcome, TerminalScreenSize, TimeoutPayload, WorkerHealth,
    WorkerSnapshotPhase, WorkerSnapshotRequest, WorkerSnapshotResult, FRAME_BELL,
    FRAME_CWD_CHANGED, FRAME_GET_MODE_FLAGS, FRAME_GET_SNAPSHOT, FRAME_METADATA_SHAPING,
    FRAME_MODE_FLAGS, FRAME_MODE_GATED_CANCEL, FRAME_MODE_GATED_PTY_INPUT,
    FRAME_MODE_GATED_PTY_INPUT_RESULT, FRAME_NOTIFICATION, FRAME_PING, FRAME_PONG,
    FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE,
    FRAME_SET_TIMEOUT, FRAME_SHUTDOWN, FRAME_SNAPSHOT, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED,
};
use botster_terminal_ghostty::{
    GhosttyAdapterConfig, GhosttySnapshotFrameKind, GhosttyTerminal, GHOSTTY_SNAPSHOT_FORMAT,
};

const LOOP_SLEEP: Duration = Duration::from_millis(10);

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

    let _peer_version = read_hello(&mut initial_control).map_err(|error| error.to_string())?;
    let spawn_frame = read_frame(&mut initial_control)?;
    if spawn_frame.frame_type != FRAME_SPAWN_SESSION {
        return Err("worker expected FRAME_SPAWN_SESSION after hello".to_string());
    }
    let spawn_request: SessionSpawnRequest =
        serde_json::from_slice(&spawn_frame.payload).map_err(|error| error.to_string())?;
    let session_id = spawn_request.session_id.clone();

    let runtime_options = LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(args.shutdown_grace_ms),
        poll_interval: Duration::from_millis(args.poll_interval_ms),
        pty_reader_chunk_capacity: args.pty_reader_chunk_capacity,
        test_hold_after_read_ms: args.test_hold_after_read_ms,
        test_write_block_until_unix_ms: args.test_write_block_until_unix_ms,
        test_write_max_chunk: args.test_write_max_chunk,
        test_pending_capacity: args.test_pending_capacity,
        test_hold_after_enqueue_ms: args.test_hold_after_enqueue_ms,
        test_fail_pty_writes: false,
    };
    let mut runtime = LocalProcessRuntime::with_options(runtime_options);
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
    let mut mode_owner = WorkerModeOwner::new(
        ghostty
            .mode_flags()
            .map_err(|error| format!("worker initial mode flags failed: {error}"))?,
    );
    let metadata = SessionMetadata {
        session_uuid: session_id.0.clone(),
        pid: handle.process.pid.unwrap_or_else(process::id),
        rows: initial_rows,
        cols: initial_cols,
        last_output_at: 0,
        title: None,
        cwd: None,
        port: None,
        mode_flags: mode_owner.mode_flags.clone(),
        recovery_identity: Some(serde_json::json!({
            "session_uuid": session_id.0,
            "runtime_id": handle.process.runtime_id,
            "worker_pid": process::id(),
            "worker_control_socket": args.control_socket,
            "mode_generation": mode_owner.token().mode_generation,
            "atomic_snapshot_boundary": true,
            "snapshot_delivery": "ready_then_history",
        })),
    };
    write_welcome(&mut initial_control, &metadata).map_err(|error| error.to_string())?;

    let (frame_sender, frame_receiver) = mpsc::channel();
    let snapshot_barrier = Arc::new(SnapshotBarrierControl::default());
    let cancel_cell = Arc::new(Mutex::new(None::<String>));
    control.spawn_readers(
        initial_control,
        frame_sender,
        Arc::clone(&snapshot_barrier),
        Arc::clone(&cancel_cell),
    );

    let (egress, protected_receiver, metadata_receiver) =
        WorkerEgress::new(args.egress_capacity.max(1));
    let writer = control.spawn_writer(protected_receiver, metadata_receiver);
    let mut metadata_producer = TerminalMetadataProducer::new();
    let mut metadata_shaper = TerminalMetadataLaneShaper::new(
        (args.egress_capacity / 2).max(1),
        args.egress_capacity.saturating_mul(4).max(1),
    );
    let mut reconnect_timeout_seconds = None;
    let mut lifecycle = WorkerLifecycle::default();

    while lifecycle.should_continue() {
        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => match frame.frame_type {
                    FRAME_PTY_INPUT => runtime
                        .send_input(SessionRuntimeInput::PtyInput {
                            session_id: handle.session_id.clone(),
                            data: frame.payload,
                        })
                        .map_err(|error| error.to_string())?,
                    FRAME_MODE_GATED_PTY_INPUT => {
                        // Process one gated admit fully before the next frame.
                        // Parent also rejects concurrent gated waits per session.
                        let result = match serde_json::from_slice::<ModeGatedPtyInputRequest>(
                            &frame.payload,
                        ) {
                            Ok(request) => atomic_mode_gated_admit(
                                &mut runtime,
                                &handle.session_id,
                                &mut ghostty,
                                &mut mode_owner,
                                &mut metadata_producer,
                                &mut metadata_shaper,
                                &egress,
                                &cancel_cell,
                                request,
                            ),
                            Err(error) => ModeGatedPtyInputResult {
                                request_id: String::new(),
                                admitted: false,
                                bytes_written: 0,
                                mode_flags: mode_owner.mode_flags.clone(),
                                mode_freshness: mode_owner.token(),
                                error_kind: Some(format!("malformed request: {error}")),
                            },
                        };
                        egress.send_protected_json(FRAME_MODE_GATED_PTY_INPUT_RESULT, &result);
                    }
                    FRAME_GET_MODE_FLAGS => {
                        let probe_request_id =
                            serde_json::from_slice::<serde_json::Value>(&frame.payload)
                                .ok()
                                .and_then(|value| {
                                    value
                                        .get("request_id")
                                        .and_then(|id| id.as_str())
                                        .map(str::to_owned)
                                })
                                .unwrap_or_default();
                        // Probe under the same reader fence so returned modes
                        // cannot race with later unapplied PTY output.
                        match runtime.with_pty_io_barrier(&handle.session_id, |barrier| {
                            apply_barrier_outputs(
                                barrier,
                                &mut ghostty,
                                &mut mode_owner,
                                &mut metadata_producer,
                                &mut metadata_shaper,
                                &egress,
                            )
                        }) {
                            Ok(()) => {
                                egress.send_protected_json(
                                    FRAME_MODE_FLAGS,
                                    &ModeFlagsPayload {
                                        request_id: probe_request_id,
                                        mode_flags: mode_owner.mode_flags.clone(),
                                        mode_freshness: mode_owner.token(),
                                        error_kind: None,
                                    },
                                );
                            }
                            Err(error) => {
                                // Fail closed: correlated explicit probe failure,
                                // not a successful token after a drain error.
                                egress.send_protected_json(
                                    FRAME_MODE_FLAGS,
                                    &ModeFlagsPayload {
                                        request_id: probe_request_id,
                                        mode_flags: mode_owner.mode_flags.clone(),
                                        mode_freshness: mode_owner.token(),
                                        error_kind: Some(error.to_string()),
                                    },
                                );
                            }
                        }
                    }
                    FRAME_RESIZE => {
                        let size: ResizePayload = serde_json::from_slice(&frame.payload)
                            .map_err(|error| error.to_string())?;
                        ghostty.resize(TerminalScreenSize::new(size.rows, size.cols));
                        runtime
                            .send_input(SessionRuntimeInput::Resize {
                                session_id: handle.session_id.clone(),
                                size,
                            })
                            .map_err(|error| error.to_string())?;
                    }
                    FRAME_GET_SNAPSHOT => {
                        let request =
                            serde_json::from_slice::<WorkerSnapshotRequest>(&frame.payload);
                        match request {
                            Ok(request) => {
                                let request_id = request.request_id;
                                let barrier_control = Arc::clone(&snapshot_barrier);
                                let result = runtime.with_pty_io_barrier(&handle.session_id, |barrier| {
                                    let encoded = (|| {
                                        apply_barrier_outputs(
                                            barrier,
                                            &mut ghostty,
                                            &mut mode_owner,
                                            &mut metadata_producer,
                                            &mut metadata_shaper,
                                            &egress,
                                        )?;
                                        let size = ghostty.size();
                                        ghostty.export_snapshot_frames(|frame| {
                                            let phase = match frame.kind {
                                                GhosttySnapshotFrameKind::Ready => WorkerSnapshotPhase::Ready,
                                                GhosttySnapshotFrameKind::History => {
                                                    WorkerSnapshotPhase::History
                                                }
                                                GhosttySnapshotFrameKind::Finish => {
                                                    WorkerSnapshotPhase::Finish
                                                }
                                            };
                                            if args.test_fail_snapshot_history_after_ready
                                                && phase == WorkerSnapshotPhase::History
                                            {
                                                return false;
                                            }
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
                                                },
                                                || barrier_control.is_cancelled(&request_id),
                                            )
                                        }).map_err(|error| {
                                            botster_core::SessionRuntimeError::new(
                                                botster_core::SessionRuntimeErrorKind::OutputFailed,
                                                error.to_string(),
                                            )
                                        })
                                    })();
                                    if let Err(error) = encoded {
                                        let _ = egress.send_protected_json_cancellable(
                                            FRAME_SNAPSHOT,
                                            &WorkerSnapshotResult {
                                                request_id: request_id.clone(),
                                                snapshot: None,
                                                phase: None,
                                                error_kind: Some(error.to_string()),
                                                barrier_released: false,
                                            },
                                            || barrier_control.is_cancelled(&request_id),
                                        );
                                    }
                                    match barrier_control.wait_for_release(&request_id) {
                                        SnapshotBarrierRelease::Cancel => return Ok(()),
                                        SnapshotBarrierRelease::Complete(resize) => {
                                            let release_error = if let Some(size) = resize {
                                                ghostty.resize(TerminalScreenSize::new(size.rows, size.cols));
                                                barrier.resize(size).err().map(|error| error.to_string())
                                            } else {
                                                None
                                            };
                                            let _ = egress.send_protected_json(
                                                FRAME_SNAPSHOT,
                                                &WorkerSnapshotResult {
                                                    request_id: request_id.clone(),
                                                    snapshot: None,
                                                    phase: None,
                                                    error_kind: release_error,
                                                    barrier_released: true,
                                                },
                                            );
                                        }
                                    }
                                    Ok(())
                                });
                                if let Err(error) = result {
                                    let _ = egress.send_protected_json(
                                        FRAME_SNAPSHOT,
                                        &WorkerSnapshotResult {
                                            request_id: request_id.clone(),
                                            snapshot: None,
                                            phase: None,
                                            error_kind: Some(error.to_string()),
                                            barrier_released: false,
                                        },
                                    );
                                }
                                snapshot_barrier.clear(&request_id);
                            }
                            Err(error) => {
                                let _ = egress.send_protected_json(
                                    FRAME_SNAPSHOT,
                                    &WorkerSnapshotResult {
                                        request_id: String::new(),
                                        snapshot: None,
                                        phase: None,
                                        error_kind: Some(format!(
                                            "malformed snapshot request: {error}"
                                        )),
                                        barrier_released: false,
                                    },
                                );
                            }
                        }
                    }
                    FRAME_PING => {
                        let health = WorkerHealth {
                            session_id: handle.session_id.clone(),
                            worker_pid: process::id(),
                            reconnect_timeout_seconds,
                        };
                        egress.send_protected_json(FRAME_PONG, &health);
                    }
                    FRAME_SET_TIMEOUT => {
                        let timeout: TimeoutPayload = serde_json::from_slice(&frame.payload)
                            .map_err(|error| error.to_string())?;
                        reconnect_timeout_seconds = Some(timeout.seconds);
                    }
                    FRAME_SHUTDOWN => {
                        runtime
                            .send_input(SessionRuntimeInput::Shutdown {
                                session_id: handle.session_id.clone(),
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
                                session_id: handle.session_id.clone(),
                            })
                            .map_err(|error| error.to_string())?;
                        lifecycle.request_shutdown();
                    }
                    break;
                }
            }
        }

        let exited = drain_and_apply_pty_output(
            &mut runtime,
            &handle.session_id,
            &mut ghostty,
            &mut mode_owner,
            &mut metadata_producer,
            &mut metadata_shaper,
            &egress,
        )?;
        if exited {
            lifecycle.observe_process_exit();
        }

        thread::sleep(LOOP_SLEEP);
    }

    if let Some(hold_ms) = args.test_hold_before_exit_ms {
        thread::sleep(Duration::from_millis(hold_ms));
    }

    drop(egress);
    writer
        .join()
        .map_err(|_| "worker egress writer panicked".to_string())??;
    if let Some(exit_code) = args.test_exit_code {
        process::exit(exit_code);
    }
    Ok(())
}

struct WorkerModeOwner {
    generation: u64,
    revision: u64,
    mode_flags: ModeFlags,
}

impl WorkerModeOwner {
    fn new(initial: ModeFlags) -> Self {
        Self {
            generation: new_mode_generation(),
            revision: 1,
            mode_flags: initial,
        }
    }

    fn token(&self) -> ModeFreshnessToken {
        ModeFreshnessToken {
            mode_generation: self.generation,
            mode_revision: self.revision,
        }
    }

    fn observe(&mut self, mode_flags: ModeFlags) {
        if mode_flags != self.mode_flags {
            if self.revision < u64::MAX {
                self.revision = self.revision.saturating_add(1);
            }
            self.mode_flags = mode_flags;
        }
    }
}

/// Allocate a process-local mode generation token that is safe to round-trip
/// through JSON numbers used by browser clients (`Number.MAX_SAFE_INTEGER` =
/// 2^53 - 1). Wall-clock nanos / pointer mixing previously produced full `u64`
/// values that browsers silently corrupted, breaking ModeGatedInput admission.
fn new_mode_generation() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Largest integer that every IEEE-754 binary64 JSON number can represent
    /// exactly. Browser `Number` and `JSON.parse` share this bound.
    const JSON_SAFE_INTEGER_MAX: u64 = (1u64 << 53) - 1;

    static NEXT: AtomicU64 = AtomicU64::new(1);
    // Keep the token in `(1 ..= JSON_SAFE_INTEGER_MAX)` so Web clients can
    // ModeGatedInput without a send_input fallback.
    let next = NEXT.fetch_add(1, Ordering::Relaxed);
    (next % JSON_SAFE_INTEGER_MAX).max(1)
}

fn apply_pty_output_chunk(
    ghostty: &mut GhosttyTerminal,
    mode_owner: &mut WorkerModeOwner,
    metadata_producer: &mut TerminalMetadataProducer,
    metadata_shaper: &mut TerminalMetadataLaneShaper,
    egress: &WorkerEgress,
    data: Vec<u8>,
) {
    let observations = metadata_producer.observe(&data);
    ghostty.write_output(&data);
    // Do not inject Ghostty write_pty replies here: the parent dual-shadow still
    // owns OSC write_pty injection for worker-backed sessions. Worker Ghostty is
    // the mode-token authority only.
    if let Ok(flags) = ghostty.mode_flags() {
        mode_owner.observe(flags);
    }
    egress.send_protected_frame(FRAME_PTY_OUTPUT, data);
    let mut shaping_reports = MetadataShapingReportAccumulator::default();
    for observation in observations {
        for shaping in metadata_shaper.push(observation) {
            shaping_reports.record(shaping);
        }
    }
    for observation in metadata_shaper.drain() {
        send_metadata_observation(egress, observation);
    }
    for shaping in shaping_reports.into_reports() {
        egress.send_protected_json(FRAME_METADATA_SHAPING, &shaping);
    }
}

fn drain_and_apply_pty_output(
    runtime: &mut LocalProcessRuntime,
    session_id: &botster_core::SessionId,
    ghostty: &mut GhosttyTerminal,
    mode_owner: &mut WorkerModeOwner,
    metadata_producer: &mut TerminalMetadataProducer,
    metadata_shaper: &mut TerminalMetadataLaneShaper,
    egress: &WorkerEgress,
) -> Result<bool, String> {
    let mut process_exited = false;
    for output in runtime
        .drain_output(session_id)
        .map_err(|error| error.to_string())?
    {
        match output {
            SessionRuntimeOutput::PtyOutput { data, .. } => {
                apply_pty_output_chunk(
                    ghostty,
                    mode_owner,
                    metadata_producer,
                    metadata_shaper,
                    egress,
                    data,
                );
            }
            SessionRuntimeOutput::ProcessExited { payload, .. } => {
                for observation in metadata_shaper.drain() {
                    send_metadata_observation(egress, observation);
                }
                egress.send_protected_json(FRAME_PROCESS_EXITED, &payload);
                process_exited = true;
            }
            SessionRuntimeOutput::Backpressure(_) => {}
            SessionRuntimeOutput::TitleChanged { .. }
            | SessionRuntimeOutput::CwdChanged { .. }
            | SessionRuntimeOutput::PromptMark { .. }
            | SessionRuntimeOutput::Bell { .. }
            | SessionRuntimeOutput::Notification { .. }
            | SessionRuntimeOutput::MetadataShaping(_) => {}
        }
    }
    Ok(process_exited)
}

#[allow(clippy::too_many_arguments)]
fn atomic_mode_gated_admit(
    runtime: &mut LocalProcessRuntime,
    session_id: &botster_core::SessionId,
    ghostty: &mut GhosttyTerminal,
    mode_owner: &mut WorkerModeOwner,
    metadata_producer: &mut TerminalMetadataProducer,
    metadata_shaper: &mut TerminalMetadataLaneShaper,
    egress: &WorkerEgress,
    cancel_cell: &Arc<Mutex<Option<String>>>,
    request: ModeGatedPtyInputRequest,
) -> ModeGatedPtyInputResult {
    struct ClearCancel<'a>(&'a Arc<Mutex<Option<String>>>);
    impl Drop for ClearCancel<'_> {
        fn drop(&mut self) {
            *self.0.lock().unwrap_or_else(|error| error.into_inner()) = None;
        }
    }
    let _clear = ClearCancel(cancel_cell);
    let request_id = request.request_id.clone();
    let expected = request.expected;
    let data = request.data;
    let deadline_unix_ms = request.deadline_unix_ms;
    let test_hold_ms = request.test_hold_ms.unwrap_or(0);

    let outcome = runtime.with_pty_io_barrier(session_id, |barrier| {
        // Optional deterministic hold while the reader is paused so mode-changing
        // output can accumulate in the OS PTY buffer after the first drain.
        if test_hold_ms > 0 {
            // First drain empties the pre-hold queue so the hold window is exact.
            apply_barrier_outputs(
                barrier,
                ghostty,
                mode_owner,
                metadata_producer,
                metadata_shaper,
                egress,
            )?;
            thread::sleep(Duration::from_millis(test_hold_ms));
        }

        // Drain/apply every pre-barrier byte with the reader paused.
        apply_barrier_outputs(
            barrier,
            ghostty,
            mode_owner,
            metadata_producer,
            metadata_shaper,
            egress,
        )?;

        let current = mode_owner.token();
        if mode_owner.revision == u64::MAX {
            return Ok(ModeGatedPtyInputResult {
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: 0,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: Some("revision_overflow".to_string()),
            });
        }
        // Fail closed at or after the parent deadline (inclusive).
        if unix_now_ms() >= deadline_unix_ms {
            return Ok(ModeGatedPtyInputResult {
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: 0,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: Some("deadline_exceeded".to_string()),
            });
        }
        if expected != current {
            return Ok(ModeGatedPtyInputResult {
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: 0,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: None,
            });
        }
        let cancelled = {
            let mut cell = cancel_cell
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let matched = cell.as_ref() == Some(&request_id);
            *cell = None;
            matched
        };
        if cancelled {
            return Ok(ModeGatedPtyInputResult {
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: 0,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: Some("cancelled".to_string()),
            });
        }
        // Bound the complete write, including WouldBlock retries.
        let data_len = data.len();
        match barrier.write_input(&data, Some(deadline_unix_ms)) {
            Ok(written) if written == data_len => Ok(ModeGatedPtyInputResult {
                request_id: request_id.clone(),
                admitted: true,
                bytes_written: written,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: None,
            }),
            Ok(written) => Ok(ModeGatedPtyInputResult {
                // Should not happen: write_all returns Ok only for complete.
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: written,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: Some("partial_write".to_string()),
            }),
            Err(error) if error.bytes_written == 0 => Ok(ModeGatedPtyInputResult {
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: 0,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: Some(error.message),
            }),
            Err(error) => Ok(ModeGatedPtyInputResult {
                // Explicit partial delivery: callers must check bytes_written.
                request_id: request_id.clone(),
                admitted: false,
                bytes_written: error.bytes_written,
                mode_flags: mode_owner.mode_flags.clone(),
                mode_freshness: current,
                error_kind: Some(format!("partial_write:{}", error.message)),
            }),
        }
    });

    match outcome {
        Ok(result) => result,
        Err(error) => ModeGatedPtyInputResult {
            request_id,
            admitted: false,
            bytes_written: 0,
            mode_flags: mode_owner.mode_flags.clone(),
            mode_freshness: mode_owner.token(),
            error_kind: Some(error.to_string()),
        },
    }
}

fn apply_barrier_outputs(
    barrier: &mut botster_core::PtyIoBarrier<'_>,
    ghostty: &mut GhosttyTerminal,
    mode_owner: &mut WorkerModeOwner,
    metadata_producer: &mut TerminalMetadataProducer,
    metadata_shaper: &mut TerminalMetadataLaneShaper,
    egress: &WorkerEgress,
) -> Result<(), botster_core::SessionRuntimeError> {
    // Apply retained output first, then fail closed on sticky authority so the
    // first post-overflow probe/admit cannot succeed after incomplete modes.
    let outputs = barrier.drain_output()?;
    for output in outputs {
        match output {
            SessionRuntimeOutput::PtyOutput { data, .. } => {
                apply_pty_output_chunk(
                    ghostty,
                    mode_owner,
                    metadata_producer,
                    metadata_shaper,
                    egress,
                    data,
                );
            }
            SessionRuntimeOutput::ProcessExited { payload, .. } => {
                for observation in metadata_shaper.drain() {
                    send_metadata_observation(egress, observation);
                }
                egress.send_protected_json(FRAME_PROCESS_EXITED, &payload);
            }
            SessionRuntimeOutput::Backpressure(_) => {}
            SessionRuntimeOutput::TitleChanged { .. }
            | SessionRuntimeOutput::CwdChanged { .. }
            | SessionRuntimeOutput::PromptMark { .. }
            | SessionRuntimeOutput::Bell { .. }
            | SessionRuntimeOutput::Notification { .. }
            | SessionRuntimeOutput::MetadataShaping(_) => {}
        }
    }
    barrier.ensure_mode_authority()?;
    Ok(())
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
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
    cancel_cell: Arc<Mutex<Option<String>>>,
) {
    thread::spawn(move || {
        while let Ok(frame) = read_frame(&mut control) {
            if frame.frame_type == FRAME_MODE_GATED_CANCEL {
                if let Ok(request) =
                    serde_json::from_slice::<ModeGatedCancelRequest>(&frame.payload)
                {
                    *cancel_cell
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = Some(request.request_id);
                }
                continue;
            }
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
        }
        snapshot_barrier.cancel_active();
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
        cancel_cell: Arc<Mutex<Option<String>>>,
    ) {
        spawn_control_reader(
            initial,
            sender.clone(),
            Arc::clone(&snapshot_barrier),
            Arc::clone(&cancel_cell),
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
                    if read_hello(&mut stream).is_err() {
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
                        Arc::clone(&cancel_cell),
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
    test_fail_snapshot_history_after_ready: bool,
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
        let mut test_fail_snapshot_history_after_ready = false;
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
                "--test-fail-snapshot-history-after-ready" => {
                    test_fail_snapshot_history_after_ready = true;
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
            test_fail_snapshot_history_after_ready,
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

    #[test]
    fn mode_generation_tokens_are_json_safe_integers() {
        use super::new_mode_generation;

        // Browser JSON numbers only preserve integers up to 2^53 - 1 exactly.
        const JSON_SAFE_INTEGER_MAX: u64 = (1u64 << 53) - 1;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let generation = new_mode_generation();
            assert!(generation >= 1, "generation must be non-zero");
            assert!(
                generation <= JSON_SAFE_INTEGER_MAX,
                "generation {generation} exceeds JSON-safe integer max"
            );
            // Round-trip through serde_json number must preserve equality.
            let encoded = serde_json::to_string(&generation).expect("encode");
            let decoded: u64 = serde_json::from_str(&encoded).expect("decode");
            assert_eq!(decoded, generation);
            // Also prove f64 JSON parse path used by browsers would match.
            let as_f64 = encoded.parse::<f64>().expect("parse f64");
            assert_eq!(as_f64 as u64, generation);
            seen.insert(generation);
        }
        assert!(seen.len() > 1, "tokens must advance");
    }
}
