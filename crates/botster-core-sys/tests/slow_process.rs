//! The real `Process` edge (plan 2.3), proved with real processes. Slow tier: each test starts a child in its own process
//! group, and ends it on every exit path.
//!
//! Clause: Core AD-6 (identity by pid and start time; a reused pid is never signalled), Core LC-5 and SV-9 (the kill of a
//! group).
#![cfg(feature = "slow")]

use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnSpec,
};
use botster_core_sys::process::{identity_state, start_time, Children};

fn spec(program: &str, args: &[&str]) -> SpawnSpec {
    SpawnSpec {
        program: program.into(),
        args: args.iter().map(Into::into).collect(),
        env: vec![("A".into(), "1".into())],
        cwd: None,
    }
}

/// Ends the child on every exit path, panics included.
struct Reaper<'a>(&'a mut Children, ProcessIdentity);

impl Drop for Reaper<'_> {
    fn drop(&mut self) {
        self.0.signal_group(self.1, GroupSignal::Kill);
        // Reap what ended, so no zombie is left. The group was killed, so the wait ends.
        let _ = self.0.wait_exit(self.1.pid);
    }
}

/// Core AD-6: a spawned child has an identity that matches while it lives, and the kill of its group ends it with the signal.
#[test]
fn a_spawned_child_matches_its_identity_and_dies_by_the_group_kill() {
    let mut children = Children::new();
    let identity = children.spawn(&spec("/bin/sleep", &["30"])).expect("spawn");
    let reaper = Reaper(&mut children, identity);
    assert_eq!(identity_state(identity), IdentityState::Matches);
    assert_eq!(start_time(identity.pid), Some(identity.start_time));
    reaper.0.signal_group(identity, GroupSignal::Kill);
    let exit = reaper.0.wait_exit(identity.pid).expect("the child ends");
    assert_eq!(exit, (identity, ExitStatus::Signal(9)));
    std::mem::forget(reaper);
}

/// Core AD-6: a process that does not match its identity is never signalled.
#[test]
fn a_reused_identity_is_never_signalled() {
    let mut children = Children::new();
    let identity = children.spawn(&spec("/bin/sleep", &["30"])).expect("spawn");
    let reaper = Reaper(&mut children, identity);
    let stale = ProcessIdentity {
        pid: identity.pid,
        start_time: identity.start_time + 1,
    };
    assert_eq!(identity_state(stale), IdentityState::Reused);
    reaper.0.signal_group(stale, GroupSignal::Kill);
    assert_eq!(
        identity_state(identity),
        IdentityState::Matches,
        "the child still lives"
    );
}

/// Core LC-1, AD-7: a program that cannot be run is a typed spawn error with the errno, and the exit code of a child that ends
/// by itself is reported.
#[test]
fn a_missing_program_is_a_spawn_error_and_an_exit_code_is_reported() {
    let mut children = Children::new();
    let error = children
        .spawn(&spec("/botster-no-such-program", &[]))
        .expect_err("refused");
    assert_eq!(error.errno, 2, "ENOENT");
    let identity = children
        .spawn(&spec("/bin/sh", &["-c", "exit 3"]))
        .expect("spawn");
    let reaper = Reaper(&mut children, identity);
    let exit = reaper.0.wait_exit(identity.pid).expect("the child ends");
    assert_eq!(exit.1, ExitStatus::Code(3));
    std::mem::forget(reaper);
}

/// Core A2-1: the child has exactly the environment that it was given.
#[test]
fn the_child_environment_is_exact() {
    let mut children = Children::new();
    let identity = children
        .spawn(&spec(
            "/bin/sh",
            &["-c", "test \"$A\" = 1 && test -z \"$HOME\""],
        ))
        .expect("spawn");
    let reaper = Reaper(&mut children, identity);
    let exit = reaper.0.wait_exit(identity.pid).expect("the child ends");
    assert_eq!(exit.1, ExitStatus::Code(0), "nothing is inherited");
    std::mem::forget(reaper);
}
