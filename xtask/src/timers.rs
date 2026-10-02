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

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let re = timer_regex();
    let mut violations = 0;
    let mut scanned = 0;
    for file in tracked_files(root)? {
        // The xtask is tooling: its own waits are on child processes and carry no test timer.
        if !file.ends_with(".rs") || file.starts_with("xtask/") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
            continue;
        };
        scanned += 1;
        for (line, message) in scan(&file, &text, &re) {
            eprintln!("{file}:{line}: {message}");
            violations += 1;
        }
    }
    println!("timers: {scanned} Rust files scanned");
    if violations > 0 {
        bail!("{violations} violation(s)");
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
}
