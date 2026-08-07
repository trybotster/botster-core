//! Cache-path contract tests for the Ghostty build script.

#[allow(dead_code)]
#[path = "../build_support.rs"]
mod build_support;

use std::path::{Path, PathBuf};

use build_support::{
    direct_zig, resolve_zig_command, zig_candidates, zig_global_cache_dir, zig_local_cache_dir,
    REQUIRED_ZIG_VERSION,
};

#[test]
fn default_zig_caches_share_the_cargo_out_dir() {
    let out_dir = Path::new("target/build/botster-terminal-ghostty/out");

    assert_eq!(
        Path::new(&zig_local_cache_dir(
            out_dir.to_str().expect("test path is UTF-8")
        )),
        out_dir.join("zig-local-cache")
    );
    assert_eq!(
        Path::new(&zig_global_cache_dir(
            out_dir.to_str().expect("test path is UTF-8"),
            None
        )),
        out_dir.join("zig-global-cache")
    );
}

#[test]
fn resolution_requires_the_exact_pinned_zig_version() {
    let candidates = [
        direct_zig("/opt/zig-old/zig", "old"),
        direct_zig("/opt/zig-pinned/zig", "pinned"),
    ];

    // A near-miss version must not satisfy the gate: upstream Ghostty declares
    // `minimum_zig_version`, and a mismatched toolchain fails deep inside the
    // Zig build rather than at this preflight.
    let resolved = resolve_zig_command(&candidates, |candidate| match candidate.label.as_str() {
        "old" => Ok("0.15.2".to_owned()),
        _ => Ok(REQUIRED_ZIG_VERSION.to_owned()),
    })
    .expect("pinned Zig candidate resolves");

    assert_eq!(resolved.label, "pinned");
}

#[test]
fn resolution_fails_when_no_candidate_matches_the_pinned_version() {
    let candidates = [direct_zig("/opt/zig-old/zig", "old")];

    let error = resolve_zig_command(&candidates, |_| Ok("0.15.2".to_owned()))
        .expect_err("mismatched Zig must not resolve");

    assert!(
        error.contains(REQUIRED_ZIG_VERSION),
        "error must name the required version, got: {error}"
    );
    assert!(
        error.contains("0.15.2"),
        "error must name the rejected version, got: {error}"
    );
}

#[test]
fn unavailable_candidates_do_not_stop_resolution() {
    let candidates = [
        direct_zig("/nonexistent/zig", "missing"),
        direct_zig("/opt/zig-pinned/zig", "pinned"),
    ];

    let resolved = resolve_zig_command(&candidates, |candidate| match candidate.label.as_str() {
        "missing" => Err("not available".to_owned()),
        _ => Ok(REQUIRED_ZIG_VERSION.to_owned()),
    })
    .expect("resolution continues past an unavailable candidate");

    assert_eq!(resolved.label, "pinned");
}

#[test]
fn mise_install_candidate_tracks_the_pinned_version() {
    let candidates = zig_candidates(None, None, Some("/home/agent".to_owned()), |path| {
        path == &PathBuf::from(format!(
            "/home/agent/.local/share/mise/installs/zig/{REQUIRED_ZIG_VERSION}/bin/zig"
        ))
    });

    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.program.contains(REQUIRED_ZIG_VERSION)),
        "mise install candidate must follow the pinned version, got: {:?}",
        candidates
            .iter()
            .map(|candidate| candidate.program.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn configured_global_cache_does_not_change_local_isolation() {
    let out_dir = "target/build/botster-terminal-ghostty/out";
    let configured_global = "target/shared-zig-global-cache";

    assert_eq!(
        zig_local_cache_dir(out_dir),
        Path::new(out_dir)
            .join("zig-local-cache")
            .display()
            .to_string()
    );
    assert_eq!(
        zig_global_cache_dir(out_dir, Some(configured_global.to_owned())),
        configured_global
    );
}
