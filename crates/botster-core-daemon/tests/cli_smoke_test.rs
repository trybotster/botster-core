#![allow(missing_docs)]

use std::process::Command;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use botster_core::{ProcessIdentity, ResizePayload, SessionId};
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig, RegistryRecord};

#[cfg(unix)]
#[test]
fn daemon_cli_smoke_starts_inspects_and_uses_session() {
    let data_dir = temp_data_dir("daemon-cli");
    build_worker_binary();
    let binary = env!("CARGO_BIN_EXE_botster-core-daemon");

    let status = Command::new(binary)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("start")
        .output()
        .expect("daemon CLI start/status should run");
    assert!(
        status.status.success(),
        "start command failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("\"running\": true"));

    let smoke = Command::new(binary)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("smoke")
        .output()
        .expect("daemon CLI smoke should run");
    assert!(
        smoke.status.success(),
        "smoke command failed: {}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    let stdout = String::from_utf8_lossy(&smoke.stdout);
    assert!(stdout.contains("botster-core-daemon smoke"));
    assert!(stdout.contains("daemon-cli-smoke-session"));
    assert!(stdout.contains("shutdown: true"));

    let _ = std::fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_cli_adopt_exits_nonzero_when_worker_adoption_fails() {
    let data_dir = temp_data_dir("daemon-cli-adopt-failure");
    build_worker_binary();
    let binary = env!("CARGO_BIN_EXE_botster-core-daemon");
    let session_id = SessionId("daemon-cli-adopt-failure-session".to_string());
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let mut record = RegistryRecord::running(
        session_id.clone(),
        Some(ProcessIdentity {
            pid: Some(42),
            runtime_id: Some("daemon-cli-adopt-failure-runtime".to_string()),
        }),
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        10,
    );
    record.observe_restart_contract(
        serde_json::json!({
            "session": session_id.0,
            "worker_control_socket": data_dir.join("missing-worker.sock"),
        }),
        11,
    );
    daemon
        .registry()
        .save(&record)
        .expect("adopt failure fixture should save");

    let adopt = Command::new(binary)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("adopt")
        .output()
        .expect("daemon CLI adopt should run");
    assert!(
        !adopt.status.success(),
        "adopt command should fail for an unreachable worker socket"
    );
    let stderr = String::from_utf8_lossy(&adopt.stderr);
    assert!(
        stderr.contains("failed to adopt session"),
        "stderr should name adoption failure: {stderr}"
    );
    assert!(
        stderr.contains("daemon-cli-adopt-failure-session"),
        "stderr should name the failed session: {stderr}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

fn build_worker_binary() {
    static BUILD_WORKER: Once = Once::new();
    BUILD_WORKER.call_once(|| {
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "botster-core",
                "--bin",
                "botster-session-worker",
            ])
            .status()
            .expect("worker binary build command should run");
        assert!(
            status.success(),
            "worker binary should build for daemon CLI smoke"
        );
    });
}

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-daemon-{label}-{nanos}"))
}
