//! `cargo xtask signals`: the pattern rule over the source. A raw signal call outside `botster_core_sys::signal`, or a
//! `Command::new` of a kill program, is a violation.
//!
//! The check reads the tokens of each file (`proc-macro2`), not lints: a `#[allow]`, a feature that the clippy run does
//! not enable, or a `cfg` cannot hide a call from it. A comment is no token, and a string literal is no identifier, so a
//! mention in either is no call.

use crate::fsutil::tracked_files;
use anyhow::{bail, Result};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::path::Path;

/// The one file that may make the raw calls: it refuses a target of 0, 1 and our own first.
const GUARD: &str = "crates/botster-core-sys/src/signal.rs";

/// The rustix calls that only [`GUARD`] may make.
const RAW_CALLS: [&str; 3] = [
    "kill_process_group",
    "kill_process",
    "kill_current_process_group",
];

/// The programs that signal a pid that the caller passes as text: a pid from `ps` or a file could be 1.
const KILL_PROGRAMS: [&str; 3] = ["kill", "pkill", "killall"];

/// The text of a string literal token (`"…"`, `r"…"`, `r#"…"#`), or `None` for another literal. Escapes are kept as
/// written: no kill program name has one.
fn string_text(literal: &str) -> Option<&str> {
    let raw = literal
        .strip_prefix('r')
        .map_or(literal, |rest| rest.trim_matches('#'));
    raw.strip_prefix('"')?.strip_suffix('"')
}

/// Whether `tokens` start with `Command :: new` and a parenthesized group whose first token is a kill program literal.
fn kill_program(tokens: &[TokenTree]) -> bool {
    let [TokenTree::Ident(command), TokenTree::Punct(first), TokenTree::Punct(second), TokenTree::Ident(new), TokenTree::Group(args), ..] =
        tokens
    else {
        return false;
    };
    if command != "Command" || new != "new" || args.delimiter() != Delimiter::Parenthesis {
        return false;
    }
    if (first.as_char(), second.as_char()) != (':', ':') {
        return false;
    }
    let Some(TokenTree::Literal(program)) = args.stream().into_iter().next() else {
        return false;
    };
    string_text(&program.to_string())
        .map(|text| text.rsplit('/').next().unwrap_or(text))
        .is_some_and(|name| KILL_PROGRAMS.contains(&name))
}

/// Every violation in `tokens`, groups included: `(line, message)`. A raw call counts only outside the guard.
fn violations(tokens: TokenStream, guard: bool, found: &mut Vec<(usize, String)>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, tree) in tokens.iter().enumerate() {
        if kill_program(&tokens[index..]) {
            found.push((
                tree.span().start().line,
                "a kill program: signal through botster_core_sys::signal (the pattern rule)"
                    .to_string(),
            ));
        }
        match tree {
            TokenTree::Group(group) => violations(group.stream(), guard, found),
            TokenTree::Ident(ident) if !guard => {
                let name = ident.to_string();
                let name = name.trim_start_matches("r#");
                if RAW_CALLS.contains(&name) {
                    found.push((
                        ident.span().start().line,
                        format!("raw `{name}`: signal through botster_core_sys::signal (the pattern rule)"),
                    ));
                }
            }
            _ => {}
        }
    }
}

/// The violations of one file: `(line, message)`. A file that does not lex is one violation on line 1.
pub fn scan(file: &str, text: &str) -> Vec<(usize, String)> {
    let tokens: TokenStream = match text.parse() {
        Ok(tokens) => tokens,
        Err(error) => return vec![(1, format!("does not lex: {error}"))],
    };
    let mut found = Vec::new();
    violations(tokens, file == GUARD, &mut found);
    found
}

/// A violation in a file: `(file, 1-based line, message)`.
pub type Violation = (String, usize, String);

/// The violations over the Rust files of `files`, given as `(path, text)`, and the count of Rust files scanned.
pub fn scan_files(files: &[(String, String)]) -> (usize, Vec<Violation>) {
    let mut scanned = 0;
    let mut violations = Vec::new();
    for (file, text) in files.iter().filter(|(f, _)| f.ends_with(".rs")) {
        scanned += 1;
        for (line, message) in scan(file, text) {
            violations.push((file.clone(), line, message));
        }
    }
    (scanned, violations)
}

/// # Errors
/// An argument, a failure to list the files, or a violation.
pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let mut files = Vec::new();
    for file in tracked_files(root)? {
        if let Ok(text) = std::fs::read_to_string(root.join(&file)) {
            files.push((file, text));
        }
    }
    verdict(&files)
}

