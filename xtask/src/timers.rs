//! `cargo xtask timers`: no sleeps in test code unless a marker names a deadline timer, and no timer in a machine crate
//! (BUILD.md Testing rule 5, plan 2.3c).
//!
//! A timer call in test code needs `// timer: deadline — <reason>` next to it: on a line of the statement that holds the
//! call, from the statement's first line to the call's line, or on the line directly above that statement. The
//! check parses the file (`syn`), so the marker binds to the statement, not to a line: a chain that rustfmt splits keeps
//! its marker (lead, 2026-10-08: the #162 gate went red when `.recv_timeout` moved two lines below the marker).

use crate::fsutil::tracked_files;
use anyhow::{bail, Result};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::path::Path;
use syn::visit::Visit;

/// The marker that allows a timer in test code; the reason after the dash must not be empty.
const MARKER: &str = "// timer: deadline \u{2014} ";

/// The calls that are timers.
const TIMERS: [&str; 4] = ["sleep", "park_timeout", "recv_timeout", "wait_timeout"];

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

/// Whether attributes make an item test code: `#[test]` or `#[cfg(test)]` (and `#[cfg(all(test, ..))]`).
fn is_test_item(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        if path.is_ident("cfg") {
            attr.parse_args::<syn::Meta>()
                .is_ok_and(|meta| requires_test(&meta))
        } else {
            path.segments
                .last()
                .is_some_and(|last| last.ident == "test")
        }
    })
}

fn requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|members| members.iter().any(requires_test)),
        _ => false,
    }
}

/// Whether line `index` (0-based) holds the marker with a reason.
fn has_marker(lines: &[&str], index: usize) -> bool {
    lines.get(index).is_some_and(|line| {
        line.find(MARKER)
            .is_some_and(|at| !line[at + MARKER.len()..].trim().is_empty())
    })
}

/// Whether the timer on line `call` (1-based), in the statement that starts on line `start`, is marked: on a line from
/// `start` to `call`, or on the line directly above `start`.
fn marked(lines: &[&str], start: usize, call: usize) -> bool {
    let start = start.clamp(1, call.max(1));
    (start - 1..=call).any(|line| line > 0 && has_marker(lines, line - 1))
}

struct Scan<'a> {
    lines: Vec<&'a str>,
    machine: bool,
    in_test: bool,
    /// The first lines of the statements that hold the current position, innermost last.
    statements: Vec<usize>,
    found: Vec<(usize, String)>,
}

impl Scan<'_> {
    fn timer(&mut self, call: usize) {
        if self.in_test {
            let start = self.statements.last().copied().unwrap_or(call);
            if !marked(&self.lines, start, call) {
                self.found.push((
                    call,
                    "timer without `// timer: deadline \u{2014} <reason>` on its statement or the line above it"
                        .to_string(),
                ));
            }
        } else if self.machine {
            self.found.push((
                call,
                "timer in a machine crate: a machine takes the time as an input".to_string(),
            ));
        }
    }

    /// A macro body that does not parse as expressions: each identifier in `TIMERS` followed by parentheses.
    fn scan_tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        for (i, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Group(group) => self.scan_tokens(group.stream()),
                TokenTree::Ident(ident) if TIMERS.contains(&ident.to_string().as_str()) => {
                    if matches!(tokens.get(i + 1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
                    {
                        self.timer(ident.span().start().line);
                    }
                }
                _ => {}
            }
        }
    }

    fn with_test(&mut self, test: bool, visit: impl FnOnce(&mut Self)) {
        let was = self.in_test;
        self.in_test |= test;
        visit(self);
        self.in_test = was;
    }
}

/// The attributes of an item that can hold code (a `use` holds none).
fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        _ => &[],
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        self.with_test(is_test_item(item_attrs(item)), |scan| {
            syn::visit::visit_item(scan, item);
        });
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.with_test(is_test_item(&item.attrs), |scan| {
            syn::visit::visit_impl_item_fn(scan, item);
        });
    }

    fn visit_stmt(&mut self, stmt: &'ast syn::Stmt) {
        let start = match stmt {
            syn::Stmt::Local(local) => local.let_token.span.start().line,
            syn::Stmt::Item(_) => {
                return syn::visit::visit_stmt(self, stmt);
            }
            syn::Stmt::Expr(expr, _) => first_line(expr),
            syn::Stmt::Macro(mac) => mac
                .mac
                .path
                .segments
                .first()
                .map_or(0, |s| s.ident.span().start().line),
        };
        self.statements.push(start);
        syn::visit::visit_stmt(self, stmt);
        self.statements.pop();
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if TIMERS.contains(&call.method.to_string().as_str()) {
            self.timer(call.method.span().start().line);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            if let Some(last) = path.path.segments.last() {
                if TIMERS.contains(&last.ident.to_string().as_str()) {
                    self.timer(last.ident.span().start().line);
                }
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        match mac.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
        ) {
            Ok(exprs) => {
                for expr in &exprs {
                    self.visit_expr(expr);
                }
            }
            Err(_) => self.scan_tokens(mac.tokens.clone()),
        }
    }
}

