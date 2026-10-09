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

/// The calls that only [`GUARD`] may make: rustix's three, and libc's `killpg`. libc's `kill` is matched only as the path
/// `libc::kill`, because a bare `kill` is also `Child::kill`.
const RAW_CALLS: [&str; 4] = [
    "kill_process_group",
    "kill_process",
    "kill_current_process_group",
    "killpg",
];

/// Whether `tokens` start with the path `libc :: kill`, or with `libc :: { … }` whose list names `kill`.
fn libc_kill(tokens: &[TokenTree]) -> bool {
    let [TokenTree::Ident(krate), TokenTree::Punct(first), TokenTree::Punct(second), next, ..] =
        tokens
    else {
        return false;
    };
    if krate != "libc" || (first.as_char(), second.as_char()) != (':', ':') {
        return false;
    }
    match next {
        TokenTree::Ident(name) => name == "kill",
        TokenTree::Group(list) if list.delimiter() == Delimiter::Brace => list
            .stream()
            .into_iter()
            .any(|tree| matches!(tree, TokenTree::Ident(name) if name == "kill")),
        _ => false,
    }
}

/// The programs that signal a pid that the caller passes as text: a pid from `ps` or a file could be 1.
const KILL_PROGRAMS: [&str; 3] = ["kill", "pkill", "killall"];

/// The value of a string literal token (`"…"`, `r"…"`, `r#"…"#`), every escape decoded, or `None` for another literal.
fn string_value(literal: &str) -> Option<String> {
    syn::parse_str::<syn::LitStr>(literal)
        .ok()
        .map(|lit| lit.value())
}

/// The names under which `tokens` can name `std::process::Command`: `Command`, each `Command as X` (an import rename), and
/// each `type X = …Command…;` (a type alias). Groups included.
fn command_names(tokens: TokenStream, names: &mut Vec<String>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, tree) in tokens.iter().enumerate() {
        match &tokens[index..] {
            [TokenTree::Ident(command), TokenTree::Ident(rename), TokenTree::Ident(alias), ..]
                if command == "Command" && rename == "as" =>
            {
                names.push(alias.to_string());
            }
            [TokenTree::Ident(keyword), TokenTree::Ident(alias), TokenTree::Punct(equals), rest @ ..]
                if keyword == "type" && equals.as_char() == '=' =>
            {
                let names_command = rest
                    .iter()
                    .take_while(
                        |tree| !matches!(tree, TokenTree::Punct(end) if end.as_char() == ';'),
                    )
                    .any(|tree| matches!(tree, TokenTree::Ident(name) if name == "Command"));
                if names_command {
                    names.push(alias.to_string());
                }
            }
            _ => {}
        }
        if let TokenTree::Group(group) = tree {
            command_names(group.stream(), names);
        }
    }
}

/// Whether `tokens` start with `<command> :: new` (`<command>` one of `names`) and a parenthesized group whose first token
/// is a string literal whose value names a kill program, by name or by path.
fn kill_program(tokens: &[TokenTree], names: &[String]) -> bool {
    let [TokenTree::Ident(command), TokenTree::Punct(first), TokenTree::Punct(second), TokenTree::Ident(new), TokenTree::Group(args), ..] =
        tokens
    else {
        return false;
    };
    if !names.iter().any(|name| command == name)
        || new != "new"
        || args.delimiter() != Delimiter::Parenthesis
    {
        return false;
    }
    if (first.as_char(), second.as_char()) != (':', ':') {
        return false;
    }
    let Some(TokenTree::Literal(program)) = args.stream().into_iter().next() else {
        return false;
    };
    string_value(&program.to_string()).is_some_and(|value| {
        let name = value.rsplit('/').next().unwrap_or(&value);
        KILL_PROGRAMS.contains(&name)
    })
}

