//! The one exception to the workspace's `unsafe_code = "forbid"` (lead ruling 2026-10-09 on #171 TP3, (A)).
//!
//! botster-test-process copies the workspace's lint table with exactly one difference, `unsafe_code = "deny"`, so that one
//! function of its anchor binary, `close_inherited`, can allow the lint. The check fails:
//! - on any other difference between that crate's lint table and the workspace's;
//! - on an `unsafe_code` entry in any other manifest, but the libghostty-vt binding's (the earlier exception: that crate
//!   calls a C ABI, and its manifest allows `unsafe_code` for the whole crate);
//! - on any attribute that names `unsafe_code`, other than the one `#[allow(unsafe_code)]` of `close_inherited`.

use crate::fsutil::tracked_files;
use anyhow::{bail, Context, Result};
use std::path::Path;

/// The crate with the exception.
const CRATE: &str = "crates/botster-test-process/Cargo.toml";
/// The C ABI binding of libghostty-vt, which allows `unsafe_code` in its own lint table (eddd5e75; not this ruling).
const FFI_CRATE: &str = "crates/botster-terminal-ghostty/Cargo.toml";
/// The file and the function that may allow `unsafe_code`.
const ALLOWED_FILE: &str = "crates/botster-test-process/src/bin/botster-test-anchor.rs";
const ALLOWED_FN: &str = "fn close_inherited(";

/// Whether the crate's lint table is the workspace's with `unsafe_code = "deny"` in place of `"forbid"`, and nothing else.
///
/// # Errors
/// A manifest does not parse, the workspace does not forbid `unsafe_code`, or the tables differ otherwise.
pub fn lint_drift(workspace: &str, krate: &str) -> Result<(), String> {
    let workspace: toml::Table = workspace
        .parse()
        .map_err(|e| format!("the workspace manifest: {e}"))?;
    let krate: toml::Table = krate.parse().map_err(|e| format!("{CRATE}: {e}"))?;
    let mut expected = workspace
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .cloned()
        .ok_or("the workspace has no lint table")?;
    let rust = expected
        .get_mut("rust")
        .and_then(toml::Value::as_table_mut)
        .ok_or("the workspace has no rust lint table")?;
    if rust.get("unsafe_code").and_then(toml::Value::as_str) != Some("forbid") {
        return Err("the workspace does not forbid unsafe_code".into());
    }
    rust.insert("unsafe_code".into(), "deny".into());
    let actual = krate
        .get("lints")
        .ok_or(format!("{CRATE} has no lint table"))?;
    if actual != &expected {
        return Err(format!(
            "the lint table of {CRATE} must be the workspace's with only unsafe_code = \"deny\": it is {actual}, expected {expected}"
        ));
    }
    Ok(())
}

/// The manifests that name `unsafe_code`, other than the workspace's, the crate's and the C ABI binding's.
pub fn other_manifests(manifests: &[(String, String)]) -> Vec<String> {
    manifests
        .iter()
        .filter(|(file, text)| {
            !["Cargo.toml", CRATE, FFI_CRATE].contains(&file.as_str())
                && text.contains("unsafe_code")
        })
        .map(|(file, _)| format!("{file}: an unsafe_code lint"))
        .collect()
}

/// The attributes that name `unsafe_code`, other than the one `#[allow(unsafe_code)]` directly above `close_inherited` (doc
/// comments and other attributes may come between them). A second allowed attribute is reported too.
pub fn other_attributes(files: &[(String, String)]) -> Vec<String> {
    let mut found = Vec::new();
    let mut allowed = false;
    for (file, text) in files {
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (index, line) in lines.iter().enumerate() {
            if !(line.starts_with("#[") || line.starts_with("#![")) || !line.contains("unsafe_code")
            {
                continue;
            }
            // The item that the attribute is on: the first line from here that is neither an attribute nor a comment.
            let item = lines[index..]
                .iter()
                .find(|l| !(l.starts_with("#[") || l.starts_with("//")));
            if file == ALLOWED_FILE
                && *line == "#[allow(unsafe_code)]"
                && item.is_some_and(|l| l.starts_with(ALLOWED_FN))
                && !allowed
            {
                allowed = true;
                continue;
            }
            found.push(format!("{file}:{}: {line}", index + 1));
        }
    }
    found
}