/// The step's result over `files`: each violation is printed, and any violation fails the step.
///
/// # Errors
/// A violation.
pub fn verdict(files: &[(String, String)]) -> Result<()> {
    let (scanned, violations) = scan_files(files);
    for (file, line, message) in &violations {
        eprintln!("{file}:{line}: {message}");
    }
    println!("signals: {scanned} Rust files scanned");
    if !violations.is_empty() {
        bail!("{} violation(s)", violations.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(file: &str, text: &str) -> Vec<usize> {
        scan(file, text).into_iter().map(|(l, _)| l).collect()
    }

    /// Red on revert: each raw call, as a path, an import, a renamed import or a glob-imported name, is a violation in every
    /// file but the guard. An `#[allow]`, a `cfg(feature = "slow")` and a test module hide nothing. The fixture is text:
    /// no call runs.
    #[test]
    fn a_raw_signal_call_outside_the_guard_is_a_violation_under_any_allow_or_cfg() {
        let reverted = "#![allow(clippy::disallowed_methods)]\n\
            #[cfg(feature = \"slow\")]\n\
            fn end(pid: Pid) {\n\
            \x20   rustix::process::kill_process(pid, Signal::TERM).unwrap();\n\
            }\n\
            #[cfg(test)]\n\
            mod tests {\n\
            \x20   use rustix::process::{kill_process_group as group, *};\n\
            \x20   fn f() { let _ = r#kill_current_process_group(Signal::KILL); }\n\
            }\n";
        for file in [
            "crates/botster-core/tests/slow_real_core.rs",
            "crates/botster-core-sys/src/process.rs",
            "xtask/src/test_budget.rs",
        ] {
            assert_eq!(lines(file, reverted), [4, 8, 9], "{file}");
        }
        assert!(lines(GUARD, reverted).is_empty());
    }

    /// Red on revert of S1: a kill program as the program of a `Command::new`, by name, by path or as a raw string, is a
    /// violation, in the guard too. The same text elsewhere (a message name, an argument, a longer name) is not a program.
    #[test]
    fn a_kill_program_command_is_a_violation() {
        let text = "let a = Command::new(\"kill\");\nlet b = std::process::Command::new(\"/usr/bin/pkill\");\n\
            let c = Command::new(\n  r#\"killall\"#);\nlet d = { Command::new(r\"kill\") };\n\
            let ok = (\"kill\", Command::new(\"sh\").arg(\"kill\"), Command::new(\"skill\"), Command::new(\"killer\"));\n\
            let ok = (Command::new(KILL), Command::old(\"kill\"), Builder::new(\"kill\"), Command::new[\"kill\"]);\n\
            let ok = (Command:new(\"kill\"), Command;:new(\"kill\"), Command::new(b'k'), Command::new());\n";
        assert_eq!(lines("xtask/src/test_budget.rs", text), [1, 2, 3, 5]);
        assert_eq!(lines(GUARD, text), [1, 2, 3, 5]);
    }

    /// A mention in a comment or in a string is no call; a longer name that contains a raw call is not that call.
    #[test]
    fn comments_strings_and_longer_names_are_not_calls() {
        let text = "// kill_process(1)\n/* kill_process_group /* nested */ kill_process */\n\
            let s = \"kill_process_group\";\nlet r = r##\"kill_process \"# still\"##;\n\
            let b = b\"kill_process\";\nlet _ = test_kill_process(pid);\nlet _ = test_kill_process_group(group);\n\
            fn f<'a>(x: &'a str) -> char { 'k' }\nlet _ = signal_process(pid, Signal::KILL);\n";
        assert!(lines("crates/botster-core/src/real.rs", text).is_empty());
    }

    /// A literal is a string only with its quotes; its text is between them.
    #[test]
    fn a_string_literal_gives_its_text() {
        assert_eq!(string_text("\"kill\""), Some("kill"));
        assert_eq!(string_text("r\"kill\""), Some("kill"));
        assert_eq!(string_text("r##\"kill\"##"), Some("kill"));
        for other in ["'k'", "b'k'", "7", "\"kill", "kill\""] {
            assert_eq!(string_text(other), None, "{other}");
        }
    }

    /// A file that does not lex cannot be checked, so it is a violation.
    #[test]
    fn a_file_that_does_not_lex_is_a_violation() {
        let found = scan("a.rs", "fn f() { \"open");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 1);
        assert!(found[0].1.starts_with("does not lex"), "{found:?}");
    }

    /// Only Rust files are scanned and counted.
    #[test]
    fn only_rust_files_are_scanned() {
        let files = [
            ("a.rs".to_string(), "kill_process(p);\n".to_string()),
            ("b.toml".to_string(), "kill_process\n".to_string()),
        ];
        let (scanned, violations) = scan_files(&files);
        assert_eq!(scanned, 1);
        assert_eq!(
            violations,
            [("a.rs".to_string(), 1, violations[0].2.clone())]
        );
    }

    /// Red on revert at the step: one raw call in one file fails the step.
    #[test]
    fn one_violation_fails_the_step() {
        let mut files = vec![("a.rs".to_string(), "signal_process(p, s);\n".to_string())];
        verdict(&files).unwrap();
        files.push((
            "b.rs".to_string(),
            "rustix::process::kill_process(p, s);\n".to_string(),
        ));
        assert!(verdict(&files).is_err());
    }

    /// The repo has no violation: the scan that the gate runs passes on this tree. The walk needs no git, so the test
    /// also runs in the cargo-mutants copy.
    #[test]
    fn the_repo_has_no_raw_signal_call() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let files: Vec<(String, String)> = crate::fsutil::walk_files(&root)
            .into_iter()
            .filter_map(|f| Some((f.clone(), std::fs::read_to_string(root.join(&f)).ok()?)))
            .collect();
        let (scanned, violations) = scan_files(&files);
        assert!(scanned > 100, "{scanned}");
        assert!(violations.is_empty(), "{violations:?}");
        assert!(files.iter().any(|(f, _)| f == GUARD));
        verdict(&files).unwrap();
        assert!(command(&root, &["x".to_string()]).is_err());
    }
}
