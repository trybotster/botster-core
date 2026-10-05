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

/// Ends every live member of `group` but this process, which stays a member until the end so that the group id cannot be
/// reused while it signals. One kill of the group is not enough: on macOS a child whose fork completes after the kill
/// escapes it and stays in the group. So each round kills the live members that it finds, until a round finds none.
pub(crate) fn end_group(group: rustix::process::Pid) {
    // timer: deadline — bounds the anchor's cleanup; a member that never ends cannot hold the anchor forever.
    let deadline = std::time::Instant::now() + CLEANUP;
    end_members(
        || live_members(group),
        |member| member.kill(group),
        |member| member.await_end(deadline),
        || std::time::Instant::now() >= deadline,
    );
}

/// The decision of `end_group`: each round kills the members that `members` lists and waits for their ends, and a round
/// that lists none is final, because only a live member can fork a new one. A member that joins after a kill is in a later
/// round's list. It stops at `expired`, so it never runs without end.
fn end_members<M>(
    mut members: impl FnMut() -> Vec<M>,
    mut kill: impl FnMut(&M),
    mut await_end: impl FnMut(&M),
    mut expired: impl FnMut() -> bool,
) {
    loop {
        let live = members();
        if live.is_empty() || expired() {
            return;
        }
        for member in &live {
            kill(member);
        }
        for member in &live {
            await_end(member);
        }
    }
}

/// A live member of the group, held by a pidfd: the kill and the wait reach this process and no other, even if it ends and
/// its pid is reused.
#[cfg(target_os = "linux")]
struct Member {
    pidfd: std::os::fd::OwnedFd,
}

#[cfg(target_os = "linux")]
impl Member {
    fn kill(&self, _group: rustix::process::Pid) {
        let _ = rustix::process::pidfd_send_signal(&self.pidfd, rustix::process::Signal::KILL);
    }

    /// Waits for this member's exit event (it may stay a zombie), at most until `deadline`.
    fn await_end(&self, deadline: std::time::Instant) {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        // timer: deadline — bounds the wait for a killed member's exit event.
        let limit = rustix::event::Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: left.subsec_nanos().into(),
        };
        let mut fds = [rustix::event::PollFd::new(
            &self.pidfd,
            rustix::event::PollFlags::IN,
        )];
        let _ = rustix::event::poll(&mut fds, Some(&limit));
    }
}

/// Whether the process of `pidfd` has ended: its pidfd is readable then.
#[cfg(target_os = "linux")]
fn ended(pidfd: &std::os::fd::OwnedFd) -> bool {
    let mut fds = [rustix::event::PollFd::new(
        pidfd,
        rustix::event::PollFlags::IN,
    )];
    let now = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    rustix::event::poll(&mut fds, Some(&now)).map_or(true, |ready| ready > 0)
}

/// The state and process group of `pid` from `/proc` (the fields after the command name: state, ppid, pgrp).
#[cfg(target_os = "linux")]
fn state_and_group(pid: u32) -> Option<(String, i32)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let mut fields = stat.rsplit_once(") ")?.1.split_whitespace();
    let state = fields.next()?.to_string();
    let group = fields.nth(1)?.parse().ok()?;
    Some((state, group))
}

/// The live members of `group` other than this process. A zombie is not live: it cannot fork, and its parent reaps it.
/// Each is held by a pidfd that is checked after it is opened: the pid still names a member of the group while the pidfd's
/// process lives, so the pidfd holds that member and not a later process with a reused pid.
#[cfg(target_os = "linux")]
fn live_members(group: rustix::process::Pid) -> Vec<Member> {
    let me = std::process::id();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let is_member = |pid: u32| {
        state_and_group(pid)
            .is_some_and(|(state, pgrp)| state != "Z" && pgrp == group.as_raw_nonzero().get())
    };
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|&pid| pid != me && is_member(pid))
        .filter_map(|pid| {
            let process = rustix::process::Pid::from_raw(pid as i32)?;
            let pidfd =
                rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty()).ok()?;
            (is_member(pid) && !ended(&pidfd)).then_some(Member { pidfd })
        })
        .collect()
}

/// A live member of the group, named by its pid and its start time.
#[cfg(target_os = "macos")]
struct Member {
    pid: rustix::process::Pid,
    start: (u64, u64),
}

/// The process group, start time and liveness of `pid`, or `None` when no process has it.
#[cfg(target_os = "macos")]
fn identity(pid: u32) -> Option<(u32, (u64, u64), bool)> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    let info = pidinfo::<BSDInfo>(pid as i32, 0).ok()?;
    Some((
        info.pbi_pgid,
        (info.pbi_start_tvsec, info.pbi_start_tvusec),
        info.pbi_status != libc::SZOMB,
    ))
}

#[cfg(target_os = "macos")]
impl Member {
    /// macOS has no pidfd. The kill checks just before it that the pid still names this member (its start time and the
    /// group). The window left is between that check and the signal: the member would have to end, be reaped by its
    /// parent and have its pid reused within it.
    fn kill(&self, group: rustix::process::Pid) {
        let pid = self.pid.as_raw_nonzero().get() as u32;
        let same = identity(pid).is_some_and(|(pgid, start, live)| {
            live && start == self.start && pgid == group.as_raw_nonzero().get() as u32
        });
        if same {
            let _ = rustix::process::kill_process(self.pid, rustix::process::Signal::KILL);
        }
    }

    /// Waits for this member's exit event (it may stay a zombie), at most until `deadline`. A process that is already gone
    /// has ended.
    fn await_end(&self, deadline: std::time::Instant) {
        let Ok(mut watcher) = kqueue::Watcher::new() else {
            return;
        };
        let added = watcher.add_pid(
            self.pid.as_raw_nonzero().get(),
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
}

/// The live members of `group` other than this process. A zombie is not live: it cannot fork, and its parent reaps it.
#[cfg(target_os = "macos")]
fn live_members(group: rustix::process::Pid) -> Vec<Member> {
    use libproc::processes::{pids_by_type, ProcFilter};
    let me = std::process::id();
    let group = group.as_raw_nonzero().get() as u32;
    pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group })
        .unwrap_or_default()
        .into_iter()
        .filter(|&pid| pid != 0 && pid != me)
        .filter_map(|pid| {
            let (pgid, start, live) = identity(pid)?;
            (live && pgid == group).then_some(Member {
                pid: rustix::process::Pid::from_raw(pid as i32)?,
                start,
            })
        })
        .collect()
}

/// A member that joins the group after the first kill still ends: a later round lists it and kills it (the macOS fork
/// race of a single group kill). The rounds stop once a round lists no member.
#[test]
fn a_member_that_joins_after_the_first_kill_still_ends() {
    let mut rounds = std::collections::VecDeque::from([vec![1], vec![2], vec![]]);
    let mut killed = Vec::new();
    let mut awaited = Vec::new();
    end_members(
        || rounds.pop_front().expect("no round after an empty one"),
        |pid: &i32| killed.push(*pid),
        |pid: &i32| awaited.push(*pid),
        || false,
    );
    assert_eq!(killed, [1, 2]);
    assert_eq!(awaited, killed, "each kill waits for that member's end");
    assert!(rounds.is_empty());
}

/// The rounds stop at the deadline, even while a member is still listed.
#[test]
fn the_rounds_stop_at_the_deadline() {
    let mut checks = 0;
    let mut killed = 0;
    end_members(
        || vec![1],
        |_: &i32| killed += 1,
        |_: &i32| {},
        || {
            checks += 1;
            checks > 2
        },
    );
    assert_eq!(killed, 2);
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