/// Every violation in `tokens`, groups included: `(line, message)`. A raw call counts only outside the guard.
fn violations(
    tokens: TokenStream,
    guard: bool,
    names: &[String],
    found: &mut Vec<(usize, String)>,
) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, tree) in tokens.iter().enumerate() {
        if kill_program(&tokens[index..], names) {
            found.push((
                tree.span().start().line,
                "a kill program: signal through botster_core_sys::signal (the pattern rule)"
                    .to_string(),
            ));
        }
        if !guard && libc_kill(&tokens[index..]) {
            found.push((
                tree.span().start().line,
                "raw `libc::kill`: signal through botster_core_sys::signal (the pattern rule)"
                    .to_string(),
            ));
        }
        match tree {
            TokenTree::Group(group) => violations(group.stream(), guard, names, found),
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
    let mut names = vec!["Command".to_string()];
    command_names(tokens.clone(), &mut names);
    let mut found = Vec::new();
    violations(tokens, file == GUARD, &names, &mut found);
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
    /// file but the guard; so are libc's `kill` (as a path or in an import list) and `killpg`. An `#[allow]`, a
    /// `cfg(feature = "slow")` and a test module hide nothing. `Child::kill`, another `kill` and another libc name are not
    /// raw calls. The fixture is text: no call runs.
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
            }\n\
            #[allow(unsafe_code)]\n\
            fn g() { unsafe { libc::kill(-1, libc::SIGKILL); libc::killpg(1, libc::SIGKILL); } }\n\
            use libc::{getpid, kill};\n\
            fn h(child: &mut Child) { child.kill().unwrap(); test_budget::kill(&[]); libc::killer(); libc::getpid(); let _ = libc::(kill); }\n\
            use libc::{getpid as k};\n";
        for file in [
            "crates/botster-core/tests/slow_real_core.rs",
            "crates/botster-core-sys/src/process.rs",
            "xtask/src/test_budget.rs",
        ] {
            assert_eq!(lines(file, reverted), [4, 8, 9, 12, 12, 13], "{file}");
        }
        assert!(lines(GUARD, reverted).is_empty());
    }

    /// Red on revert of S1: a kill program as the program of a `Command::new`, by name, by path, as a raw string, with an
    /// escape, under an import rename or in a macro, is a violation, in the guard too. The same text elsewhere (a message name, an argument, a longer name) is not a program.
    #[test]
    fn a_kill_program_command_is_a_violation() {
        let text = "let a = Command::new(\"kill\");\nlet b = std::process::Command::new(\"/usr/bin/pkill\");\n\
            let c = Command::new(\n  r#\"killall\"#);\nlet d = { Command::new(r\"kill\") };\n\
            let ok = (\"kill\", Command::new(\"sh\").arg(\"kill\"), Command::new(\"skill\"), Command::new(\"killer\"));\n\
            let ok = (Command::new(KILL), Command::old(\"kill\"), Builder::new(\"kill\"), Command::new[\"kill\"]);\n\
            let ok = (Command:new(\"kill\"), Command;:new(\"kill\"), Command::new(b'k'), Command::new());\n\
            use std::process::Command as Proc;\nlet e = Proc::new(\"ki\\x6cl\");\nlet f = vec![Command::new(\"\\u{6b}ill\")];\n\
            let ok = Proc::new(\"true\");\n";
        assert_eq!(
            lines("xtask/src/test_budget.rs", text),
            [1, 2, 3, 5, 10, 11]
        );
        assert_eq!(lines(GUARD, text), [1, 2, 3, 5, 10, 11]);
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

    /// A string literal gives its value with every escape decoded; another literal gives none.
    #[test]
    fn a_string_literal_gives_its_decoded_value() {
        for literal in [
            "\"kill\"",
            "r\"kill\"",
            "r##\"kill\"##",
            "\"ki\\x6cl\"",
            "\"\\u{6b}ill\"",
            "\"k\\\nill\"",
        ] {
            assert_eq!(string_value(literal).as_deref(), Some("kill"), "{literal}");
        }
        for other in ["'k'", "b'k'", "b\"kill\"", "7", "\"kill", "kill\""] {
            assert_eq!(string_value(other), None, "{other}");
        }
    }

    /// Every name of `Command` in a file: the name itself, an import rename (in a list too) and a type alias.
    #[test]
    fn a_renamed_or_aliased_command_is_a_command_name() {
        let text =
            "use std::process::{Command as Proc, Stdio};\nuse std::process::Command as Run;\n\
            type Shell = std::process::Command;\ntype Other = Vec<u8>;\nlet x = Thing as Not;\n\
            let started = Command::new(\"true\");\n";
        let mut names = Vec::new();
        command_names(text.parse().unwrap(), &mut names);
        assert_eq!(names, ["Proc", "Run", "Shell"]);
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
