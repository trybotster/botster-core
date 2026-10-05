//! The fixture processes of `tests/slow_real_harness.rs`: one role for each process that a test needs and cannot be itself.
//! A binary of its own, so that no role is reported as a test (audit A51). `argv[1]` names the role. The roles that own a
//! harness or a guard take the outer test's candidate directory as `argv[2]`.
//!
//! - `move-group`: moves itself to a new process group, says `moved`, and waits for the end of its stdin.
//! - `fds`: prints its open descriptors on one line.
//! - `own-harness`: a test that owns a `RealCoreHarness` with one running session. It prints the anchors' reports and
//!   `ready`, then waits for the end of its stdin. The outer test kills it, so its cleanup never runs.
//! - `own-guard`: a test that owns an `AnchorGuard` and starts one wrapper itself. It prints the wrapper's identity and
//!   `ready` once the real program runs, and never asks the guard for its anchors: it dies before it registers them.

#[path = "../common/mod.rs"]
mod common;

use botster_core_contract::prelude::CoreLimits;
use botster_core_testkit::anchor::Line;
use botster_core_testkit::candidate::PROBE;
use botster_core_testkit::real::AnchorGuard;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

fn wait_for_end_of_stdin() {
    let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
}

fn move_group() {
    rustix::process::setpgid(None, None).expect("a new group");
    println!("moved");
    std::io::stdout().flush().unwrap();
    wait_for_end_of_stdin();
}

fn fds() {
    let dir = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };
    let mut fds: Vec<u32> = std::fs::read_dir(dir)
        .expect("the descriptor directory")
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    fds.sort_unstable();
    println!("{fds:?}");
}

fn candidate_dir() -> std::path::PathBuf {
    std::env::args_os()
        .nth(2)
        .expect("the candidate directory as argv[2]")
        .into()
}

fn own_harness() {
    let mut harness = common::harness_in(&candidate_dir());
    let mut core = common::open(&mut harness);
    common::start(&harness, core.as_mut(), "s", common::stubborn(None));
    for report in harness
        .await_anchors(2)
        .expect("the worker and the program")
    {
        println!("{}", Line::Anchor(report).encode());
    }
    println!("ready");
    std::io::stdout().flush().unwrap();
    wait_for_end_of_stdin();
}

fn own_guard() {
    let candidate = common::candidate_in(&candidate_dir());
    let mut guard = AnchorGuard::new(&candidate.anchor).expect("a guard");
    let wrapper = guard
        .wrapper(PROBE, &candidate.probe, CoreLimits::default().stop_grace)
        .expect("a wrapper");
    let mut child = Command::new(wrapper)
        .arg(common::script(common::stubborn(Some("running\n"))))
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the wrapper starts");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("a pipe"))
        .read_line(&mut line)
        .expect("the probe runs");
    assert_eq!(line, "running\n");
    let pid = child.id();
    let start_time = botster_core_sys::process::start_time(pid).expect("the probe runs");
    println!(
        "{}",
        serde_json::json!({"pid": pid, "start_time": start_time})
    );
    println!("ready");
    std::io::stdout().flush().unwrap();
    wait_for_end_of_stdin();
    // Reached only when the outer test closes stdin instead of killing this process.
    drop(guard);
    let _ = child.wait();
}

fn main() {
    let role = std::env::args().nth(1).unwrap_or_default();
    match role.as_str() {
        "move-group" => move_group(),
        "fds" => fds(),
        "own-harness" => own_harness(),
        "own-guard" => own_guard(),
        other => {
            eprintln!("unknown fixture role `{other}`");
            std::process::exit(64);
        }
    }
}