/// Whether the check reads `file`: a manifest or a Rust source.
pub fn checked(file: &str) -> bool {
    is_manifest(file) || file.ends_with(".rs")
}

fn is_manifest(file: &str) -> bool {
    file == "Cargo.toml" || file.ends_with("/Cargo.toml")
}

/// The verdict of the check over the checked `files` (path and text): a summary line, or every problem with their count.
///
/// # Errors
/// The report of the problems that the check found.
pub fn verdict(workspace: &str, krate: &str, files: &[(String, String)]) -> Result<String, String> {
    let (manifests, sources): (Vec<_>, Vec<_>) = files
        .iter()
        .cloned()
        .partition(|(file, _)| is_manifest(file));
    let mut problems: Vec<String> = lint_drift(workspace, krate).err().into_iter().collect();
    problems.extend(other_manifests(&manifests));
    problems.extend(other_attributes(&sources));
    if problems.is_empty() {
        Ok(format!(
            "unsafe-code: {} manifests and {} sources checked",
            manifests.len(),
            sources.len()
        ))
    } else {
        Err(format!(
            "{}\n{} problem(s) with the unsafe_code exception",
            problems.join("\n"),
            problems.len()
        ))
    }
}

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
    let summary =
        verdict(&read("Cargo.toml")?, &read(CRATE)?, &files).map_err(anyhow::Error::msg)?;
    println!("{summary}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: &str = include_str!("../../Cargo.toml");
    const KRATE: &str = include_str!("../../crates/botster-test-process/Cargo.toml");
    const ANCHOR: &str =
        include_str!("../../crates/botster-test-process/src/bin/botster-test-anchor.rs");

    #[test]
    fn the_crate_differs_from_the_workspace_only_by_deny() {
        assert_eq!(lint_drift(WORKSPACE, KRATE), Ok(()));
    }

    /// Red on revert: each other difference of the crate's table fails the check, and so does a workspace without `forbid`.
    #[test]
    fn any_other_lint_difference_fails() {
        for changed in [
            KRATE.replace("unsafe_code = \"deny\"", "unsafe_code = \"allow\""),
            KRATE.replace("unsafe_code = \"deny\"", "unsafe_code = \"forbid\""),
            KRATE.replace(
                "unsafe_code = \"deny\"",
                "unsafe_code = \"deny\"\nunused = \"allow\"",
            ),
            KRATE.replace(
                "[lints.rust]\nunsafe_code = \"deny\"",
                "[lints]\nworkspace = true",
            ),
            KRATE.replace("[lints.rust]\nunsafe_code = \"deny\"\n", ""),
        ] {
            assert_ne!(changed, KRATE, "the replacement applies");
            assert!(lint_drift(WORKSPACE, &changed).is_err(), "{changed}");
        }
        let unforbidden = WORKSPACE.replace("unsafe_code = \"forbid\"", "unsafe_code = \"deny\"");
        assert_eq!(
            lint_drift(&unforbidden, KRATE),
            Err("the workspace does not forbid unsafe_code".into())
        );
        assert!(lint_drift("[workspace]\n", KRATE).is_err());
    }

    #[test]
    fn the_verdict_counts_the_files_or_reports_every_problem() {
        let files = vec![
            ("Cargo.toml".to_string(), WORKSPACE.to_string()),
            (CRATE.to_string(), KRATE.to_string()),
            (ALLOWED_FILE.to_string(), ANCHOR.to_string()),
        ];
        assert_eq!(
            verdict(WORKSPACE, KRATE, &files),
            Ok("unsafe-code: 2 manifests and 1 sources checked".into())
        );
        // A source that names the manifest key in an attribute is still a source, and a manifest is never read as a source.
        let bad = vec![
            ("a.rs".to_string(), "#![allow(unsafe_code)]\n".to_string()),
            ("x/Cargo.toml".to_string(), "#[unsafe_code]\n".to_string()),
        ];
        assert_eq!(
            verdict(WORKSPACE, KRATE, &bad),
            Err("x/Cargo.toml: an unsafe_code lint\na.rs:1: #![allow(unsafe_code)]\n2 problem(s) with the unsafe_code exception".into())
        );
        let drift = verdict(WORKSPACE, "[lints]\nworkspace = true\n", &[]).unwrap_err();
        assert!(
            drift.ends_with("\n1 problem(s) with the unsafe_code exception"),
            "{drift}"
        );
    }

    #[test]
    fn the_check_reads_manifests_and_rust_sources_only() {
        assert!(checked("Cargo.toml"));
        assert!(checked("crates/x/Cargo.toml"));
        assert!(checked("xtask/src/main.rs"));
        assert!(!checked("crates/x/Cargo.lock"));
        assert!(!checked("docs/notCargo.toml"));
        assert!(!checked("a.rs.txt"));
    }

    #[test]
    fn only_the_two_manifests_name_unsafe_code() {
        let manifests = |other: &str| {
            vec![
                ("Cargo.toml".to_string(), WORKSPACE.to_string()),
                (CRATE.to_string(), KRATE.to_string()),
                ("crates/x/Cargo.toml".to_string(), other.to_string()),
            ]
        };
        assert!(other_manifests(&manifests("[lints]\nworkspace = true\n")).is_empty());
        let ffi = vec![(
            FFI_CRATE.to_string(),
            "[lints.rust]\nunsafe_code = \"allow\"\n".to_string(),
        )];
        assert!(other_manifests(&ffi).is_empty(), "the earlier exception");
        assert_eq!(
            other_manifests(&manifests("[lints.rust]\nunsafe_code = \"allow\"\n")),
            ["crates/x/Cargo.toml: an unsafe_code lint"]
        );
    }

    /// Red on revert: the anchor binary has exactly the one allowed attribute; an allow elsewhere, on another function, or a
    /// second one fails the check.
    #[test]
    fn only_close_inherited_allows_unsafe_code() {
        let files = |file: &str, text: &str| vec![(file.to_string(), text.to_string())];
        assert!(other_attributes(&files(ALLOWED_FILE, ANCHOR)).is_empty());
        let elsewhere = "#[allow(unsafe_code)]\nfn close_inherited() {}\n";
        assert_eq!(
            other_attributes(&files("crates/x/src/lib.rs", elsewhere)),
            ["crates/x/src/lib.rs:1: #[allow(unsafe_code)]"]
        );
        let other_fn = "/// doc\n#[allow(unsafe_code)]\n#[inline]\nfn other() {}\n";
        assert_eq!(
            other_attributes(&files(ALLOWED_FILE, other_fn)),
            [format!("{ALLOWED_FILE}:2: #[allow(unsafe_code)]")]
        );
        let between = "#[allow(unsafe_code)]\n// note\n#[inline]\nfn close_inherited() {}\n";
        assert!(other_attributes(&files(ALLOWED_FILE, between)).is_empty());
        let twice = format!("{ANCHOR}\n#[allow(unsafe_code)]\nfn close_inherited() {{}}\n");
        assert_eq!(other_attributes(&files(ALLOWED_FILE, &twice)).len(), 1);
        let inner = "#![allow(unsafe_code)]\n";
        assert_eq!(
            other_attributes(&files("crates/x/src/lib.rs", inner)).len(),
            1
        );
        let expect = "#[expect(unsafe_code)]\nfn close_inherited() {}\n";
        assert_eq!(other_attributes(&files(ALLOWED_FILE, expect)).len(), 1);
    }
}
