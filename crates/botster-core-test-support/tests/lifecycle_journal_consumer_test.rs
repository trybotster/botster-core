//! Isolated Hub-shaped consumer of the control-plane lifecycle journal.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn isolated_hub_shaped_lifecycle_consumer_uses_observe_wake_and_page() {
    let consumer =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/consumers/hub-lifecycle-shaped");
    let source = fs::read_to_string(consumer.join("src/lib.rs")).expect("read consumer source");
    assert!(
        source.contains("observe_lifecycle_slice"),
        "consumer must compile against observe_lifecycle_slice"
    );
    assert!(
        source.contains("lifecycle_baseline_page"),
        "consumer must page the frozen baseline"
    );
    assert!(
        !source.contains(".observe_lifecycle("),
        "Stage A must not call the unbounded observe wrapper"
    );
    assert!(
        source.contains("fn install_baseline")
            && source.contains("lifecycle_baseline_page")
            && source.contains("LifecycleBaselineBudget"),
        "Stage A baseline install must use the paged freeze budget"
    );
    assert!(
        source.contains("index-in-progress") && source.contains("next = None"),
        "Stage A baseline install must retry setup-only and index-in-progress yields"
    );
    assert!(
        source.contains("fn observe_lifecycle_stage_a")
            && source.contains("observe_lifecycle_resume_cursor")
            && !source.contains("let mut resume = None;"),
        "Stage A observe must be one slice per owner turn"
    );
    assert!(
        source.contains("lifecycle_changes_page"),
        "consumer must page through the bounded API"
    );
    assert!(
        source.contains("take_journal_advanced_wake"),
        "consumer must own the coalesced wake take"
    );
    assert!(
        source.contains("BudgetTooSmall"),
        "consumer must match BudgetTooSmall"
    );
    assert!(
        source.contains("_ =>"),
        "consumer must wildcard unknown SessionLifecyclePageError variants"
    );
    assert!(
        source.contains("install_baseline")
            && source.contains("page.changes.is_empty()"),
        "consumer must recover via baseline when an empty successful page is still behind the watermark"
    );
    for forbidden in [".drain(", "drain_runtime_once", "drain_runtime_all_once"] {
        assert!(
            !source.contains(forbidden),
            "consumer must not call terminal Drain ({forbidden})"
        );
    }

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let output = Command::new(env!("CARGO"))
        .args(["test", "--quiet", "--offline"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", workspace_target)
        .output()
        .expect("cargo test hub-lifecycle-shaped consumer");
    assert!(
        output.status.success(),
        "hub-lifecycle-shaped consumer failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
