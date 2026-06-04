#![allow(missing_docs)]

use std::process::Command;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

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
