//! `cargo xtask timers`: no sleeps in test code unless a line marks a deadline timer, and no timer in a machine crate
//! (BUILD.md Testing rule 5, plan 2.3c).
//!
//! A timer call in test code needs `// timer: deadline — <reason>` on the same line or the line directly above it.

use crate::fsutil::tracked_files;
use anyhow::{bail, Result};
use regex::Regex;
use std::path::Path;

/// The marker that allows a timer in test code; the reason after the dash must not be empty.
const MARKER: &str = "// timer: deadline \u{2014} ";

/// Crates that hold machines: a timer is never allowed in their production code (a machine reads the time it is given).
const MACHINE_CRATES: [&str; 5] = [
    "crates/botster-core-edges/",
    "crates/botster-core-link/",
    "crates/botster-worker-core/",
    "crates/botster-guardian-core/",
    "crates/botster-core-host/",
];

fn is_test_file(file: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    file.split('/').any(|part| matches!(part, "tests" | "test")) || name.ends_with("_test.rs")
}

fn has_timer(line: &str, re: &Regex) -> bool {
    let code = line.trim_start();
    !code.starts_with("//") && re.is_match(code)
}

fn marked(lines: &[&str], index: usize) -> bool {
    let has_marker = |line: &str| {
        line.find(MARKER)
            .is_some_and(|at| !line[at + MARKER.len()..].trim().is_empty())
    };
    has_marker(lines[index]) || (index > 0 && has_marker(lines[index - 1]))
}

/// Which lines are test code: an item with `#[cfg(test)]` or `#[test]`, from its attribute to the end of the item.
fn test_lines(lines: &[&str]) -> Vec<bool> {
    let attr = Regex::new(r"^\s*#\[\s*(?:cfg\(test\)|test)\s*\]").expect("regex");
    let mut in_test = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if !attr.is_match(lines[i]) {
            i += 1;
            continue;
        }
        let mut depth = 0i32;
        let mut opened = false;
        let mut end = lines.len() - 1;
        'item: for (j, line) in lines.iter().enumerate().skip(i) {
            let code = line.split("//").next().unwrap_or("");
            for c in code.chars() {
                match c {
                    '{' => {
                        depth += 1;
                        opened = true;
                    }
                    '}' => depth -= 1,
                    ';' if !opened => {
                        end = j;
                        break 'item;
                    }
                    _ => {}
                }
                if opened && depth <= 0 {
                    end = j;
                    break 'item;
                }
            }
        }
        for flag in &mut in_test[i..=end] {
            *flag = true;
        }
        i = end + 1;
    }
    in_test
}

/// The violations of one file: (1-based line, message).
fn scan(file: &str, text: &str, re: &Regex) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let file_is_test = is_test_file(file);
    let machine = MACHINE_CRATES.iter().any(|c| file.starts_with(c));
    let in_test = test_lines(&lines);
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !has_timer(line, re) {
            continue;
        }
        if file_is_test || in_test[index] {
            if !marked(&lines, index) {
                out.push((
                    index + 1,
                    "timer without `// timer: deadline \u{2014} <reason>` on this or the previous line".to_string(),
                ));
            }
        } else if machine {
            out.push((
                index + 1,
                "timer in a machine crate: a machine takes the time as an input".to_string(),
            ));
        }
    }
    out
}

fn timer_regex() -> Regex {
    Regex::new(r"(?:^|[^A-Za-z0-9_])(?:sleep|park_timeout|recv_timeout|wait_timeout)\s*\(")
        .expect("regex")
}

