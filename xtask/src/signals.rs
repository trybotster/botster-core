//! `cargo xtask signals`: the pattern rule over the source. A raw signal call outside `botster_core_sys::signal`, or a
//! program literal that names a kill command, is a violation.
//!
//! The check reads tokens, not lints: a `#[allow]`, a feature that the clippy run does not enable, or a `cfg` cannot hide a
//! call from it. Comments and the text of string literals are not code, so a mention in either is no call.

use crate::fsutil::tracked_files;
use anyhow::{bail, Result};
use std::path::Path;

/// The one file that may make the raw calls: it refuses a target of 0, 1 and our own first.
const GUARD: &str = "crates/botster-core-sys/src/signal.rs";

/// This file names the kill programs as its own banned literals.
const SCANNER: &str = "xtask/src/signals.rs";

/// The rustix calls that only [`GUARD`] may make.
const RAW_CALLS: [&str; 3] = [
    "kill_process_group",
    "kill_process",
    "kill_current_process_group",
];

/// The programs that signal a pid that the caller passes as text: a pid from `ps` or a file could be 1.
const KILL_PROGRAMS: [&str; 3] = ["kill", "pkill", "killall"];

/// What the scan finds in code: identifiers, the text of string literals, and the literals that name the program of a
/// `Command::new(…)`, each with its 1-based line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub identifiers: Vec<(usize, String)>,
    pub strings: Vec<(usize, String)>,
    pub programs: Vec<(usize, String)>,
}

/// The code before a literal that makes it the program of a command.
const PROGRAM_CALL: &str = "Command::new(";

/// The identifiers and string literals of Rust source. Line and block comments (nested) are skipped. A char literal is
/// skipped; a lifetime is not a literal. Raw and byte strings are strings.
pub fn tokens(text: &str) -> Tokens {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Tokens::default();
    let (mut i, mut line) = (0, 1);
    let at = |i: usize| chars.get(i).copied();
    // The code so far without whitespace, comments or literal text: what precedes the next token.
    let mut code = String::new();
    let literal = |found: &mut Tokens, code: &mut String, line: usize, text: String| {
        if code.ends_with(PROGRAM_CALL) {
            found.programs.push((line, text.clone()));
        }
        found.strings.push((line, text));
        code.push('"');
    };
    while let Some(c) = at(i) {
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            '/' if at(i + 1) == Some('/') => {
                while at(i).is_some_and(|c| c != '\n') {
                    i += 1;
                }
            }
            '/' if at(i + 1) == Some('*') => {
                let mut depth = 0;
                while let Some(c) = at(i) {
                    if c == '/' && at(i + 1) == Some('*') {
                        depth += 1;
                        i += 2;
                    } else if c == '*' && at(i + 1) == Some('/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        line += usize::from(c == '\n');
                        i += 1;
                    }
                }
            }
            '"' => {
                let (text, next, lines) = quoted(&chars, i + 1);
                literal(&mut found, &mut code, line, text);
                (i, line) = (next, line + lines);
            }
            '\'' => {
                // A char literal: `'x'` or `'\…'`. Otherwise a lifetime or a label: skip the quote.
                if at(i + 1) == Some('\\') {
                    i += 2;
                    while at(i).is_some_and(|c| c != '\'') {
                        i += 1;
                    }
                    i += 1;
                } else if at(i + 2) == Some('\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while at(i).is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                // `r"…"`, `r#"…"#`, `br"…"`: a raw string. `b"…"`: a byte string. `r#name`: a raw identifier.
                if matches!(word.as_str(), "r" | "br") && matches!(at(i), Some('"' | '#')) {
                    let hashes = chars[i..].iter().take_while(|&&c| c == '#').count();
                    if at(i + hashes) == Some('"') {
                        let (text, next, lines) = raw(&chars, i + hashes + 1, hashes);
                        literal(&mut found, &mut code, line, text);
                        (i, line) = (next, line + lines);
                        continue;
                    }
                    i += hashes;
                    continue;
                }
                if word == "b" && at(i) == Some('"') {
                    continue;
                }
                code.push_str(&word);
                found.identifiers.push((line, word));
            }
            c => {
                if !c.is_whitespace() {
                    code.push(c);
                }
                i += 1;
            }
        }
    }
    found
}

