//! Local session worker process entrypoint.

use std::io::{self, Read, Write};
use std::process;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::Duration;

use botster_core::{
    read_hello, write_welcome, Frame, LocalProcessRuntime, LocalProcessRuntimeOptions,
    ResizePayload, SessionMetadata, SessionRuntime, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest, TimeoutPayload, WorkerHealth, FRAME_PING, FRAME_PONG,
    FRAME_PROCESS_EXITED, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE, FRAME_SET_TIMEOUT,
    FRAME_SHUTDOWN, FRAME_SPAWN_SESSION,
};

const LOOP_SLEEP: Duration = Duration::from_millis(10);

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr(), "botster-session-worker failed: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = WorkerArgs::parse(std::env::args().skip(1).collect())?;
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    let _peer_version = read_hello(&mut stdin).map_err(|error| error.to_string())?;
    let spawn_frame = read_frame(&mut stdin)?;
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
    };
    let mut runtime = LocalProcessRuntime::with_options(runtime_options);
    let handle = runtime
        .spawn_session(spawn_request)
        .map_err(|error| error.to_string())?;
    let metadata = SessionMetadata {
        session_uuid: session_id.0.clone(),
        pid: handle.process.pid.unwrap_or_else(process::id),
        rows: 24,
        cols: 80,
        last_output_at: 0,
        title: None,
        cwd: None,
        port: None,
        mode_flags: Default::default(),
        recovery_identity: Some(serde_json::json!({
            "session_uuid": session_id.0,
            "runtime_id": handle.process.runtime_id,
            "worker_pid": process::id(),
        })),
    };
    write_welcome(&mut stdout, &metadata).map_err(|error| error.to_string())?;

    let (frame_sender, frame_receiver) = mpsc::channel();
    spawn_stdin_reader(stdin, frame_sender);

    let (egress_sender, egress_receiver) = mpsc::sync_channel(args.egress_capacity.max(1));
    let writer = thread::spawn(move || write_egress(stdout, egress_receiver));
    let mut reconnect_timeout_seconds = None;
    let mut shutdown_requested = false;

    while !shutdown_requested {
        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => match frame.frame_type {
                    FRAME_PTY_INPUT => runtime
                        .send_input(SessionRuntimeInput::PtyInput {
                            session_id: handle.session_id.clone(),
                            data: frame.payload,
                        })
                        .map_err(|error| error.to_string())?,
                    FRAME_RESIZE => {
                        let size: ResizePayload = serde_json::from_slice(&frame.payload)
                            .map_err(|error| error.to_string())?;
                        runtime
                            .send_input(SessionRuntimeInput::Resize {
                                session_id: handle.session_id.clone(),
                                size,
                            })
                            .map_err(|error| error.to_string())?;
                    }
                    FRAME_PING => {
                        let health = WorkerHealth {
                            session_id: handle.session_id.clone(),
                            worker_pid: process::id(),
                            reconnect_timeout_seconds,
                        };
                        send_json(&egress_sender, FRAME_PONG, &health);
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
                        shutdown_requested = true;
                    }
                    _ => {}
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    runtime
                        .send_input(SessionRuntimeInput::Shutdown {
                            session_id: handle.session_id.clone(),
                        })
                        .map_err(|error| error.to_string())?;
                    shutdown_requested = true;
                    break;
                }
            }
        }

        for output in runtime
            .drain_output(&handle.session_id)
            .map_err(|error| error.to_string())?
        {
            match output {
                SessionRuntimeOutput::PtyOutput { data, .. } => {
                    send_frame(&egress_sender, FRAME_PTY_OUTPUT, data);
                }
                SessionRuntimeOutput::ProcessExited { payload, .. } => {
                    send_json(&egress_sender, FRAME_PROCESS_EXITED, &payload);
                    shutdown_requested = true;
                }
                SessionRuntimeOutput::Backpressure(_) => {}
            }
        }

        thread::sleep(LOOP_SLEEP);
    }

    drop(egress_sender);
    writer
        .join()
        .map_err(|_| "worker egress writer panicked".to_string())??;
    Ok(())
}

fn spawn_stdin_reader(mut stdin: impl Read + Send + 'static, sender: mpsc::Sender<Frame>) {
    thread::spawn(move || {
        while let Ok(frame) = read_frame(&mut stdin) {
            if sender.send(frame).is_err() {
                break;
            }
        }
    });
}

fn write_egress(mut stdout: impl Write, receiver: Receiver<Vec<u8>>) -> Result<(), String> {
    while let Ok(frame) = receiver.recv() {
        stdout
            .write_all(&frame)
            .map_err(|error| error.to_string())?;
        stdout.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn send_frame(sender: &SyncSender<Vec<u8>>, frame_type: u8, payload: Vec<u8>) {
    if let Ok(frame) = botster_core::encode_frame(frame_type, &payload) {
        let _ = sender.try_send(frame).or_else(|error| match error {
            TrySendError::Full(_) => Ok(()),
            TrySendError::Disconnected(_) => Err(()),
        });
    }
}

fn send_json<T: serde::Serialize>(sender: &SyncSender<Vec<u8>>, frame_type: u8, payload: &T) {
    if let Ok(frame) = botster_core::encode_json(frame_type, payload) {
        let _ = sender.try_send(frame).or_else(|error| match error {
            TrySendError::Full(_) => Ok(()),
            TrySendError::Disconnected(_) => Err(()),
        });
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
}

impl WorkerArgs {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut egress_capacity = 64;
        let mut pty_reader_chunk_capacity = botster_core::DEFAULT_PTY_READER_CHUNK_CAPACITY;
        let mut shutdown_grace_ms = 500;
        let mut poll_interval_ms = 10;
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
                other => return Err(format!("unknown worker argument: {other}")),
            }
            index += 1;
        }

        Ok(Self {
            egress_capacity,
            pty_reader_chunk_capacity,
            shutdown_grace_ms,
            poll_interval_ms,
        })
    }
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
