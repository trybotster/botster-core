//! The exceptions to the workspace's `unsafe_code = "forbid"`, each of one item in one file of one crate:
//! - botster-test-process, the function `close_inherited` of its anchor binary (lead ruling 2026-10-09 on #171 TP3, (A));
//! - botster-core-testkit, `unsafe impl GlobalAlloc for PeakAllocator`, the allocator of the `alloc_window` and
//!   `alloc_peak` controls (lead ruling 2026-10-10, (A); steward rulings R-37, R-52). Its methods only call the system
//!   allocator.
//!
//! Each of these crates copies the workspace's lint table with exactly one difference, `unsafe_code = "deny"`, so that its
//! one item can allow the lint. The check fails:
//! - on any other difference between such a crate's lint table and the workspace's;
//! - on any key or string that names the lint in a manifest or a Cargo configuration (`rustflags` included), but the lint
//!   entries of the workspace, of those crates and of the libghostty-vt binding (the earlier exception: that crate calls a C
//!   ABI, and its manifest allows `unsafe_code` for the whole crate). TOML is parsed, so an escaped key is read as Cargo reads
//!   it; `-` is read as `_`, as rustc reads a lint name;
//! - on any `unsafe_code` identifier in a Rust source, other than the one in the `#[allow(unsafe_code)]` of an exception's
//!   top-level item in its own file. Sources are read as Rust tokens: comments and string literals are not identifiers, and
//!   an attribute on several lines, a `cfg_attr`, an `expect` or a macro body is read as a whole (#171 round 2, TP7).
//!
//! The rule lives in `botster_gate_kit::unsafe_exception`; this module holds Core's exceptions, its FFI crate and the I/O.

use crate::fsutil::tracked_files;
use anyhow::{bail, Context, Result};
use botster_gate_kit::unsafe_exception::{checked, verdict, Exception, Policy};
use std::path::Path;

/// The exceptions (see the module's text).
pub const EXCEPTIONS: [Exception; 2] = [
    Exception {
        manifest: "crates/botster-test-process/Cargo.toml",
        file: "crates/botster-test-process/src/bin/botster-test-anchor.rs",
        item: &["fn", "close_inherited"],
    },
    Exception {
        manifest: "crates/botster-core-testkit/Cargo.toml",
        file: "crates/botster-core-testkit/src/alloc_controls.rs",
        item: &["unsafe", "impl", "GlobalAlloc", "for", "PeakAllocator"],
    },
];
/// The C ABI binding of libghostty-vt, which allows `unsafe_code` in its own lint table (eddd5e75; not this ruling).
const FFI_CRATE: &str = "crates/botster-terminal-ghostty/Cargo.toml";

/// Core's policy: its exceptions and its FFI crate.
const POLICY: Policy = Policy {
    exceptions: &EXCEPTIONS,
    ffi_manifests: &[FFI_CRATE],
};

/// The check over the repository's tracked files.
///
/// # Errors
/// A file could not be read, or the check found a problem.
pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let read = |file: &str| {
        std::fs::read_to_string(root.join(file)).with_context(|| format!("read {file}"))
    };
    let mut files = Vec::new();
    for file in tracked_files(root)?
        .into_iter()
        .filter(|file| checked(file))
    {
        let text = read(&file)?;
        files.push((file, text));
    }
    let mut krates = Vec::new();
    for exception in EXCEPTIONS {
        krates.push((exception.manifest, read(exception.manifest)?));
    }
    let summary =
        verdict(&POLICY, &read("Cargo.toml")?, &krates, &files).map_err(anyhow::Error::msg)?;
    println!("{summary}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_gate_kit::unsafe_exception::{lint_drift, other_attributes, other_manifests};

    const WORKSPACE: &str = include_str!("../../Cargo.toml");
    const KRATE: &str = include_str!("../../crates/botster-test-process/Cargo.toml");
    const TESTKIT: &str = include_str!("../../crates/botster-core-testkit/Cargo.toml");
    const ALLOC: &str = include_str!("../../crates/botster-core-testkit/src/alloc_controls.rs");
    const CRATE: &str = EXCEPTIONS[0].manifest;
    const ALLOWED_FILE: &str = EXCEPTIONS[0].file;
    const TESTKIT_CRATE: &str = EXCEPTIONS[1].manifest;
    const ALLOC_FILE: &str = EXCEPTIONS[1].file;
    const FFI: &str = include_str!("../../crates/botster-terminal-ghostty/Cargo.toml");
    const CONFIG: &str = include_str!("../../.cargo/config.toml");
    const ANCHOR: &str =
        include_str!("../../crates/botster-test-process/src/bin/botster-test-anchor.rs");

    #[test]
    fn each_exception_crate_differs_from_the_workspace_only_by_deny() {
        assert_eq!(lint_drift(WORKSPACE, CRATE, KRATE), Ok(()));
        assert_eq!(lint_drift(WORKSPACE, TESTKIT_CRATE, TESTKIT), Ok(()));
        // The testkit back on the workspace's table fails.
        let workspace_only = TESTKIT.replace(
            "[lints.rust]\nunsafe_code = \"deny\"",
            "[lints]\nworkspace = true",
        );
        assert_ne!(workspace_only, TESTKIT, "the replacement applies");
        assert!(lint_drift(WORKSPACE, TESTKIT_CRATE, &workspace_only).is_err());
    }

    /// Core's own files pass the rule: the verdict counts them, only the four lint entries name the lint, and each
    /// exception's file has exactly its one allowed identifier (the rule's own tests are in `botster-gate-kit`).
    #[test]
    fn the_core_files_name_only_their_exceptions() {
        let files = vec![
            ("Cargo.toml".to_string(), WORKSPACE.to_string()),
            (CRATE.to_string(), KRATE.to_string()),
            (".cargo/config.toml".to_string(), CONFIG.to_string()),
            (TESTKIT_CRATE.to_string(), TESTKIT.to_string()),
            (ALLOWED_FILE.to_string(), ANCHOR.to_string()),
            (ALLOC_FILE.to_string(), ALLOC.to_string()),
        ];
        let krates = vec![
            (CRATE, KRATE.to_string()),
            (TESTKIT_CRATE, TESTKIT.to_string()),
        ];
        assert_eq!(
            verdict(&POLICY, WORKSPACE, &krates, &files),
            Ok("unsafe-code: 4 manifests and configurations and 2 sources checked".into())
        );
        for (file, text) in [
            ("Cargo.toml", WORKSPACE),
            (CRATE, KRATE),
            (TESTKIT_CRATE, TESTKIT),
            (FFI_CRATE, FFI),
            (".cargo/config.toml", CONFIG),
        ] {
            let found = other_manifests(&POLICY, &[(file.to_string(), text.to_string())]);
            assert_eq!(found, Vec::<String>::new(), "{file}");
        }
        for (file, text) in [(ALLOWED_FILE, ANCHOR), (ALLOC_FILE, ALLOC)] {
            let found = other_attributes(&POLICY, &[(file.to_string(), text.to_string())]);
            assert_eq!(found, Vec::<String>::new(), "{file}");
        }
    }
}
