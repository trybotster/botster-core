//! Source guard: every timed wait or sleep in Core carries a timer marker.
//!
//! Core waits on events. A call that blocks for a duration is allowed only
//! when it is a real timer, and the source says which kind with a marker:
//!
//! ```text
//! // timer: deadline | backoff | rate-limit | ui-lifetime | os-no-event
//! ```
//!
//! The marker is a `//` comment on the call's line or on the line directly
//! above it, never further up, and its category is exactly one of the
//! allowed tokens. The guard reads Rust
//! syntax well enough to skip comments and string, raw-string, and char
//! literals, `fn` definitions of these names, and calls that set no timer:
//! a zero duration, which does not wait, and `None` or `Duration::MAX`, which
//! wait only for the event.

use std::path::{Path, PathBuf};

/// Calls that block the caller for a duration.
const TIMED_CALLS: &[&str] = &[
    "sleep",
    "park_timeout",
    "recv_timeout",
    "wait_timeout",
    "wait_timeout_while",
    "wait_timeout_ms",
    "wait_wakes",
    "wait_wakes_bounded",
    "wait_wakes_interruptible",
    "wait_pump",
    "set_read_timeout",
    "set_write_timeout",
    // `libc::poll` only; its timeout is the last argument.
    "poll",
];

const CATEGORIES: &[&str] = &[
    "deadline",
    "backoff",
    "rate-limit",
    "ui-lifetime",
    "os-no-event",
];

/// Arguments that set no timer: a zero duration does not wait, and `None`
/// or `Duration::MAX` waits only for the event.
const NO_TIMER_ARGS: &[&str] = &[
    "Duration::MAX",
    "std::time::Duration::MAX",
    "Duration::ZERO",
    "std::time::Duration::ZERO",
    "Duration::from_millis(0)",
    "std::time::Duration::from_millis(0)",
    "Duration::from_secs(0)",
    "std::time::Duration::from_secs(0)",
    "None",
];

/// Directory names the guard never enters: build output and vendored code.
const SKIPPED_DIRS: &[&str] = &["target", "vendor", "node_modules", ".git"];

/// Source split into code (comments and literals blanked, newlines kept)
/// and the text of the `//` comments on each line.
struct Lexed {
    code: String,
    line_comments: Vec<String>,
}

fn lex(source: &str) -> Lexed {
    let chars: Vec<char> = source.chars().collect();
    let mut code = String::with_capacity(source.len());
    let mut line_comments = vec![String::new()];
    let mut i = 0;
    let blank = |c: char, code: &mut String| code.push(if c == '\n' { '\n' } else { ' ' });
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '\n' {
            code.push('\n');
            line_comments.push(String::new());
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                line_comments
                    .last_mut()
                    .expect("a line is open")
                    .push(chars[i]);
                code.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    code.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    code.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if chars[i] == '\n' {
                        line_comments.push(String::new());
                    }
                    blank(chars[i], &mut code);
                    i += 1;
                }
            }
        } else if let Some(hashes) = raw_string_start(&chars, i) {
            // r"..", r#".."#, br"..": no escapes; ends at a quote plus hashes.
            let open = chars[i..].iter().position(|&c| c == '"').expect("quote") + i;
            for &c in &chars[i..=open] {
                blank(c, &mut code);
            }
            i = open + 1;
            while i < chars.len() {
                let closes =
                    chars[i] == '"' && (0..hashes).all(|h| chars.get(i + 1 + h) == Some(&'#'));
                if closes {
                    for _ in 0..=hashes {
                        code.push(' ');
                    }
                    i += 1 + hashes;
                    break;
                }
                if chars[i] == '\n' {
                    line_comments.push(String::new());
                }
                blank(chars[i], &mut code);
                i += 1;
            }
        } else if c == '"' {
            code.push(' ');
            i += 1;
            while i < chars.len() {
                match chars[i] {
                    '\\' => {
                        code.push(' ');
                        if let Some(&escaped) = chars.get(i + 1) {
                            if escaped == '\n' {
                                line_comments.push(String::new());
                            }
                            blank(escaped, &mut code);
                        }
                        i += 2;
                    }
                    '"' => {
                        code.push(' ');
                        i += 1;
                        break;
                    }
                    other => {
                        if other == '\n' {
                            line_comments.push(String::new());
                        }
                        blank(other, &mut code);
                        i += 1;
                    }
                }
            }
        } else if c == '\'' {
            // A char literal ('a', '\n', '\u{..}'), or else a lifetime.
            let literal_end = if next == Some('\\') {
                chars[i + 2..]
                    .iter()
                    .position(|&c| c == '\'')
                    .map(|p| i + 2 + p)
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            match literal_end {
                Some(end) => {
                    for _ in i..=end {
                        code.push(' ');
                    }
                    i = end + 1;
                }
                None => {
                    code.push(c);
                    i += 1;
                }
            }
        } else {
            code.push(c);
            i += 1;
        }
    }
    Lexed {
        code,
        line_comments,
    }
}

