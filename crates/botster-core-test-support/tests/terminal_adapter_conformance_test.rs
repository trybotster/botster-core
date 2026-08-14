//! Published Core drivers plus isolated Hub-shaped consumer proof.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use botster_core_test_support::terminal_adapter::{
    assert_terminal_adapter_conformance, FakeTerminalAdapter, UnixShapedTerminalAdapter,
    WebRtcShapedTerminalAdapter,
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
