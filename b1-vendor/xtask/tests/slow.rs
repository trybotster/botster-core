//! Slow tier: the one test that runs the xtask binary (BUILD.md Testing rule 7, the exception: argv, exit status and the stderr format
//! are the binary's own job; `tests/commands.rs` proves every command in-process).
#![cfg(feature = "slow")]

use botster_test_support::tempdir::TempRoot;
use std::process::Command;

#[test]
fn the_binary_reads_argv_prints_problems_and_the_error_line_and_exits_1() {
    let tmp = TempRoot::new().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("contracts.toml"), "dir = \"frozen/final6\"\n").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .env("BOTSTER_XTASK_ROOT", root)
            .output()
            .unwrap()
    };

    let help = run(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).starts_with("usage: cargo xtask <command>"));

    // `manifest` with no MANIFEST.md fails with its message on stderr as `error: ...`.
    let out = run(&["manifest"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("error: "), "{err}");

    let unknown = run(&["nope"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr)
        .starts_with("error: unknown command 'nope'\nusage:"));

    // `ci` runs the commands itself, so it prints their problem lines too, then `error: <summary>`, then the table.
    let tmp = TempRoot::new().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("contracts.toml"),
        "dir = \"frozen/final6\"\n[[contract]]\nprefix = \"hc\"\nfiles = [\"hc.md\"]\n[superseded]\nfiles = []\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("frozen/final6")).unwrap();
    let sha = "0".repeat(64);
    std::fs::write(
        root.join("frozen/final6/MANIFEST.md"),
        format!("| X | `gone.md` | `{sha}` |\n"),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["ci", "--job", "manifest"])
        .env("BOTSTER_XTASK_ROOT", root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    let line = "gone.md: listed in the manifest, but missing from frozen/final6\nerror: 1 problem(s)\nerror: ci failed\n";
    assert_eq!(err, line);
    let table = String::from_utf8_lossy(&out.stdout);
    assert!(table.contains("manifest   FAIL"), "{table}");
}
