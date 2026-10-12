//! A slow-tier driver process, stopped in a fixed order: TERM, a grace period, KILL, reap, drain the pipes.
//!
//! The child leads its own process group, so one signal reaches its descendants. Its stdout and stderr are read by
//! threads from the start, so a chatty child never blocks on a full pipe. Waits are channel waits on the real event
//! (an exit, an EOF); the only timeout is the grace period and the drain deadline, both given by the caller.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// The result of [`Driver::shutdown`].
#[derive(Debug)]
pub struct Shutdown {
    /// How the child ended.
    pub status: ExitStatus,
    /// `true` when the grace period ended before the child exited and the group got KILL.
    pub killed: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// `true` when a pipe still had a writer after the group died (a descendant that left the group, for example by `setsid`).
    /// The output is then what arrived before the drain deadline.
    pub pipes_held_open: bool,
}

/// A child that leads its own process group, with its output drained by threads.
pub struct Driver {
    child: Option<Child>,
    group: u32,
    out: Option<Drain>,
    err: Option<Drain>,
    finished: bool,
}

struct Drain {
    thread: JoinHandle<()>,
    done: mpsc::Receiver<()>,
    data: Arc<(Mutex<Vec<u8>>, Condvar)>,
}

fn drain(mut pipe: impl Read + Send + 'static) -> Drain {
    let data = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
    let sink = data.clone();
    let (tx, done) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = pipe.read(&mut buf) {
            if n == 0 {
                break;
            }
            sink.0
                .lock()
                .expect("sink lock")
                .extend_from_slice(&buf[..n]);
            sink.1.notify_all();
        }
        let _ = tx.send(());
    });
    Drain { thread, done, data }
}

impl Drain {
    /// Waits for EOF up to `deadline`. Returns the bytes and whether EOF came.
    fn finish(self, deadline: Duration) -> (Vec<u8>, bool) {
        let eof = self.done.recv_timeout(deadline).is_ok();
        if eof {
            let _ = self.thread.join();
        }
        // On a missed EOF the thread keeps its pipe end; it ends when the last writer dies. It is not joined.
        let bytes = self.data.0.lock().expect("sink lock").clone();
        (bytes, eof)
    }
}

/// Sends `signal` to a process group. The `kill` command is used because the workspace forbids `unsafe`.
fn signal_group(signal: &str, group: u32) {
    let _ = Command::new("kill")
        .args(["-s", signal, "--", &format!("-{group}")])
        .stderr(Stdio::null())
        .status();
}

impl Driver {
    /// Starts `command` in a new process group with piped stdout and stderr.
    pub fn spawn(mut command: Command) -> std::io::Result<Driver> {
        let mut child = command
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let group = child.id();
        let out = child.stdout.take().map(drain);
        let err = child.stderr.take().map(drain);
        Ok(Driver {
            child: Some(child),
            group,
            out,
            err,
            finished: false,
        })
    }

    /// Blocks until the child has written `needle` to stdout (the event that says it is ready), or `deadline` passes.
    /// Returns whether the bytes arrived.
    pub fn wait_for_stdout(&self, needle: &[u8], deadline: Duration) -> bool {
        let Some(out) = &self.out else { return false };
        let contains = |d: &Vec<u8>| d.windows(needle.len()).any(|w| w == needle);
        let guard = out.data.0.lock().expect("sink lock");
        let (guard, _) = out
            .data
            .1
            .wait_timeout_while(guard, deadline, |d| !contains(d))
            .expect("sink lock");
        contains(&guard)
    }

    /// The pid of the child, which is also its process group id.
    pub fn pid(&self) -> u32 {
        self.group
    }

    /// TERM to the group; wait `grace` for the child to exit; KILL to the group if it did not; reap the child;
    /// read both pipes to EOF, for at most `drain_deadline` each.
    pub fn shutdown(
        mut self,
        grace: Duration,
        drain_deadline: Duration,
    ) -> std::io::Result<Shutdown> {
        signal_group("TERM", self.group);
        let (status, killed) = self.wait_with_grace(grace)?;
        // KILL after the leader exits too: a descendant may ignore TERM and hold the pipes.
        signal_group("KILL", self.group);
        self.finished = true;
        let (stdout, out_eof) = self
            .out
            .take()
            .map_or((Vec::new(), true), |d| d.finish(drain_deadline));
        let (stderr, err_eof) = self
            .err
            .take()
            .map_or((Vec::new(), true), |d| d.finish(drain_deadline));
        Ok(Shutdown {
            status,
            killed,
            stdout,
            stderr,
            pipes_held_open: !(out_eof && err_eof),
        })
    }

    /// Waits for the exit up to `grace`; on a miss sends KILL to the group, then reaps.
    fn wait_with_grace(&mut self, grace: Duration) -> std::io::Result<(ExitStatus, bool)> {
        // `Child::wait` has no timeout, so a helper thread waits and signals the exit. The child goes into the
        // thread and comes back, so the reap happens on every path.
        let mut child = self.child.take().expect("shutdown runs once");
        let (tx, rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let status = child.wait();
            let _ = tx.send(());
            (child, status)
        });
        let killed = rx.recv_timeout(grace).is_err();
        if killed {
            signal_group("KILL", self.group);
        }
        let (child, status) = waiter.join().expect("the wait thread does not panic");
        self.child = Some(child);
        Ok((status?, killed))
    }
}

impl Drop for Driver {
    /// A driver that is dropped without `shutdown` (a panic) still ends its group and reaps the child.
    fn drop(&mut self) {
        if !self.finished {
            signal_group("KILL", self.group);
            if let Some(child) = self.child.as_mut() {
                let _ = child.wait();
            }
        }
    }
}
