//! Test ownership of a process group, independent of the production reaper.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// The anchor joins the worker's group before the worker body runs.
/// The anchor stays alive after the production reaper reaps the worker.
/// Closing the control stream makes the anchor kill its group, including itself.
pub struct GroupGuard {
    control: UnixStream,
    anchor: Child,
    registration: Option<std::thread::JoinHandle<()>>,
    socket: PathBuf,
}

fn helper(name: &str) -> String {
    let module = module_path!().split_once("::").unwrap().1;
    format!("{module}::{name}")
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

impl GroupGuard {
    /// Creates ownership before the production spawn can run.
    pub fn new(dir: &Path) -> Self {
        let socket = dir.join("guard.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let (control, child_control) = UnixStream::pair().unwrap();
        let input: OwnedFd = child_control.try_clone().unwrap().into();
        let output: OwnedFd = child_control.into();
        let anchor = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &helper("anchor_process"), "--nocapture"])
            .env("BOTSTER_TEST_ANCHOR", "1")
            .stdin(Stdio::from(input))
            .stderr(Stdio::from(output))
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let mut registration_control = control.try_clone().unwrap();
        let registration = std::thread::spawn(move || {
            let Ok((mut worker, _)) = listener.accept() else {
                return;
            };
            let mut pid = String::new();
            if BufReader::new(&worker).read_line(&mut pid).is_err() || pid.is_empty() {
                return;
            }
            if registration_control.write_all(pid.as_bytes()).is_err() {
                return;
            }
            let mut ready = [0];
            if registration_control.read_exact(&mut ready).is_ok() && ready == [1] {
                let _ = worker.write_all(&ready);
            }
        });
        Self {
            control,
            anchor,
            registration: Some(registration),
            socket,
        }
    }

    /// The shell waits for registration before it starts any blocking command.
    pub fn prefix(&self) -> String {
        format!(
            "BOTSTER_TEST_GROUP_SOCKET={} BOTSTER_TEST_GROUP_PID=$$ {} --exact {} --nocapture >/dev/null 2>/dev/null || exit 1\n",
            quoted(&self.socket),
            quoted(&std::env::current_exe().unwrap()),
            helper("register_worker")
        )
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        // Shutdown also closes the writing side of the registration thread's clone.
        let _ = self.control.shutdown(std::net::Shutdown::Write);
        // Release accept if no worker reached registration.
        if let Ok(stream) = UnixStream::connect(&self.socket) {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(thread) = self.registration.take() {
            let _ = thread.join();
        }
        let _ = self.anchor.wait();
    }
}

/// A separate test process holds membership in the worker's group.
#[test]
fn anchor_process() {
    if std::env::var_os("BOTSTER_TEST_ANCHOR").is_none() {
        return;
    }
    let mut input = BufReader::new(std::io::stdin());
    let mut pid = String::new();
    if input.read_line(&mut pid).unwrap() == 0 {
        return;
    }
    let group = rustix::process::Pid::from_raw(pid.trim().parse().unwrap()).unwrap();
    rustix::process::setpgid(None, Some(group)).unwrap();
    let _ = std::io::stderr().write_all(&[1]);
    // EOF is the test's Drop or death. No timer or parent-PID check is needed.
    let mut remaining = Vec::new();
    let _ = input.read_to_end(&mut remaining);
    end_group(group);
}

/// The limit of the anchor's cleanup: the guards' cleanup limit. Not a contract value.
const CLEANUP: std::time::Duration = std::time::Duration::from_secs(10);

/// Ends every member of `group`, the group of this process. One kill of the group is not enough: on macOS a child whose
/// fork completes after the kill escapes it and stays in the group. So the kill is repeated until no live member is left.
///
/// Each kill is a signal to the group, never to a pid, so it reaches members only. The group id stays reserved through
/// every signal: a child of this process stays in the group, unreaped, while this process leaves the group and signals it
/// from outside, so no other group can take the id before the end.
pub(crate) fn end_group(group: rustix::process::Pid) {
    let kill = || {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    };
    // The reserve inherits this process's group. A failed spawn or move leaves the single kill, which ends this process too.
    let Ok(mut reserve) = std::process::Command::new("/usr/bin/true").spawn() else {
        return kill();
    };
    if rustix::process::setpgid(None, None).is_err() {
        return kill();
    }
    // timer: deadline — bounds the anchor's cleanup; a member that never ends cannot hold the anchor forever.
    let deadline = std::time::Instant::now() + CLEANUP;
    end_members(
        kill,
        || live_members(group),
        |&pid| await_end(pid, deadline),
        || std::time::Instant::now() >= deadline,
    );
    // Every member has ended; the reserve is a zombie of this process and gives the id back.
    let _ = reserve.wait();
}

/// The decision of `end_group`: each round kills the group, lists the members that are still live and waits for their
/// ends. A round that lists none is final, because only a live member can fork a new one; a member that joins after a kill
/// is in a later round's list, and the next kill reaches it. It stops at `expired`, so it never runs without end.
fn end_members<M>(
    mut kill: impl FnMut(),
    mut members: impl FnMut() -> Vec<M>,
    mut await_end: impl FnMut(&M),
    mut expired: impl FnMut() -> bool,
) {
    loop {
        kill();
        let live = members();
        if live.is_empty() || expired() {
            return;
        }
        for member in &live {
            await_end(member);
        }
    }
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. A process that is already gone has
/// ended. It only observes: a pid that was reused meanwhile can make it wait for another process, within the deadline.
#[cfg(target_os = "macos")]
fn await_end(pid: rustix::process::Pid, deadline: std::time::Instant) {
    let Ok(mut watcher) = kqueue::Watcher::new() else {
        return;
    };
    let added = watcher.add_pid(
        pid.as_raw_nonzero().get(),
        kqueue::EventFilter::EVFILT_PROC,
        kqueue::FilterFlag::NOTE_EXIT,
    );
    if added.is_err() || watcher.watch().is_err() {
        return;
    }
    // timer: deadline — bounds the wait for a killed member's exit event.
    let _ = watcher.poll(Some(
        deadline.saturating_duration_since(std::time::Instant::now()),
    ));
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. A process that is already gone has
/// ended. It only observes: a pid that was reused meanwhile can make it wait for another process, within the deadline.
#[cfg(target_os = "linux")]
fn await_end(pid: rustix::process::Pid, deadline: std::time::Instant) {
    let Ok(pidfd) = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) else {
        return;
    };
    let left = deadline.saturating_duration_since(std::time::Instant::now());
    // timer: deadline — bounds the wait for a killed member's exit event.
    let limit = rustix::event::Timespec {
        tv_sec: left.as_secs() as i64,
        tv_nsec: left.subsec_nanos().into(),
    };
    let mut fds = [rustix::event::PollFd::new(
        &pidfd,
        rustix::event::PollFlags::IN,
    )];
    let _ = rustix::event::poll(&mut fds, Some(&limit));
}

/// The live members of `group`. A zombie is not live: it cannot fork, and its parent reaps it.
#[cfg(target_os = "macos")]
fn live_members(group: rustix::process::Pid) -> Vec<rustix::process::Pid> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    use libproc::processes::{pids_by_type, ProcFilter};
    let group = group.as_raw_nonzero().get() as u32;
    pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group })
        .unwrap_or_default()
        .into_iter()
        .filter(|&pid| {
            pid != 0
                && pidinfo::<BSDInfo>(pid as i32, 0)
                    .is_ok_and(|info| info.pbi_pgid == group && info.pbi_status != libc::SZOMB)
        })
        .filter_map(|pid| rustix::process::Pid::from_raw(pid as i32))
        .collect()
}

