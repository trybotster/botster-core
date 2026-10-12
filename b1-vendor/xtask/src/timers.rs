//! `cargo xtask timers`: no sleeps in test code or fakes unless a line marks a deadline timer
//! (BUILD.md Testing rule 5, design section 9).
//!
//! A timer call needs `// timer: deadline — <reason>` on the same line or the line before.
//! In a fake crate, outside its test code (an item with `#[cfg(test)]` or `#[test]`), a timer is never allowed (a fake takes an injected clock).

use crate::fsutil::tracked_files;
use anyhow::{bail, Result};
use regex::Regex;
use std::path::Path;

/// The marker that allows a timer in test code; the reason after the dash must not be empty.
const MARKER: &str = "// timer: deadline \u{2014} ";

/// Tracked files that are never scanned: the frozen contracts and the process prose.
const SKIPPED_PREFIXES: [&str; 2] = ["frozen/", "docs/"];

const SCANNED_EXTENSIONS: [&str; 5] = [".rs", ".ts", ".tsx", ".js", ".mjs"];

#[derive(Debug, PartialEq)]
enum Kind {
    /// Test code: a marked timer is allowed.
    Test,
    /// Production code of a fake crate: no timer at all.
    Fake,
}

fn is_fake_crate(file: &str) -> bool {
    file.starts_with("crates/botster-fake-") || file.starts_with("crates/botster-plugin-fakes/")
}

fn is_test_file(file: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    file.split('/')
        .any(|part| matches!(part, "tests" | "test" | "__tests__"))
        || name.ends_with("_test.rs")
        || name.contains(".test.")
        || name.contains(".spec.")
}

/// A timer call in one line of code.
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

/// Which lines are test code in a Rust file: an item (a module, a function, a `use`) that carries
/// `#[cfg(test)]`, `#[test]` or `#[tokio::test]`, from its attribute to the end of the item.
/// The end of the item is the closing brace that matches its first brace, or its `;`.
/// Braces inside string and char literals are ignored; a multi-line raw string that holds a brace can confuse the count.
fn test_lines(lines: &[&str]) -> Vec<bool> {
    let attr =
        Regex::new(r"^\s*#\[\s*(?:cfg\(test\)|test|tokio::test\b[^\]]*)\s*\]").expect("regex");
    let leading_attrs = Regex::new(r"^\s*(?:#\[[^\]]*\]\s*)+").expect("regex");
    let literals = Regex::new(r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)'"#).expect("regex");
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
            let code = leading_attrs.replace(line, "");
            let code = literals.replace_all(&code, "");
            let code = code.split("//").next().unwrap_or("");
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
    let fake = is_fake_crate(file);
    let in_test = if file.ends_with(".rs") {
        test_lines(&lines)
    } else {
        vec![false; lines.len()]
    };
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !has_timer(line, re) {
            continue;
        }
        let kind = if file_is_test || in_test[index] {
            Kind::Test
        } else if fake {
            Kind::Fake
        } else {
            continue;
        };
        match kind {
            Kind::Fake => out.push((
                index + 1,
                "timer in a fake: a fake takes an injected clock".to_string(),
            )),
            Kind::Test if !marked(&lines, index) => out.push((
                index + 1,
                "timer without `// timer: deadline \u{2014} <reason>` on this or the previous line"
                    .to_string(),
            )),
            Kind::Test => {}
        }
    }
    out
}

