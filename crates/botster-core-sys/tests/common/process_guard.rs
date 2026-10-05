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
        Self::with_cleanup(dir, CLEANUP)
    }

    /// A guard whose anchor ends the group within `cleanup` (the failure test gives it no time).
    pub fn with_cleanup(dir: &Path, cleanup: std::time::Duration) -> Self {
        let socket = dir.join("guard.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let (control, child_control) = UnixStream::pair().unwrap();
        let input: OwnedFd = child_control.try_clone().unwrap().into();
        let output: OwnedFd = child_control.into();
        let anchor = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &helper("anchor_process"), "--nocapture"])
            .env("BOTSTER_TEST_ANCHOR", "1")
            .env(
                "BOTSTER_TEST_ANCHOR_CLEANUP_MS",
                cleanup.as_millis().to_string(),
            )
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
        // The anchor ends with success only when no member of the group is left; otherwise it wrote why on its stderr,
        // which is this end of the control stream.
        let status = self.anchor.wait();
        if !matches!(&status, Ok(status) if status.success()) {
            let mut report = String::new();
            let _ = self.control.read_to_string(&mut report);
            let report = format!(
                "the group guard's cleanup failed ({status:?}): {}",
                report.trim_start_matches('\u{1}').trim()
            );
            if std::thread::panicking() {
                eprintln!("{report}");
            } else {
                panic!("{report}");
            }
        }
    }
}

#[path = "guard_cleanup.rs"]
pub(crate) mod cleanup;

use cleanup::{end_group, CLEANUP};

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
    let cleanup = std::env::var("BOTSTER_TEST_ANCHOR_CLEANUP_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(CLEANUP, std::time::Duration::from_millis);
    if let Err(report) = end_group(group, cleanup) {
        let _ = writeln!(std::io::stderr(), "{report}");
        std::process::exit(1);
    }
}

/// A cleanup that cannot finish fails the test through the real guard: with no time for its rounds, the anchor reports
/// the live member, and the guard's drop fails with that report. The member still ends, by the anchor's last kill.
#[test]
fn a_cleanup_that_cannot_finish_fails_through_the_guard() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let never = dir.path().join("never");
    assert!(Command::new("/usr/bin/mkfifo")
        .arg(&never)
        .status()
        .unwrap()
        .success());
    let guard = GroupGuard::with_cleanup(dir.path(), std::time::Duration::ZERO);
    // The member blocks without CPU on a FIFO that nothing opens for writing, and holds the pipe until it ends.
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "{}/bin/echo up; exec /bin/cat {} >/dev/null",
                guard.prefix(),
                quoted(&never)
            ),
        ])
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let pipe = child.stdout.take().unwrap();
    let (pipe, line) = first_line(pipe);
    assert_eq!(line, "up\n");
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
        .expect_err("the guard reports the cleanup failure");
    let report = failed.downcast_ref::<String>().expect("a report").clone();
    assert!(report.contains("members left"), "{report}");
    eof(pipe);
    child.wait().unwrap();
}

/// The first line of `reader`, read with the cleanup deadline.
pub(crate) fn first_line(
    reader: impl Read + Send + 'static,
) -> (BufReader<Box<dyn Read + Send>>, String) {
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(Box::new(reader) as Box<dyn Read + Send>);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| (reader, line));
        let _ = sent.send(result);
    });
    received
        // timer: deadline — bounds the wait for a member's first line.
        .recv_timeout(CLEANUP)
        .expect("the member wrote its first line")
        .unwrap()
}

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

pub(crate) fn eof(reader: impl Read + Send + 'static) {
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