/// The violations over files given as `(path, text)`: `(file, 1-based line, message)`, and the number of Rust files scanned.
/// The xtask is tooling: its own waits are on child processes and carry no test timer.
pub fn scan_files(files: &[(String, String)], re: &Regex) -> (usize, Vec<(String, usize, String)>) {
    let mut scanned = 0;
    let mut found = Vec::new();
    for (file, text) in files {
        if !file.ends_with(".rs") || file.starts_with("xtask/") {
            continue;
        }
        scanned += 1;
        for (line, message) in scan(file, text, re) {
            found.push((file.clone(), line, message));
        }
    }
    (scanned, found)
}

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
    let (scanned, violations) = scan_files(&files, &timer_regex());
    for (file, line, message) in &violations {
        eprintln!("{file}:{line}: {message}");
    }
    println!("timers: {scanned} Rust files scanned");
    if !violations.is_empty() {
        bail!("{} violation(s)", violations.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations(file: &str, text: &str) -> Vec<usize> {
        scan(file, text, &timer_regex())
            .into_iter()
            .map(|(l, _)| l)
            .collect()
    }

    #[test]
    fn an_unmarked_sleep_in_a_test_file_is_a_violation() {
        assert_eq!(
            violations("crates/x/tests/a.rs", "fn f() {\n    sleep(d);\n}\n"),
            [2]
        );
    }

    #[test]
    fn a_marker_on_the_line_or_the_line_above_allows_the_timer() {
        let same = "sleep(d); // timer: deadline \u{2014} the child must not outlive the grace\n";
        assert!(violations("crates/x/tests/a.rs", same).is_empty());
        let above = "// timer: deadline \u{2014} reason\nsleep(d);\n";
        assert!(violations("crates/x/tests/a.rs", above).is_empty());
        let two_above = "// timer: deadline \u{2014} reason\n\nsleep(d);\n";
        assert_eq!(violations("crates/x/tests/a.rs", two_above), [3]);
    }

    #[test]
    fn a_marker_without_a_reason_does_not_count() {
        assert_eq!(
            violations(
                "crates/x/tests/a.rs",
                "// timer: deadline \u{2014} \nsleep(d);\n"
            ),
            [2]
        );
    }

    #[test]
    fn production_code_of_an_ordinary_crate_may_wait() {
        assert!(
            violations("crates/botster-core-sys/src/a.rs", "fn f() { sleep(d); }\n").is_empty()
        );
    }

    #[test]
    fn a_machine_crate_has_no_timer_outside_tests() {
        assert_eq!(
            violations(
                "crates/botster-core-link/src/a.rs",
                "fn f() {\n    sleep(d);\n}\n"
            ),
            [2]
        );
        let in_test = "#[cfg(test)]\nmod t {\n    fn f() {\n        sleep(d);\n    }\n}\n";
        assert_eq!(
            violations("crates/botster-core-link/src/a.rs", in_test),
            [4],
            "unmarked, in test code"
        );
    }

    #[test]
    fn a_comment_is_not_a_timer() {
        assert!(violations("crates/x/tests/a.rs", "// sleep(d)\n").is_empty());
    }

    #[test]
    fn an_identifier_that_ends_in_sleep_is_not_a_timer() {
        assert!(violations("crates/x/tests/a.rs", "fn f() { nosleep(d); }\n").is_empty());
    }

    #[test]
    fn a_timer_on_the_first_line_needs_its_marker_too() {
        assert_eq!(violations("crates/x/tests/a.rs", "sleep(d);\n"), [1]);
        let marked_first = "sleep(d); // timer: deadline \u{2014} reason\n";
        assert!(violations("crates/x/tests/a.rs", marked_first).is_empty());
    }

    #[test]
    fn test_code_ends_at_the_closing_brace_of_its_item() {
        let text = "#[test]\nfn t() {\n    if x {\n        y();\n    }\n    sleep(a);\n}\nfn prod() {\n    sleep(b);\n}\n";
        // Line 6 is in the test (unmarked: a violation); line 9 is production of an ordinary crate (allowed).
        assert_eq!(violations("crates/x/src/a.rs", text), [6]);
        let machine = violations("crates/botster-core-link/src/a.rs", text);
        assert_eq!(machine, [6, 9]);
    }

    #[test]
    fn a_test_attribute_on_a_statement_item_ends_at_its_semicolon() {
        let text = "#[cfg(test)]\nuse a::b;\nfn prod() {\n    sleep(b);\n}\n";
        assert_eq!(
            violations("crates/botster-core-link/src/a.rs", text),
            [4],
            "production code after a test `use`"
        );
        // In an ordinary crate production code may wait: the sleep is not test code.
        assert!(violations("crates/x/src/a.rs", text).is_empty());
    }

    #[test]
    fn an_item_without_a_closing_brace_runs_to_the_end_of_the_file() {
        let text = "#[test]\nfn t() {\n    let a = 1;\n    sleep(d);\n";
        assert_eq!(violations("crates/x/src/a.rs", text), [4]);
        assert_eq!(
            test_lines(&["#[test]", "fn t() {", "x;"]),
            [true, true, true]
        );
    }

    #[test]
    fn a_semicolon_inside_braces_does_not_end_the_item() {
        let lines = [
            "#[test]",
            "fn t() {",
            "    let a = 1;",
            "    b();",
            "}",
            "after",
        ];
        assert_eq!(test_lines(&lines), [true, true, true, true, true, false]);
    }

    #[test]
    fn braces_of_nested_items_are_balanced() {
        let text = "#[cfg(test)]\nmod t {\n    fn a() {\n        {\n        }\n    }\n    fn b() {\n        sleep(d);\n    }\n}\nfn prod() {\n    sleep(e);\n}\n";
        assert_eq!(
            violations("crates/botster-core-link/src/a.rs", text),
            [8, 12]
        );
    }

    #[test]
    fn a_test_attribute_with_its_item_on_the_same_line_is_test_code() {
        let text = "#[test] fn t() { sleep(d); }\nfn prod() {}\n";
        assert_eq!(violations("crates/x/src/a.rs", text), [1]);
    }

    #[test]
    fn test_lines_marks_exactly_the_item() {
        let lines = ["a", "#[test]", "fn t() {", "    x();", "}", "b"];
        assert_eq!(test_lines(&lines), [false, true, true, true, true, false]);
    }

    fn file(path: &str, text: &str) -> (String, String) {
        (path.to_string(), text.to_string())
    }

    #[test]
    fn files_are_scanned_with_their_path_and_line_and_xtask_is_skipped() {
        let files = [
            file("crates/x/tests/a.rs", "ok\nsleep(d);\n"),
            file("xtask/src/a.rs", "sleep(d);\n"),
            file("README.md", "sleep(d);\n"),
            file("crates/x/tests/b.rs", "fine\n"),
        ];
        let (scanned, found) = scan_files(&files, &timer_regex());
        assert_eq!(scanned, 2);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].0.as_str(), found[0].1),
            ("crates/x/tests/a.rs", 2)
        );
    }
}