/// The text of a string whose body starts at `from`, the index after its closing quote, and the newlines it spans.
fn quoted(chars: &[char], from: usize) -> (String, usize, usize) {
    let (mut text, mut i, mut lines) = (String::new(), from, 0);
    while let Some(&c) = chars.get(i) {
        match c {
            '"' => return (text, i + 1, lines),
            '\\' => {
                if let Some(&next) = chars.get(i + 1) {
                    lines += usize::from(next == '\n');
                    text.push(next);
                }
                i += 2;
            }
            c => {
                lines += usize::from(c == '\n');
                text.push(c);
                i += 1;
            }
        }
    }
    (text, i, lines)
}

/// The text of a raw string with `hashes` hashes whose body starts at `from`.
fn raw(chars: &[char], from: usize, hashes: usize) -> (String, usize, usize) {
    let (mut text, mut i, mut lines) = (String::new(), from, 0);
    while let Some(&c) = chars.get(i) {
        if c == '"'
            && chars[i + 1..]
                .iter()
                .take(hashes)
                .filter(|&&h| h == '#')
                .count()
                == hashes
        {
            return (text, i + 1 + hashes, lines);
        }
        lines += usize::from(c == '\n');
        text.push(c);
        i += 1;
    }
    (text, i, lines)
}

/// The violations of one file: `(line, message)`.
pub fn scan(file: &str, text: &str) -> Vec<(usize, String)> {
    let found = tokens(text);
    let mut violations = Vec::new();
    if file != GUARD {
        for (line, word) in &found.identifiers {
            if RAW_CALLS.contains(&word.as_str()) {
                violations.push((
                    *line,
                    format!(
                        "raw `{word}`: signal through botster_core_sys::signal (the pattern rule)"
                    ),
                ));
            }
        }
    }
    if file != SCANNER {
        for (line, text) in &found.programs {
            let program = text.rsplit('/').next().unwrap_or(text);
            if KILL_PROGRAMS.contains(&program) {
                violations.push((
                    *line,
                    format!("the program `{text}`: signal through botster_core_sys::signal (the pattern rule)"),
                ));
            }
        }
    }
    violations
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

    /// Red on revert: each raw call, as a path, an import or a glob-imported name, is a violation in every file but the
    /// guard. An `#[allow]`, a `cfg(feature = "slow")` and a test module hide nothing. The fixture is text: no call runs.
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
            \x20   fn f() { let _ = kill_current_process_group(Signal::KILL); }\n\
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

    /// Red on revert of S1: a kill program as the program of a `Command::new`, by name or by path, is a violation. The
    /// same text elsewhere (a message name, an argument) is not a program.
    #[test]
    fn a_kill_program_literal_is_a_violation() {
        let text = "let a = Command::new(\"kill\");\nlet b = Command::new(\"/usr/bin/pkill\");\n\
            let c = std::process::Command::new(\n  r#\"killall\"#);\nlet ok = (\"skill\", \"kill -s\", \"killer\");\n\
            let name = \"kill\"; let other = Command::new(\"sh\").arg(\"kill\");\n";
        assert_eq!(lines("xtask/src/test_budget.rs", text), [1, 2, 4]);
        assert_eq!(lines(GUARD, text), [1, 2, 4]);
        assert!(lines(SCANNER, text).is_empty());
    }

    /// A mention in a comment or in a string is no call; a longer name that contains a raw call is not that call.
    #[test]
    fn comments_strings_and_longer_names_are_not_calls() {
        let text = "// kill_process(1)\n/* kill_process_group /* nested */ kill_process */\n\
            let s = \"kill_process_group\";\nlet r = r##\"kill_process \"# still\"##;\n\
            let b = b\"kill_process\";\nlet _ = test_kill_process(pid);\nlet _ = test_kill_process_group(group);\n\
            fn f<'a>(x: &'a str) -> char { let _ = '\\''; 'k' }\nlet _ = signal_process(pid, Signal::KILL);\n";
        assert!(lines("crates/botster-core/src/real.rs", text).is_empty());
    }

    /// Tokens keep their lines through every comment and literal that spans lines.
    #[test]
    fn tokens_keep_their_lines_across_multiline_comments_and_strings() {
        let text = "/* a\nb */ one\n\"x\ny\" two\nr#\"p\nq\"# three\n'\\n' four\n";
        let found = tokens(text);
        assert_eq!(
            found.identifiers,
            [
                (2, "one".to_string()),
                (4, "two".to_string()),
                (6, "three".to_string()),
                (7, "four".to_string())
            ]
        );
        assert_eq!(
            found.strings,
            [(3, "x\ny".to_string()), (5, "p\nq".to_string())]
        );
        assert_eq!(tokens("\"a\\\"b\"").strings, [(1, "a\"b".to_string())]);
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
}