/// If a raw string starts at `i`, return its number of `#`s.
fn raw_string_start(chars: &[char], i: usize) -> Option<usize> {
    let starts_token = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
    if !starts_token {
        return None;
    }
    let mut j = i;
    if chars.get(j) == Some(&'b') {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    (chars.get(j) == Some(&'"')).then_some(hashes)
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A timed call with no marker: 1-based line and the called name.
#[derive(Debug, PartialEq, Eq)]
struct Unmarked {
    line: usize,
    call: String,
}

fn unmarked_timed_calls(source: &str) -> Vec<Unmarked> {
    let lexed = lex(source);
    let code: Vec<char> = lexed.code.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < code.len() {
        if !is_ident(code[i]) || (i > 0 && is_ident(code[i - 1])) {
            i += 1;
            continue;
        }
        let start = i;
        while i < code.len() && is_ident(code[i]) {
            i += 1;
        }
        let name: String = code[start..i].iter().collect();
        if !TIMED_CALLS.contains(&name.as_str()) {
            continue;
        }
        let mut open = i;
        while open < code.len() && code[open].is_whitespace() {
            open += 1;
        }
        if code.get(open) != Some(&'(') {
            continue;
        }
        let before: String = code[..start].iter().collect();
        if before.trim_end().ends_with("fn") {
            continue;
        }
        // Only the OS poll: a future's `poll(cx)` sets no timer.
        if name == "poll" && !before.ends_with("libc::") {
            continue;
        }
        let mut depth = 0;
        let mut close = open;
        while close < code.len() {
            match code[close] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            close += 1;
        }
        let args: String = code[open + 1..close.min(code.len())]
            .iter()
            .filter(|c| !c.is_whitespace())
            .collect();
        let args = args.trim_end_matches(',');
        // poll's timeout: -1 waits only for an event, 0 does not wait.
        if name == "poll" && matches!(last_top_level_arg(args), "-1" | "0") {
            continue;
        }
        if NO_TIMER_ARGS
            .iter()
            .any(|zero| *zero == args || args == format!("Some({zero})") && *zero != "None")
        {
            continue;
        }
        let line = code[..start].iter().filter(|&&c| c == '\n').count() + 1;
        if !marked(&lexed.line_comments, line) {
            found.push(Unmarked { line, call: name });
        }
    }
    found
}

/// The text after the last comma outside nested brackets.
fn last_top_level_arg(args: &str) -> &str {
    let mut depth = 0i32;
    let mut last = 0;
    for (index, c) in args.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => last = index + 1,
            _ => {}
        }
    }
    &args[last..]
}

/// A marker on the call's line, or on the line directly above it. A marker
/// further up does not count, even above other comment lines.
fn marked(line_comments: &[String], line: usize) -> bool {
    let has = |index: usize| {
        line_comments
            .get(index)
            .is_some_and(|comment| has_marker(comment))
    };
    // The call's own line, or the line directly above it; nothing further.
    has(line - 1) || (line >= 2 && has(line - 2))
}