/// The live members of `group`. A zombie is not live: it cannot fork, and its parent reaps it.
#[cfg(target_os = "linux")]
fn live_members(group: rustix::process::Pid) -> Vec<rustix::process::Pid> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<i32>().ok())
        .filter(|pid| {
            // The fields after the command name: state, ppid, pgrp.
            std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                let fields: Vec<&str> = stat
                    .rsplit_once(") ")
                    .map_or(Vec::new(), |(_, rest)| rest.split_whitespace().collect());
                fields.first() != Some(&"Z")
                    && fields.get(2).and_then(|pgrp| pgrp.parse::<i32>().ok())
                        == Some(group.as_raw_nonzero().get())
            })
        })
        .filter_map(rustix::process::Pid::from_raw)
        .collect()
}

/// A member that joins the group after the first kill still ends: a later round lists it, and the next kill of the group
/// reaches it (the macOS fork race of a single group kill). The rounds stop once a round lists no member.
#[test]
fn a_member_that_joins_after_the_first_kill_still_ends() {
    let mut rounds = std::collections::VecDeque::from([vec![1], vec![2], vec![]]);
    let kills = std::cell::Cell::new(0);
    let mut awaited = Vec::new();
    end_members(
        || kills.set(kills.get() + 1),
        || rounds.pop_front().expect("no round after an empty one"),
        |pid: &i32| awaited.push((*pid, kills.get())),
        || false,
    );
    assert_eq!(
        awaited,
        [(1, 1), (2, 2)],
        "each listed member is awaited after a kill"
    );
    assert_eq!(
        kills.get(),
        3,
        "the member that joined is reached by a later kill"
    );
    assert!(rounds.is_empty());
}

