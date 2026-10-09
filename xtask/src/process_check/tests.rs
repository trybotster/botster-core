use super::*;

const TEST_FILE: &str = "crates/x/tests/a.rs";

/// The `(line, item, rule)` of each finding in `text`, as the file `file`.
fn found(file: &str, text: &str) -> Vec<(usize, String, &'static str)> {
    scan(file, text)
        .unwrap()
        .into_iter()
        .map(|f| (f.line, f.item, f.rule.name()))
        .collect()
}

fn rules(text: &str) -> Vec<&'static str> {
    found(TEST_FILE, text)
        .into_iter()
        .map(|(_, _, r)| r)
        .collect()
}

#[test]
fn each_banned_call_is_found_with_its_line_and_item() {
    let text = "\
fn t(mut child: Child, mut r: BufReader<File>) {
    child.wait().unwrap();
    child.try_wait().unwrap();
    child.wait_with_output().unwrap();
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    std::thread::sleep(d);
    Command::new(\"/bin/true\").spawn().unwrap();
    Command::new(\"/bin/true\").status().unwrap();
    Command::new(\"/bin/true\").arg(\"x\").output().unwrap();
}
";
    assert_eq!(
        found(TEST_FILE, text),
        [
            (2, "t".into(), "child-wait"),
            (3, "t".into(), "child-wait"),
            (4, "t".into(), "child-wait"),
            (6, "t".into(), "blocking-read"),
            (7, "t".into(), "sleep"),
            (8, "t".into(), "spawn"),
            (9, "t".into(), "command-wait"),
            (10, "t".into(), "command-wait"),
        ]
    );
}

#[test]
fn bounded_and_owned_calls_are_not_found() {
    let text = "\
use botster_test_process::{first_line, Deadline, OwnedChild};
fn t() {
    let mut child = OwnedChild::spawn(Command::new(\"/bin/true\").arg(\"x\")).unwrap();
    assert!(child.status().success());
    let (_, line) = first_line(child.take_stdout().unwrap());
    wake.wait(Duration::ZERO);
    condvar.wait(guard).unwrap();
    rig.driver.status();
    std::thread::spawn(|| ());
    std::thread::Builder::new().spawn(|| ()).unwrap();
    let text = \"asleep 1; sleeping; while read x; do :; done\";
}
";
    assert_eq!(found(TEST_FILE, text), []);
}

#[test]
fn a_renamed_or_glob_imported_or_qualified_call_is_found() {
    for (text, rule) in [
        (
            "use std::thread::sleep as nap;\nfn t() { nap(d); }\n",
            "sleep",
        ),
        ("use std::thread as t;\nfn f() { t::sleep(d); }\n", "sleep"),
        (
            "use std::thread::{self};\nfn f() { thread::sleep(d); }\n",
            "sleep",
        ),
        ("use std::thread::*;\nfn f() { sleep(d); }\n", "sleep"),
        (
            "use std::process::{Child as C};\nfn f() { C::wait(&mut c); }\n",
            "child-wait",
        ),
        ("fn f() { <Child>::wait(&mut c); }\n", "child-wait"),
        (
            "fn f() { children.iter_mut().map(Child::wait); }\n",
            "child-wait",
        ),
        (
            "fn f() { BufRead::read_line(&mut r, &mut s); }\n",
            "blocking-read",
        ),
        (
            "fn f() { <R as BufRead>::read_line(&mut r, &mut s); }\n",
            "blocking-read",
        ),
        (
            "use std::process::Command as Cmd;\nfn f() { Cmd::new(\"x\").output(); }\n",
            "command-wait",
        ),
        ("fn f() { Command::spawn(&mut c); }\n", "spawn"),
        (
            "fn f() { pty_process::blocking::Command::new(\"x\").spawn(pts); }\n",
            "spawn",
        ),
    ] {
        assert_eq!(rules(text), [rule], "{text}");
    }
    // A local function named `sleep`, with no import, is not the thread's.
    assert_eq!(
        rules("fn sleep(d: u8) {}\nfn f() { sleep(1); }\n"),
        Vec::<&str>::new()
    );
}

#[test]
fn a_command_bound_to_a_name_is_followed_within_its_function() {
    let text = "\
fn t() {
    let mut command = Command::new(\"x\");
    command.arg(\"y\");
    command.status().unwrap();
}
fn u() {
    command.status();
}
";
    assert_eq!(found(TEST_FILE, text), [(4, "t".into(), "command-wait")]);
}

#[test]
fn a_banned_call_inside_a_macro_is_found() {
    let text = "\
fn t() {
    assert!(child.wait().unwrap().success());
    let script = format!(\"{x}; /bin/sleep 1\");
    assert!(matches!(child.try_wait(), Ok(None)));
    custom! { let line = r.read_line(&mut s); thread::sleep(d); }
}
";
    assert_eq!(
        rules(text),
        [
            "child-wait",
            "shell-loop",
            "child-wait",
            "blocking-read",
            "sleep"
        ]
    );
}

#[test]
fn shell_sleeps_and_loops_that_never_end_by_themselves_are_found() {
    for text in [
        "sleep 1",
        "exec sleep 30",
        "/bin/sleep 0.5 & wait $!",
        "trap 'exit 7' USR1; while :; do :; done",
        "while true\ndo x; done",
        "while [ \"$PPID\" != 1 ] && kill -0 $PPID; do /bin/sleep 1 & wait $!; done",
    ] {
        assert!(shell_loop(text), "{text}");
    }
    for text in [
        "asleep 1",
        "sleeping",
        "sleep",
        "while read x; do :; done",
        "awhile :",
    ] {
        assert!(!shell_loop(text), "{text}");
    }
    let constant = "pub const LOOP: &str = \"while :; do :; done\";\n";
    assert_eq!(
        found(TEST_FILE, constant),
        [(1, "LOOP".into(), "shell-loop")]
    );
}

#[test]
fn test_code_is_a_test_file_a_test_item_or_the_testkit() {
    let banned = "fn f() { c.wait(); }\n";
    for file in [
        "crates/x/tests/a.rs",
        "crates/x/tests/common/mod.rs",
        "tests/suite/mod.rs",
        "crates/x/src/tests.rs",
        "crates/x/src/core/tests.rs",
        "crates/x/src/tests/driver/api.rs",
        "crates/x/src/a_test.rs",
        "crates/botster-core-testkit/src/process_group.rs",
        "xtask/src/base_merge/tests.rs",
    ] {
        assert_eq!(found(file, banned).len(), 1, "{file}");
    }
    for file in [
        "crates/x/src/a.rs",
        "crates/x/src/testing.rs",
        "crates/botster-test-process/src/child.rs",
        "crates/botster-test-process/tests/slow_process.rs",
        "xtask/src/ci.rs",
        "xtask/fixtures/mutants-hang/src/lib.rs",
        "crates/x/README.md",
    ] {
        assert_eq!(found(file, banned), [], "{file}");
    }
    // The xtask's commands run the gate's tools; its tests are test code.
    let xtask = "fn command() { c.wait(); }\n#[cfg(test)]\nmod tests { fn h() { c.wait(); } }\n";
    assert_eq!(
        found("xtask/src/ci.rs", xtask),
        [(3, "h".into(), "child-wait")]
    );
    let src = "crates/x/src/a.rs";
    let items = "\
fn production() { c.wait(); }
#[cfg(test)]
mod tests { fn helper() { c.wait(); } }
#[test]
fn t() { c.wait(); }
#[tokio::test]
async fn u() { c.wait(); }
#[cfg(all(test, unix))]
fn v() { c.wait(); }
#[cfg(any(test, unix))]
fn w() { c.wait(); }
#[cfg(not(test))]
fn x() { c.wait(); }
#[cfg(test)]
impl S { fn y(&self) { c.wait(); } }
";
    let items: Vec<String> = found(src, items)
        .into_iter()
        .map(|(_, item, _)| item)
        .collect();
    assert_eq!(items, ["helper", "t", "u", "v", "y"]);
}

#[test]
fn a_file_that_does_not_parse_fails_with_its_position() {
    let error = scan(TEST_FILE, "fn f( {\n").unwrap_err();
    assert!(error.starts_with("crates/x/tests/a.rs:"), "{error}");
    assert!(error.contains("does not parse"), "{error}");
    assert_eq!(
        scan("crates/botster-test-process/src/a.rs", "fn f( {\n"),
        Ok(Vec::new())
    );
}

#[test]
fn the_allowlist_needs_a_reason_a_known_rule_and_three_fields() {
    let text = "\
# PR C: migrates to botster-test-process (P3).
crates/x/tests/a.rs | t | child-wait
crates/x/tests/a.rs | t | child-wait

# Another reason.
crates/x/tests/b.rs | - | shell-loop
";
    let allowed = parse_allowlist(text).unwrap();
    assert_eq!(allowed.len(), 3);
    assert_eq!(
        allowed[2],
        Allowed {
            key: ("crates/x/tests/b.rs".into(), "-".into(), Rule::ShellLoop),
            line: 6
        }
    );
    assert_eq!(
        parse_allowlist("# reason\n\ncrates/x/tests/a.rs | t | sleep\n"),
        Err(format!(
            "{ALLOW_FILE}:3: an entry needs its reason in a comment above it"
        ))
    );
    assert_eq!(
        parse_allowlist("#\ncrates/x/tests/a.rs | t | sleep\n"),
        Err(format!(
            "{ALLOW_FILE}:2: an entry needs its reason in a comment above it"
        ))
    );
    assert_eq!(
        parse_allowlist("# r\ncrates/x/tests/a.rs | t | nap\n"),
        Err(format!("{ALLOW_FILE}:2: unknown rule `nap`"))
    );
    assert_eq!(
        parse_allowlist("# r\ncrates/x/tests/a.rs | sleep\n"),
        Err(format!(
            "{ALLOW_FILE}:2: an entry is `<file> | <item> | <rule>`"
        ))
    );
    assert_eq!(parse_allowlist(""), Ok(Vec::new()));
}

#[test]
fn one_entry_allows_one_site_and_an_entry_with_no_site_is_stale() {
    let findings = scan(TEST_FILE, "fn t() {\n    a.wait();\n    b.wait();\n}\n").unwrap();
    let entry = |line| Allowed {
        key: (TEST_FILE.into(), "t".into(), Rule::ChildWait),
        line,
    };
    assert_eq!(
        judge(&findings, &[entry(1), entry(2)]),
        Vec::<String>::new()
    );
    assert_eq!(
        judge(&findings, &[entry(1)]),
        [format!(
            "{TEST_FILE}:3: [child-wait] in `t`: {}",
            Rule::ChildWait.advice()
        )]
    );
    assert_eq!(
        judge(&findings, &[entry(1), entry(2), entry(3)]),
        [format!(
            "{ALLOW_FILE}:3: the entry `{TEST_FILE} | t | child-wait` allows no site: remove it"
        )]
    );
}

/// The red-on-revert proof (brief PR B): a migrated test is clean, and re-adding one banned call (the C3 shape: an
/// unbounded `read_line` of a child's pipe, then a raw `wait`) makes the check fail at that call.
#[test]
fn re_adding_a_banned_call_to_a_migrated_test_fails_the_check() {
    let migrated = "\
use botster_test_process::{first_line, Blocker, Guard, OwnedChild};

#[test]
fn an_early_exit_keeps_the_group_owned_until_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = Blocker::new(dir.path(), \"block\").unwrap();
    let mut child = OwnedChild::spawn_group(Command::new(\"/bin/sh\").args([\"-c\", &blocker.shell()])).unwrap();
    let (_rest, line) = first_line(child.take_stdout().unwrap());
    assert_eq!(line, \"up\\n\");
    assert!(child.status().success());
}
";
    let file = "crates/botster-core-sys/tests/common/process_guard.rs";
    let clean = scan(file, migrated).unwrap();
    assert_eq!(judge(&clean, &[]), Vec::<String>::new());
    let reverted = migrated.replace(
        "    let (_rest, line) = first_line(child.take_stdout().unwrap());\n",
        "    let mut line = String::new();\n    BufReader::new(stdout).read_line(&mut line).unwrap();\n    child.wait().unwrap();\n",
    );
    assert_ne!(reverted, migrated);
    let violations = judge(&scan(file, &reverted).unwrap(), &[]);
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(
        violations[0].starts_with(&format!("{file}:9: [blocking-read]")),
        "{violations:?}"
    );
    assert!(
        violations[1].starts_with(&format!("{file}:10: [child-wait]")),
        "{violations:?}"
    );
}

/// Lead ruling 2026-10-08 (after #169 F19): every blocking read with no deadline on a pipe, a socket or a channel is found,
/// in each form; a read of a file, `str::lines` and the bounded forms are not.
#[test]
fn every_blocking_read_without_a_deadline_is_found_and_a_file_read_is_not() {
    for text in [
        "fn f() { client.read_to_end(&mut out).unwrap(); }\n",
        "fn f() { stdout.read_to_string(&mut s).unwrap(); }\n",
        "fn f() { pipe.read_exact(&mut buf).unwrap(); }\n",
        "fn f() { for line in BufReader::new(stdout).lines() {} }\n",
        "fn f() { let reader = BufReader::new(stdout);\n for line in reader.lines() {} }\n",
        "fn f() { let (stream, _) = listener.accept().unwrap(); }\n",
        "fn f() { for stream in listener.incoming() {} }\n",
        "fn f() { let value = receiver.recv().unwrap(); }\n",
        "fn f() { let socket = UdpSocket::bind(a).unwrap();\n socket.recv(&mut buf).unwrap(); }\n",
        "fn f() { UnixDatagram::bind(p).unwrap().recv_from(&mut buf).unwrap(); }\n",
        "fn f() { children.map(Read::read_to_end); }\n",
        "use std::io::Read as R;\nfn f() { R::read_exact(&mut p, &mut b); }\n",
        "fn f() { std::io::read_to_string(stdout).unwrap(); }\n",
    ] {
        assert_eq!(rules(text), ["blocking-read"], "{text}");
    }
    for text in [
        "fn f() { File::open(p).unwrap().read_to_string(&mut s).unwrap(); }\n",
        "fn f() -> io::Result<()> { let mut f = File::open(p)?;\n f.read_to_end(&mut v)?; Ok(()) }\n",
        "fn f() { std::fs::OpenOptions::new().read(true).open(p).unwrap().read_exact(&mut b).unwrap(); }\n",
        "fn f() { let text = std::fs::read_to_string(p).unwrap(); }\n",
        "fn f() { for line in text.lines() {} }\n",
        "fn f() { receiver.recv_timeout(d).unwrap(); }\n",
        // The testkit's in-memory link does not block: `recv(buf)` of a receiver that is not a socket is allowed.
        "fn f() { while let Ok(n) = link.recv(&mut buf) {} }\n",
        "fn f() { Bounded::new(client).to_eof(Deadline::cleanup()).unwrap(); }\n",
    ] {
        assert_eq!(rules(text), Vec::<&str>::new(), "{text}");
    }
}

/// The red-on-revert proof of the widened rule, in the shape of #169 F19 (an unbounded `read_to_end` of a `UnixStream` in
/// botster-core real.rs): the bounded read is clean, and the revert to `read_to_end` fails the check at that call.
#[test]
fn re_adding_a_read_to_end_of_a_socket_fails_the_check() {
    let bounded = "\
use botster_test_process::{Bounded, Deadline};
use std::os::unix::net::UnixStream;

#[test]
fn the_service_answers_once_and_closes() {
    let client = UnixStream::connect(&path).unwrap();
    let answer = Bounded::new(client).to_eof(Deadline::cleanup()).unwrap();
    assert_eq!(answer, b\"ok\\n\");
}
";
    let file = "crates/botster-core/tests/real.rs";
    assert_eq!(
        judge(&scan(file, bounded).unwrap(), &[]),
        Vec::<String>::new()
    );
    let reverted = bounded.replace(
        "    let client = UnixStream::connect(&path).unwrap();\n    let answer = Bounded::new(client).to_eof(Deadline::cleanup()).unwrap();\n",
        "    let mut client = UnixStream::connect(&path).unwrap();\n    let mut answer = Vec::new();\n    client.read_to_end(&mut answer).unwrap();\n",
    );
    assert_ne!(reverted, bounded);
    let violations = judge(&scan(file, &reverted).unwrap(), &[]);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with(&format!(
            "{file}:8: [blocking-read] in `the_service_answers_once_and_closes`"
        )),
        "{violations:?}"
    );
}

/// A `#[cfg(test)]` item of each kind that can hold code makes that code test code; the same item without it does not.
#[test]
fn each_kind_of_test_item_holds_test_code() {
    let items = concat!(
        "#[cfg(test)]\nconst C: () = { c.wait(); };\n",
        "#[cfg(test)]\nenum E { A = { c.wait(); 0 } }\n",
        "#[cfg(test)]\nm! { c.wait() }\n",
        "#[cfg(test)]\nstatic S: () = { c.wait(); };\n",
        "#[cfg(test)]\nstruct T([u8; { c.wait(); 1 }]);\n",
        "#[cfg(test)]\ntrait U { fn f() { c.wait(); } }\n",
        "#[cfg(test)]\ntype V = [u8; { c.wait(); 1 }];\n",
        "#[cfg(test)]\nunion W { a: [u8; { c.wait(); 1 }] }\n",
    );
    let sites: Vec<(usize, String)> = found("crates/x/src/a.rs", items)
        .into_iter()
        .map(|(line, item, _)| (line, item))
        .collect();
    assert_eq!(
        sites,
        [
            (2, "C".into()),
            (4, "-".into()),
            (6, "-".into()),
            (8, "S".into()),
            (10, "-".into()),
            (12, "f".into()),
            (14, "-".into()),
            (16, "-".into()),
        ]
    );
    let production = items.replace("#[cfg(test)]\n", "");
    assert_eq!(found("crates/x/src/a.rs", &production), []);
}

/// `any()` with no member never holds, so it does not make test code.
#[test]
fn an_empty_any_is_not_a_test_cfg() {
    let text = "#[cfg(any())]\nfn f() { c.wait(); }\n";
    assert_eq!(found("crates/x/src/a.rs", text), []);
}

/// A test file's trait methods and statics are scanned, each under its own name.
#[test]
fn a_trait_method_and_a_static_are_scanned_with_their_names() {
    let text = concat!(
        "trait T { fn f() { c.wait(); } }\n",
        "static S: &str = \"sl",
        "eep 1\";\n"
    );
    assert_eq!(
        found(TEST_FILE, text),
        [(1, "f".into(), "child-wait"), (2, "S".into(), "shell-loop")]
    );
}

/// The kind of a receiver is followed through parentheses and references.
#[test]
fn a_command_in_parentheses_or_behind_a_reference_is_followed() {
    let text = "\
fn t() {
    (Command::new(\"x\")).status();
    let c = &Command::new(\"x\");
    c.output();
    Command::status(&mut c);
    d.spawn();
}
";
    assert_eq!(
        found(TEST_FILE, text),
        [
            (2, "t".into(), "command-wait"),
            (4, "t".into(), "command-wait"),
            (5, "t".into(), "command-wait"),
            (6, "t".into(), "spawn"),
        ]
    );
}

/// A macro body that does not parse as expressions is read token by token: a call at the first token is a path call, and
/// a method's arguments are counted by their commas.
#[test]
fn a_token_scan_counts_arguments_and_starts_at_the_first_token() {
    let text = "\
fn t() {
    m! { wait(); a.wait(); b.wait(x); c.read_line(&mut s); d.read_line(a, b); e.read_line(a, b,); }
}
";
    assert_eq!(
        found(TEST_FILE, text),
        [
            (2, "t".into(), "child-wait"),
            (2, "t".into(), "blocking-read")
        ]
    );
}

#[test]
fn arguments_are_counted_by_their_commas() {
    let count = |text: &str| count_args(text.parse().unwrap());
    assert_eq!(count(""), 0);
    assert_eq!(count("&mut s"), 1);
    assert_eq!(count("a, b"), 2);
    assert_eq!(count("a, b,"), 2);
    assert_eq!(count("f(a, b), c"), 2);
    // Only a comma separates: another punctuation mark is part of its argument.
    assert_eq!(count("a.len() + 1"), 1);
}

#[test]
fn a_path_is_the_identifiers_joined_by_double_colons_before_the_call() {
    let tokens: Vec<TokenTree> = "x a::b::c(d)"
        .parse::<TokenStream>()
        .unwrap()
        .into_iter()
        .collect();
    let end = tokens.iter().position(|t| t.to_string() == "c").unwrap();
    assert_eq!(path_before(&tokens, end), ["a", "b", "c"]);
    assert_eq!(path_before(&tokens, 0), ["x"]);
}

#[test]
fn a_violation_names_its_site_its_rule_and_the_advice() {
    let findings = scan(TEST_FILE, "fn t() { c.wait(); }\n").unwrap();
    assert_eq!(
        judge(&findings, &[]),
        [format!(
            "{TEST_FILE}:1: [child-wait] in `t`: a raw wait for a child: use botster_test_process::OwnedChild (status, \
             exited_within)"
        )]
    );
}

/// The command reads the tracked files and the allowlist of a repository: an unallowed site fails it; files that are not
/// checked are not scanned.
#[test]
fn the_check_scans_the_tracked_test_code_against_the_allowlist() {
    let site = "fn t() { c.wait(); }\n";
    let allow = "# The reason.\ncrates/x/tests/a.rs | t | child-wait\n";
    let repo = crate::fsutil::test_repo(&[
        ("crates/x/tests/a.rs", site),
        ("crates/x/src/lib.rs", "fn f() {}\n"),
        ("crates/botster-test-process/src/a.rs", site),
        ("crates/x/README.md", site),
    ]);
    let report = check(repo.path()).unwrap();
    assert_eq!(report.scanned, 2);
    assert_eq!(report.allowed, 0);
    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    assert_eq!(
        command(repo.path(), &[]).unwrap_err().to_string(),
        "1 violation(s)"
    );
    assert!(command(repo.path(), &["x".into()]).is_err());
    let allowed = crate::fsutil::test_repo(&[("crates/x/tests/a.rs", site), (ALLOW_FILE, allow)]);
    let report = check(allowed.path()).unwrap();
    assert_eq!((report.scanned, report.allowed), (1, 1));
    assert!(report.violations.is_empty());
    command(allowed.path(), &[]).unwrap();
}
