//! Thin operator CLI over the typed core daemon API.

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TransportEgress,
};
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig, SpawnSessionRequest};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let data_dir = data_dir_arg(&args)?;
    let command = command_arg(&args).unwrap_or("status");

    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(data_dir).with_worker_path(worker_path()?));
    match command {
        "start" | "status" => print_json(&daemon.status()?)?,
        "adopt" => {
            for report in daemon.adoption_scan()? {
                if matches!(
                    report.state,
                    botster_core_daemon::SessionAdoptionState::Adoptable
                ) {
                    let _ = daemon.adopt_session(&report.record.session_id, 1);
                }
            }
            print_json(&daemon.adoption_scan()?)?;
        }
        "smoke" => run_smoke(&mut daemon)?,
        _ => {
            return Err(format!(
                "unknown command: {command}; expected start, status, adopt, or smoke"
            )
            .into())
        }
    }
    Ok(())
}

fn worker_path() -> Result<PathBuf, Box<dyn Error>> {
    let current = env::current_exe()?;
    let dir = current
        .parent()
        .ok_or("daemon executable should have a parent directory")?;
    Ok(dir.join("botster-session-worker"))
}

fn data_dir_arg(args: &[String]) -> Result<String, Box<dyn Error>> {
    for index in 0..args.len() {
        if args[index] == "--data-dir" {
            return args
                .get(index + 1)
                .cloned()
                .ok_or_else(|| "--data-dir requires a path".into());
        }
        if let Some(path) = args[index].strip_prefix("--data-dir=") {
            return Ok(path.to_string());
        }
    }
    Err("explicit --data-dir is required".into())
}

fn command_arg(args: &[String]) -> Option<&str> {
    let mut skip_next = false;
    for argument in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if argument == "--data-dir" {
            skip_next = true;
            continue;
        }
        if argument.starts_with("--data-dir=") {
            continue;
        }
        return Some(argument);
    }
    None
}

fn run_smoke(daemon: &mut CoreDaemon) -> Result<(), Box<dyn Error>> {
    let session_id = SessionId("daemon-cli-smoke-session".to_string());
    let client_id = ClientId("daemon-cli-smoke-client".to_string());
    let subscription_id = SubscriptionId("daemon-cli-smoke-subscription".to_string());
    daemon.spawn(
        SpawnSessionRequest {
            request: SessionSpawnRequest {
                request_id: RequestId("daemon-cli-smoke-spawn".to_string()),
                session_id: session_id.clone(),
                executable: "sh".to_string(),
                arguments: vec![
                    "-c".to_string(),
                    "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
                        .to_string(),
                ],
                working_directory: SpawnWorkingDirectory {
                    path: ".".to_string(),
                },
                environment: SpawnEnvironment::default(),
                initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
            },
            metadata: CoreSessionMetadata::new(),
        },
        1,
    )?;
    daemon.attach(client_id.clone(), session_id.clone(), subscription_id, 2)?;
    daemon.input(client_id.clone(), session_id.clone(), b"ping\n".to_vec(), 3)?;
    daemon.resize(client_id, session_id.clone(), 30, 100, 4)?;
    let drained = daemon.drain(&session_id, 5)?;
    daemon.shutdown(Some(session_id.clone()), 6)?;

    let output = drained
        .client_egress
        .iter()
        .filter_map(|(_, frame)| match frame {
            TransportEgress::TerminalOutput { data, .. } => {
                Some(String::from_utf8_lossy(data).to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    println!("botster-core-daemon smoke");
    println!("session: {}", session_id.0);
    println!("output: {output:?}");
    println!("shutdown: true");
    Ok(())
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