fn has_marker(comment: &str) -> bool {
    comment.split("timer:").skip(1).any(|rest| {
        // The whole category token must be an allowed one: `deadline_typo`
        // is not `deadline`.
        let category: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        CATEGORIES.contains(&category.as_str())
    })
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("read source directory");
    for entry in entries {
        let path = entry.expect("directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !SKIPPED_DIRS.contains(&name) {
                rust_sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .to_path_buf()
}

#[test]
fn every_timed_wait_and_sleep_in_core_carries_a_timer_marker() {
    let root = crates_dir();
    let mut files = Vec::new();
    rust_sources(&root, &mut files);
    files.sort();
    assert!(
        files.len() > 50,
        "the guard must scan the Core crates, found {} files under {}",
        files.len(),
        root.display()
    );
    let mut failures = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read Rust source");
        for unmarked in unmarked_timed_calls(&source) {
            failures.push(format!(
                "{}:{}: {}(..) has no `// timer: <category>` marker",
                file.strip_prefix(&root).unwrap_or(file).display(),
                unmarked.line,
                unmarked.call
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} timed waits or sleeps without a timer marker:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_guard_flags_an_unmarked_sleep_and_an_unmarked_timed_wait() {
    let source = "fn f(rx: Receiver<()>) {\n\
                  \x20   std::thread::sleep(Duration::from_millis(5));\n\
                  \n\
                  \x20   let _ = rx\n\
                  \x20       .recv_timeout(Duration::from_secs(1));\n\
                  }\n";
    assert_eq!(
        unmarked_timed_calls(source),
        vec![
            Unmarked {
                line: 2,
                call: "sleep".into()
            },
            Unmarked {
                line: 5,
                call: "recv_timeout".into()
            },
        ]
    );
}

#[test]
fn the_guard_accepts_marked_calls_and_skips_what_does_not_wait() {
    let source = "fn wait_wakes(&self, timeout: Duration) {}\n\
                  fn f(rx: Receiver<()>) {\n\
                  \x20   // timer: deadline — expiry fails the test\n\
                  \x20   let _ = rx.recv_timeout(Duration::from_secs(1));\n\
                  \x20   let _ = rx\n\
                  \x20       // timer: backoff — retry spacing\n\
                  \x20       .recv_timeout(Duration::from_secs(1));\n\
                  \x20   thread::sleep(d); // timer: os-no-event — no readiness event\n\
                  \x20   let _ = source.wait_wakes(Duration::ZERO);\n\
                  \x20   let _ = source.wait_wakes(Duration::from_millis(0));\n\
                  \x20   stream.set_read_timeout(None);\n\
                  \x20   let _ = daemon.wait_pump(Duration::MAX);\n\
                  \x20   let text = \"thread::sleep(Duration::from_secs(1))\";\n\
                  \x20   let raw = r#\"rx.recv_timeout(d)\"#;\n\
                  \x20   // thread::sleep(Duration::from_secs(1));\n\
                  \x20   let quote = '\"'; let _ = rx.try_recv();\n\
                  }\n";
    assert_eq!(unmarked_timed_calls(source), Vec::new());
}

#[test]
fn a_marker_must_name_a_category_and_sit_directly_above_the_call() {
    let source = "fn f(rx: Receiver<()>) {\n\
                  \x20   // timer: soon\n\
                  \x20   let _ = rx.recv_timeout(d);\n\
                  \x20   // timer: deadline\n\
                  \n\
                  \x20   let _ = rx.recv_timeout(d);\n\
                  \x20   // timer: deadline\n\
                  \x20   let _ = rx\n\
                  \x20       .recv_timeout(d);\n\
                  }\n";
    assert_eq!(
        unmarked_timed_calls(source),
        vec![
            Unmarked {
                line: 3,
                call: "recv_timeout".into()
            },
            Unmarked {
                line: 6,
                call: "recv_timeout".into()
            },
            Unmarked {
                line: 9,
                call: "recv_timeout".into()
            },
        ]
    );
}

#[test]
fn the_guard_flags_an_os_poll_with_a_timeout_but_not_an_event_wait() {
    let source = "fn f(fds: &mut [libc::pollfd], t: i32, cx: &mut Context) {\n\
                  \x20   let _ = unsafe { libc::poll(fds.as_mut_ptr(), 1, -1) };\n\
                  \x20   let _ = unsafe { libc::poll(fds.as_mut_ptr(), 1, 0) };\n\
                  \x20   let _ = unsafe { libc::poll(fds.as_mut_ptr(), 1, t) };\n\
                  \x20   let _ = future.poll(cx);\n\
                  }\n";
    assert_eq!(
        unmarked_timed_calls(source),
        vec![Unmarked {
            line: 4,
            call: "poll".into()
        }]
    );
}

#[test]
fn a_marker_two_lines_above_or_with_a_category_prefix_does_not_count() {
    let source = "fn f(rx: Receiver<()>) {\n\
                  \x20   // timer: deadline — the reply\n\
                  \x20   // SAFETY: an unrelated note\n\
                  \x20   let _ = rx.recv_timeout(d);\n\
                  \x20   // timer: deadline_typo — not a category\n\
                  \x20   let _ = rx.recv_timeout(d);\n\
                  \x20   // timer: rate-limit — an allowed category with a dash\n\
                  \x20   let _ = rx.recv_timeout(d);\n\
                  }\n";
    assert_eq!(
        unmarked_timed_calls(source),
        vec![
            Unmarked {
                line: 4,
                call: "recv_timeout".into()
            },
            Unmarked {
                line: 6,
                call: "recv_timeout".into()
            },
        ]
    );
}
