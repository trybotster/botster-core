//! Published Core drivers plus isolated Hub-shaped consumer proof.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use botster_core_test_support::terminal_adapter::{
    assert_terminal_adapter_conformance, assert_waking_terminal_adapter_conformance,
    FakeTerminalAdapter, UnixShapedTerminalAdapter, WebRtcShapedTerminalAdapter,
};

#[test]
fn fake_terminal_adapter_passes_published_harness() {
    let mut driver = FakeTerminalAdapter::default();
    assert_terminal_adapter_conformance(&mut driver);
}

#[test]
fn unix_shaped_terminal_adapter_passes_published_harness() {
    let mut driver = UnixShapedTerminalAdapter::default();
    assert_terminal_adapter_conformance(&mut driver);
}

#[test]
fn webrtc_shaped_terminal_adapter_passes_published_harness() {
    let mut driver = WebRtcShapedTerminalAdapter::default();
    assert_terminal_adapter_conformance(&mut driver);
}

#[test]
fn fake_waking_terminal_adapter_passes_published_harness() {
    let mut driver = FakeTerminalAdapter::default();
    assert_waking_terminal_adapter_conformance(&mut driver);
}

#[test]
fn unix_shaped_waking_terminal_adapter_passes_published_harness() {
    let mut driver = UnixShapedTerminalAdapter::default();
    assert_waking_terminal_adapter_conformance(&mut driver);
}

#[test]
fn webrtc_shaped_waking_terminal_adapter_passes_published_harness() {
    let mut driver = WebRtcShapedTerminalAdapter::default();
    assert_waking_terminal_adapter_conformance(&mut driver);
}

#[test]
fn isolated_hub_shaped_consumer_runs_harness_against_its_own_adapter() {
    let consumer =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/consumers/hub-adapter-shaped");
    let source = fs::read_to_string(consumer.join("src/lib.rs")).expect("read consumer source");
    assert!(
        source.contains("impl TerminalAdapter for"),
        "consumer must implement TerminalAdapter"
    );
    assert!(
        source.contains("impl TerminalAdapterHarnessDriver for"),
        "consumer must implement TerminalAdapterHarnessDriver"
    );
    assert!(
        source.contains("assert_terminal_adapter_conformance"),
        "consumer must run the published harness"
    );
    assert!(
        source.contains("impl WakingTerminalAdapter for"),
        "consumer must implement WakingTerminalAdapter"
    );
    assert!(
        source.contains("bind_waking_terminal_adapter"),
        "consumer must use the waking Core bind"
    );
    assert!(
        source.contains("wait_wakes") && source.contains("pump_woken"),
        "consumer must drive Core through targeted wakes"
    );
    for forbidden in [
        "FakeTerminalAdapter",
        "UnixShapedTerminalAdapter",
        "WebRtcShapedTerminalAdapter",
    ] {
        assert!(
            !source.contains(forbidden),
            "consumer must not construct published Core driver {forbidden}"
        );
    }

    let output = Command::new(env!("CARGO"))
        .args(["test", "--quiet", "--offline"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", consumer.join("target"))
        .output()
        .expect("cargo test hub-adapter-shaped consumer");
    assert!(
        output.status.success(),
        "hub-adapter-shaped consumer failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn polling_adapter_path_cannot_return_to_core_source() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("test-support crate must be under the workspace root")
        .to_path_buf();
    let source_roots = [
        workspace.join("crates/botster-core/src"),
        workspace.join("crates/botster-core-daemon/src"),
    ];
    let forbidden = [
        "fn bind_terminal_adapter",
        "pub fn pump(&mut self)",
        "ClientWorker::pump(",
        "intake_terminal_input(",
        "pump_bound_adapters(",
        "drain_runtime_once_without_pump(",
    ];

    for root in source_roots {
        for path in rust_sources(&root) {
            let source = fs::read_to_string(&path).expect("read Core source");
            for pattern in forbidden {
                assert!(
                    !source.contains(pattern),
                    "deleted polling adapter path `{pattern}` returned in {}",
                    path.display()
                );
            }
        }
    }
}

fn rust_sources(root: &std::path::Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("read Core source directory") {
            let path = entry.expect("read Core source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources
}