/// The rounds stop at the deadline, even while a member is still listed.
#[test]
fn the_rounds_stop_at_the_deadline() {
    let mut checks = 0;
    let mut kills = 0;
    end_members(
        || kills += 1,
        || vec![1],
        |_: &i32| {},
        || {
            checks += 1;
            checks > 2
        },
    );
    assert_eq!(kills, 3);
}

/// A shell cannot start its body until the anchor holds its group.
#[test]
fn register_worker() {
    let Some(socket) = std::env::var_os("BOTSTER_TEST_GROUP_SOCKET") else {
        return;
    };
    let registered = (|| -> std::io::Result<()> {
        let mut stream = UnixStream::connect(socket)?;
        writeln!(
            stream,
            "{}",
            std::env::var("BOTSTER_TEST_GROUP_PID").unwrap()
        )?;
        let mut ready = [0];
        stream.read_exact(&mut ready)?;
        if ready != [1] {
            return Err(std::io::Error::other("the group has no anchor"));
        }
        Ok(())
    })();
    if registered.is_err() {
        std::process::exit(1);
    }
}

struct Parent(Option<Child>);

impl Drop for Parent {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn eof(reader: impl Read + Send + 'static) {
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut rest = Vec::new();
        let result = reader.read_to_end(&mut rest);
        let _ = sent.send(result);
    });
    received
        // timer: deadline — bounds cleanup when a guard fails
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("all descendants closed the pipe")
        .unwrap();
}

/// The parent dies while the worker has no FIFO reader.
#[test]
fn parent_dies_before_fifo_reader() {
    for handoff in ["ready", "launch"] {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("fifo");
        assert!(Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        let mut parent = Parent(Some(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &helper("blocked_parent"), "--nocapture"])
                .env("BOTSTER_TEST_PARENT_DIR", dir.path())
                .env("BOTSTER_TEST_HANDOFF", handoff)
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        ));
        let child = parent.0.as_mut().unwrap();
        let mut reader = BufReader::new(child.stderr.take().unwrap());
        let mut pid = String::new();
        reader.read_line(&mut pid).unwrap();
        assert!(
            pid.trim().parse::<u32>().is_ok(),
            "the worker reached its FIFO: {pid}"
        );
        child.kill().unwrap();
        // Drop reaps only the parent. The parent cannot run its group guard.
        drop(parent);
        eof(reader);
    }
}

#[test]
fn blocked_parent() {
    use std::os::unix::process::CommandExt;
    let Some(dir) = std::env::var_os("BOTSTER_TEST_PARENT_DIR") else {
        return;
    };
    let dir = Path::new(&dir);
    let guard = GroupGuard::new(dir);
    let write = if std::env::var("BOTSTER_TEST_HANDOFF").unwrap() == "launch" {
        format!(
            "/bin/echo launch | /usr/bin/tee {} >/dev/null",
            quoted(&dir.join("fifo"))
        )
    } else {
        format!("/bin/echo ready > {}", quoted(&dir.join("fifo")))
    };
    let mut worker = Command::new("/bin/sh")
        .args(["-c", &format!("{}echo $$ >&2; {write}", guard.prefix())])
        .process_group(0)
        .spawn()
        .unwrap();
    // The outer test kills this parent while the child holds the stderr pipe.
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    drop(guard);
    worker.wait().unwrap();
}

/// Panic cleanup starts before any readiness indication exists.
#[test]
fn a_panic_before_ready_ends_the_child() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let guard = GroupGuard::new(dir.path());
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            &format!("{}while :; do /bin/sleep 1; done", guard.prefix()),
        ])
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let pipe = child.stdout.take().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = guard;
        panic!("the test failed before ready");
    }));
    assert!(result.is_err());
    eof(pipe);
    assert!(!child.wait().unwrap().success());
}

/// An anchor keeps the group after another owner reaps the leader.
#[test]
fn an_early_exit_keeps_the_group_owned_until_cleanup() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let guard = GroupGuard::new(dir.path());
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "{}(while :; do /bin/sleep 1; done) & echo $!; exit",
                guard.prefix()
            ),
        ])
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let group = rustix::process::Pid::from_raw(child.id() as i32).unwrap();
    let mut pipe = BufReader::new(child.stdout.take().unwrap());
    let mut descendant = String::new();
    pipe.read_line(&mut descendant).unwrap();
    assert!(descendant.trim().parse::<u32>().is_ok());
    assert!(child.wait().unwrap().success());
    let anchor = rustix::process::Pid::from_raw(guard.anchor.id() as i32).unwrap();
    assert_eq!(rustix::process::getpgid(Some(anchor)).unwrap(), group);
    drop(guard);
    eof(pipe);
}