/// The first line of an expression: the start of its span (the span starts at its first token).
fn first_line(expr: &syn::Expr) -> usize {
    syn::spanned::Spanned::span(expr).start().line
}

/// The violations of one file: `(1-based line, message)`.
fn scan(file: &str, text: &str) -> Vec<(usize, String)> {
    let parsed = match syn::parse_file(text) {
        Ok(parsed) => parsed,
        Err(error) => {
            let line = error.span().start().line;
            return vec![(
                line,
                format!("does not parse, so it cannot be checked: {error}"),
            )];
        }
    };
    let mut scan = Scan {
        lines: text.lines().collect(),
        machine: MACHINE_CRATES.iter().any(|c| file.starts_with(c)),
        in_test: is_test_file(file),
        statements: Vec::new(),
        found: Vec::new(),
    };
    scan.visit_file(&parsed);
    let mut found = scan.found;
    found.sort();
    found.dedup();
    found
}

/// The violations over files given as `(path, text)`: `(file, 1-based line, message)`, and the number of Rust files scanned.
/// The xtask is tooling: its own waits are on child processes and carry no test timer.
pub fn scan_files(files: &[(String, String)]) -> (usize, Vec<(String, usize, String)>) {
    let mut scanned = 0;
    let mut found = Vec::new();
    for (file, text) in files {
        if !file.ends_with(".rs") || file.starts_with("xtask/") {
            continue;
        }
        scanned += 1;
        for (line, message) in scan(file, text) {
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
    let (scanned, violations) = scan_files(&files);
    println!("timers: {scanned} Rust files scanned");
    let violations: Vec<String> = violations
        .iter()
        .map(|(file, line, message)| format!("{file}:{line}: {message}"))
        .collect();
    crate::tools::verdict(&violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations(file: &str, text: &str) -> Vec<usize> {
        scan(file, text).into_iter().map(|(l, _)| l).collect()
    }

    const TEST: &str = "crates/x/tests/a.rs";

    #[test]
    fn an_unmarked_sleep_in_a_test_file_is_a_violation() {
        assert_eq!(violations(TEST, "fn f() {\n    sleep(d);\n}\n"), [2]);
    }

    #[test]
    fn a_marker_on_the_line_or_the_line_above_the_statement_allows_the_timer() {
        let same = "fn f() {\n    sleep(d); // timer: deadline \u{2014} the child must not outlive the grace\n}\n";
        assert!(violations(TEST, same).is_empty());
        let above = "fn f() {\n    // timer: deadline \u{2014} reason\n    sleep(d);\n}\n";
        assert!(violations(TEST, above).is_empty());
        let in_block = "fn f() {\n    // timer: deadline \u{2014} reason\n    // more about it\n    sleep(d);\n}\n";
        assert_eq!(
            violations(TEST, in_block),
            [4],
            "the marker is the line directly above, as before"
        );
        let two_above = "fn f() {\n    // timer: deadline \u{2014} reason\n\n    sleep(d);\n}\n";
        assert_eq!(violations(TEST, two_above), [4]);
        let other_statement =
            "fn f() {\n    // timer: deadline \u{2014} reason\n    a();\n    sleep(d);\n}\n";
        assert_eq!(violations(TEST, other_statement), [4]);
    }

    /// The #162 red: rustfmt split a chain, and the timed call moved two lines below the marker above the statement. The
    /// marker binds to the statement, so the split keeps it; a marker inside the chain, above the call, counts as well.
    #[test]
    fn a_marker_survives_a_chain_that_rustfmt_split() {
        let split = "fn f() {\n    // timer: deadline \u{2014} bounds the wait\n    let x = received\n        .recv_timeout(limit)\n        .unwrap();\n}\n";
        assert!(violations(TEST, split).is_empty());
        let inside = "fn f() {\n    let x = received\n        // timer: deadline \u{2014} bounds the wait\n        .recv_timeout(limit)\n        .unwrap();\n}\n";
        assert!(violations(TEST, inside).is_empty());
        let after = "fn f() {\n    let x = received\n        .recv_timeout(limit)\n        // timer: deadline \u{2014} too late\n        .unwrap();\n}\n";
        assert_eq!(violations(TEST, after), [3]);
    }

    #[test]
    fn a_marker_binds_to_the_innermost_statement() {
        let closure = "fn f() {\n    // timer: deadline \u{2014} reason\n    spawn(move || {\n        a();\n        sleep(d);\n    });\n}\n";
        assert_eq!(violations(TEST, closure), [5]);
    }

    #[test]
    fn a_marker_without_a_reason_does_not_count() {
        assert_eq!(
            violations(
                TEST,
                "fn f() {\n    // timer: deadline \u{2014} \n    sleep(d);\n}\n"
            ),
            [3]
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
    fn a_comment_or_a_string_is_not_a_timer() {
        assert!(violations(TEST, "// sleep(d)\nfn f() { let s = \"sleep(d)\"; }\n").is_empty());
    }

    #[test]
    fn an_identifier_that_ends_in_sleep_is_not_a_timer() {
        assert!(violations(TEST, "fn f() { nosleep(d); x.nosleep(d); }\n").is_empty());
    }

    #[test]
    fn method_and_path_timers_and_timers_in_macros_are_found() {
        let text = "fn f() {\n    rx.recv_timeout(d);\n    std::thread::park_timeout(d);\n    assert!(c.wait_timeout(g, d).is_ok());\n    custom! { let x = sleep(d); }\n}\n";
        assert_eq!(violations(TEST, text), [2, 3, 4, 5]);
    }

    #[test]
    fn test_code_is_the_test_item_only() {
        let text = "#[test]\nfn t() {\n    if x {\n        y();\n    }\n    sleep(a);\n}\nfn prod() {\n    sleep(b);\n}\n";
        // Line 6 is in the test (unmarked: a violation); line 9 is production of an ordinary crate (allowed).
        assert_eq!(violations("crates/x/src/a.rs", text), [6]);
        assert_eq!(
            violations("crates/botster-core-link/src/a.rs", text),
            [6, 9]
        );
        let after_use = "#[cfg(test)]\nuse a::b;\nfn prod() {\n    sleep(b);\n}\n";
        assert_eq!(
            violations("crates/botster-core-link/src/a.rs", after_use),
            [4]
        );
        assert!(violations("crates/x/src/a.rs", after_use).is_empty());
        let same_line = "#[test] fn t() { sleep(d); }\nfn prod() {}\n";
        assert_eq!(violations("crates/x/src/a.rs", same_line), [1]);
        let all = "#[cfg(all(test, unix))]\nfn t() { sleep(d); }\n#[cfg(not(test))]\nfn u() { sleep(d); }\n";
        assert_eq!(violations("crates/x/src/a.rs", all), [2]);
        let method = "struct S;\n#[cfg(test)]\nimpl S {\n    fn f(&self) { sleep(d); }\n}\n";
        assert_eq!(violations("crates/x/src/a.rs", method), [4]);
    }

    #[test]
    fn a_file_that_does_not_parse_is_a_violation() {
        let found = scan(TEST, "fn f( {\n");
        assert_eq!(found.len(), 1);
        assert!(found[0].1.starts_with("does not parse"), "{found:?}");
    }

    fn file(path: &str, text: &str) -> (String, String) {
        (path.to_string(), text.to_string())
    }

    #[test]
    fn files_are_scanned_with_their_path_and_line_and_xtask_is_skipped() {
        let files = [
            file("crates/x/tests/a.rs", "fn f() {\n    sleep(d);\n}\n"),
            file("xtask/src/a.rs", "fn f() { sleep(d); }\n"),
            file("README.md", "sleep(d);\n"),
            file("crates/x/tests/b.rs", "fn fine() {}\n"),
        ];
        let (scanned, found) = scan_files(&files);
        assert_eq!(scanned, 2);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].0.as_str(), found[0].1),
            ("crates/x/tests/a.rs", 2)
        );
    }

    /// A `#[cfg(test)]` item of each kind that can hold code makes its timers test timers, which need a marker; the same items
    /// in production code of a crate that is not a machine are not checked.
    #[test]
    fn each_kind_of_test_item_holds_test_timers() {
        let items = concat!(
            "#[cfg(test)]\nconst C: () = { sleep(d); };\n",
            "#[cfg(test)]\nenum E { A = { sleep(d); 0 } }\n",
            "#[cfg(test)]\nm! { sleep(d) }\n",
            "#[cfg(test)]\nmod t { fn f() { sleep(d); } }\n",
            "#[cfg(test)]\nstatic S: () = { sleep(d); };\n",
            "#[cfg(test)]\nstruct T([u8; { sleep(d); 1 }]);\n",
            "#[cfg(test)]\ntrait U { fn f() { sleep(d); } }\n",
        );
        let src = "crates/x/src/a.rs";
        assert_eq!(violations(src, items), [2, 4, 6, 8, 10, 12, 14]);
        assert!(violations(src, &items.replace("#[cfg(test)]\n", "")).is_empty());
    }

    /// A macro body that does not parse as expressions is read token by token, into nested groups; only a timer name followed
    /// by parentheses is a timer.
    #[test]
    fn a_token_scan_finds_timers_in_nested_groups_and_only_timers() {
        let text = "fn f() {\n    m! { a; [sleep(d)]; wait(d); sleep; }\n}\n";
        assert_eq!(violations(TEST, text), [2]);
        let other = "fn f() {\n    m! { a; wait(d); }\n}\n";
        assert!(violations(TEST, other).is_empty());
    }

    /// The command checks the tracked files of a repository.
    #[test]
    fn the_command_fails_on_an_unmarked_timer_in_a_tracked_test_file() {
        let unmarked = crate::fsutil::test_repo(&[(TEST, "fn f() {\n    sleep(d);\n}\n")]);
        assert_eq!(
            command(unmarked.path(), &[]).unwrap_err().to_string(),
            "1 violation(s)"
        );
        let clean = crate::fsutil::test_repo(&[(TEST, "fn f() {}\n")]);
        command(clean.path(), &[]).unwrap();
        assert!(command(clean.path(), &["x".into()]).is_err());
    }
}