fn timer_regex() -> Regex {
    Regex::new(r"(?:^|[^A-Za-z0-9_])(?:sleep|setTimeout|setInterval)\s*\(").expect("regex")
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let re = timer_regex();
    let mut lines = Vec::new();
    let mut scanned = 0;
    for file in tracked_files(root)? {
        if SKIPPED_PREFIXES.iter().any(|p| file.starts_with(p))
            || !SCANNED_EXTENSIONS.iter().any(|e| file.ends_with(e))
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
            continue;
        };
        scanned += 1;
        for (line, message) in scan(&file, &text, &re) {
            lines.push(format!("{file}:{line}: {message}"));
        }
    }
    let scan = format!("timers: {scanned} files scanned");
    if !lines.is_empty() {
        let summary = format!("{} hit(s)", lines.len());
        return Err(crate::Problems::fail_after(
            lines,
            format!("{scan}\n"),
            summary,
        ));
    }
    println!("{scan}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The test sources are built from these parts, so this file holds no timer call itself.
    const SLEEP: &str = concat!("sle", "ep(d);");
    const TIMEOUT: &str = concat!("setTime", "out(r, 5);");
    const INTERVAL: &str = concat!("setInter", "val(f, 5);");

    fn scan_text(file: &str, text: &str) -> Vec<usize> {
        scan(file, text, &timer_regex())
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    #[test]
    fn an_unmarked_timer_in_a_test_file_fails() {
        let rust = format!("fn f() {{\n    std::thread::{SLEEP}\n}}\n");
        assert_eq!(scan_text("crates/a/tests/t.rs", &rust), vec![2]);
        assert_eq!(scan_text("xtask/tests/t.rs", &rust), vec![2]);
        assert_eq!(
            scan_text("ts/x.test.ts", &format!("await {TIMEOUT}\n")),
            vec![1]
        );
        assert_eq!(scan_text("ts/x.test.ts", &format!("{INTERVAL}\n")), vec![1]);
    }

    #[test]
    fn a_marker_on_the_same_or_previous_line_allows_it() {
        let same = format!("{SLEEP} // timer: deadline \u{2014} outlast the step deadline\n");
        let prev = format!("// timer: deadline \u{2014} reason\n{SLEEP}\n");
        assert!(scan_text("crates/a/tests/t.rs", &same).is_empty());
        assert!(scan_text("crates/a/tests/t.rs", &prev).is_empty());
        let empty = format!("// timer: deadline \u{2014} \n{SLEEP}\n");
        assert_eq!(scan_text("crates/a/tests/t.rs", &empty), vec![2]);
        let far = format!("// timer: deadline \u{2014} r\n\n{SLEEP}\n");
        assert_eq!(scan_text("crates/a/tests/t.rs", &far), vec![3]);
    }

    #[test]
    fn production_code_outside_a_fake_is_not_scanned() {
        assert!(scan_text("crates/a/src/lib.rs", &format!("fn f() {{ {SLEEP} }}\n")).is_empty());
    }

    #[test]
    fn a_cfg_test_module_is_test_code() {
        let text =
            format!("fn f() {{}}\n#[cfg(test)]\nmod tests {{\n    fn t() {{ {SLEEP} }}\n}}\n");
        assert_eq!(scan_text("crates/a/src/lib.rs", &text), vec![4]);
    }

    #[test]
    fn a_test_attribute_on_one_item_scopes_only_that_item() {
        let text = format!(
            "#[test]\nfn t() {{\n    let s = \"}}\";\n    {SLEEP}\n}}\nfn prod() {{\n    {SLEEP}\n}}\n"
        );
        assert_eq!(scan_text("crates/a/src/lib.rs", &text), vec![4]);
        assert_eq!(
            scan_text("crates/botster-fake-core/src/lib.rs", &text),
            vec![4, 7]
        );
    }

    #[test]
    fn a_fake_never_allows_a_timer_in_production_code_even_after_a_test_module() {
        let marked = "// timer: deadline \u{2014} r";
        let text = format!(
            "#[cfg(test)]\nmod tests {{\n    fn t() {{ {SLEEP} }} {marked}\n}}\nfn prod() {{ {SLEEP} }} {marked}\n"
        );
        assert_eq!(
            scan_text("crates/botster-fake-core/src/lib.rs", &text),
            vec![5]
        );
        assert_eq!(
            scan_text("crates/botster-plugin-fakes/src/lib.rs", &text),
            vec![5]
        );
    }

    #[test]
    fn names_that_only_contain_the_word_and_comments_do_not_match() {
        let text = format!("fn asleep(x: u8) {{}}\nlet n = sleeper(1);\n// {SLEEP} is banned\nfn sleep_ms() {{}}\n");
        assert!(scan_text("crates/a/tests/t.rs", &text).is_empty());
    }
}
