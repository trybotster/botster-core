//! Cache-path contract tests for the Ghostty build script.

#[allow(dead_code)]
#[path = "../build_support.rs"]
mod build_support;

use std::path::Path;

use build_support::{zig_global_cache_dir, zig_local_cache_dir};

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
