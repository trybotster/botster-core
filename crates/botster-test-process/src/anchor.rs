//! The group guard: ONE anchor design for every real process that production starts and reaps (lead rulings 2026-10-04 and
//! 2026-10-08, the RealCoreHarness anchor with amended item 10).
//!
//! The test cannot hold such a process unreaped, so it cannot hold the process's group id either. So production starts the
//! process through a generated wrapper script, which runs `botster-test-anchor wrap`:
//!
//! 1. **wrap** connects to the guard's socket, starts the intermediate stage, reaps it (its only child, owned: on every error
//!    path it is killed and reaped within the cleanup bound), waits for the
//!    anchor's acknowledgement, and execs the real program with the same arguments. Exec keeps the pid, the start time, the
//!    group, the session, the environment and the exit path that production watches. The real program inherits no child of
//!    the wrapper.
//! 2. **intermediate** starts the anchor and exits at once, so the anchor is no child of the real program (the double fork).
//! 3. **anchor** holds the inherited group (and session) and the guard connection. It reports its identity and the leader's,
//!    then acknowledges to wrap. An acknowledgement with no reader (production ended wrap before its exec) is not an error:
//!    the anchor still holds the group. Then it blocks until the connection ends: the guard's release or drop, or the death
//!    of the test. Then it verifies the leader's identity and group, refusing to signal a group that the leader left (ruling
//!    item 13) or whose identity or group it cannot read; sends `TERM` to the group and waits the configured grace for its
//!    members to end; verifies again, and refuses when a member that `TERM` reached moved to another group; and ends the
//!    group in the rounds of `rounds.rs`, holding it with a reserve (see there why). It reaps only the reserve. It reports
//!    `ok` or the failure on the connection.
//!
//! The intermediate and the anchor first close every inherited descriptor above 2, so that no production pipe stays open in
//! them (`close_inherited` in the binary).
//!
//! Production keeps every reaping of its own children (ruling item 11). The guard runs in the test process, next to
//! production code, and it never waits for any process: it reads reports, and it awaits a group's end only by observing.

use crate::fixture::quoted;
use crate::platform::{pid, start_time};
use crate::read::Bounded;
use crate::{fail, Deadline, CLEANUP};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The name of the anchor binary, which `cargo xtask prebuild-worker` builds into `target/candidate/`.
pub const BINARY: &str = "botster-test-anchor";

/// The exit code of a wrapper that could not reach the exec of its program.
pub const WRAP_FAILED: i32 = 125;

/// A process by pid and start time (only equality of start times has a meaning).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub start_time: u64,
}

/// What an anchor reports when it holds its group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub anchor: Identity,
    pub group: u32,
    /// The wrapper, which execs the real program: the real program's pid and start time.
    pub leader: Identity,
}

/// One line on a guard connection, from the wrapper or the anchor to the guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// The anchor holds the group.
    Anchor(Report),
    /// The cleanup ended the group.
    Ok,
    /// The cleanup did not end the group.
    Fail(String),
    /// The cleanup refused to signal the group.
    Refused(String),
    /// A stage failed before the anchor held the group.
    Error { stage: String, reason: String },
}

impl Line {
    pub fn encode(&self) -> String {
        let one = |text: &str| text.replace('\n', " ");
        match self {
            Line::Anchor(r) => format!(
                "anchor {} {} {} {} {}",
                r.anchor.pid, r.anchor.start_time, r.group, r.leader.pid, r.leader.start_time
            ),
            Line::Ok => "ok".into(),
            Line::Fail(reason) => format!("fail {}", one(reason)),
            Line::Refused(reason) => format!("refused {}", one(reason)),
            Line::Error { stage, reason } => format!("error {stage} {}", one(reason)),
        }
    }

    pub fn decode(line: &str) -> Option<Line> {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        match word {
            "anchor" => {
                let n: Vec<u64> = rest
                    .split(' ')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .ok()?;
                let [anchor, anchor_start, group, leader, leader_start] = n[..] else {
                    return None;
                };
                Some(Line::Anchor(Report {
                    anchor: Identity {
                        pid: u32::try_from(anchor).ok()?,
                        start_time: anchor_start,
                    },
                    group: u32::try_from(group).ok()?,
                    leader: Identity {
                        pid: u32::try_from(leader).ok()?,
                        start_time: leader_start,
                    },
                }))
            }
            "ok" if rest.is_empty() => Some(Line::Ok),
            "fail" => Some(Line::Fail(rest.into())),
            "refused" => Some(Line::Refused(rest.into())),
            "error" => {
                let (stage, reason) = rest.split_once(' ')?;
                Some(Line::Error {
                    stage: stage.into(),
                    reason: reason.into(),
                })
            }
            _ => None,
        }
    }
}

/// `target/candidate/botster-test-anchor` for the test binary `exe` (`target/<profile>/deps/<test>`), so it holds for any
/// target directory.
pub fn candidate(exe: &Path) -> Option<PathBuf> {
    let target = exe.parent()?.parent()?.parent()?;
    Some(target.join("candidate").join(BINARY))
}

/// The prebuilt anchor binary of this test binary (`candidate`).
///
/// # Panics
/// The binary is missing: `cargo xtask prebuild-worker` builds it.
pub fn binary() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    let binary = candidate(&exe).expect("target/<profile>/deps");
    assert!(
        binary.is_file(),
        "{} is missing: run `cargo xtask prebuild-worker` first",
        binary.display()
    );
    binary
}

/// One wrapper's connection, as the guard sees it.
struct Connection {
    /// The writing side, for the cleanup request (a shutdown).
    stream: UnixStream,
    lines: Bounded<UnixStream>,
    report: Option<Report>,
    /// The final line, or the reason why the connection failed.
    end: Option<Result<Line, String>>,
}

impl Connection {
    /// Reads one line (the caller saw it readable, or waits by `deadline`) and records it.
    fn read(&mut self, deadline: Deadline) -> Result<(), String> {
        match self.lines.line(deadline) {
            Ok(Some(text)) => match Line::decode(&text) {
                Some(Line::Anchor(report)) if self.report.is_none() => {
                    self.report = Some(report);
                }
                Some(line @ Line::Error { .. }) => self.end = Some(Ok(line)),
                Some(line) if self.report.is_some() => self.end = Some(Ok(line)),
                _ => self.end = Some(Err(format!("an unexpected line {text:?}"))),
            },
            Ok(None) => self.end = Some(Err(String::new())),
            Err(error) => return Err(error.to_string()),
        }
        Ok(())
    }
}

/// The test's side of the anchors. See the module documentation.
pub struct Guard {
    listener: Option<UnixListener>,
    socket: PathBuf,
    binary: PathBuf,
    grace: Duration,
    cleanup: Duration,
    connections: Vec<Connection>,
}

impl Guard {
    /// A guard with its socket in `dir`, the prebuilt anchor binary and no grace.
    ///
    /// # Errors
    /// The socket could not be bound.
    pub fn new(dir: &Path) -> io::Result<Self> {
        Self::with_binary(dir, binary())
    }

    /// A guard with its socket in `dir` and the anchor binary `binary`.
    ///
    /// # Errors
    /// The socket could not be bound.
    pub fn with_binary(dir: &Path, binary: PathBuf) -> io::Result<Self> {
        let socket = dir.join("anchor.sock");
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener: Some(listener),
            socket,
            binary,
            grace: Duration::ZERO,
            cleanup: CLEANUP,
            connections: Vec::new(),
        })
    }

    /// The time that an anchor gives the group's members after its `TERM`, before the rounds of `KILL`: the existing
    /// configured stop grace of the subject (ruling item 9). Zero sends no `TERM`.
    #[must_use]
    pub fn grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    /// The bound of an anchor's rounds: the existing cleanup bound unless a test of a failed cleanup gives it no time (lead
    /// ruling 2026-10-08, condition 1b).
    #[must_use]
    pub fn cleanup(mut self, cleanup: Duration) -> Self {
        self.cleanup = cleanup;
        self
    }

    /// Writes the executable wrapper script `path`: it execs the anchor's wrap stage, which execs `program` with `args` and
    /// then the arguments that the script receives.
    ///
    /// # Errors
    /// The script could not be written.
    pub fn wrapper(&self, path: &Path, program: &Path, args: &[&str]) -> io::Result<()> {
        let mut words = vec![
            "exec".to_string(),
            quoted(&self.binary),
            "wrap".into(),
            quoted(&self.socket),
            self.grace.as_nanos().to_string(),
            self.cleanup.as_nanos().to_string(),
            quoted(program),
        ];
        words.extend(args.iter().map(|arg| quoted(Path::new(arg))));
        words.push("\"$@\"".into());
        std::fs::write(path, format!("#!/bin/sh\n{}\n", words.join(" ")))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
    }

    /// Accepts every connection that is waiting.
    fn accept(&mut self) -> Result<(), String> {
        let Some(listener) = &self.listener else {
            return Ok(());
        };
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let lines = stream
                        .try_clone()
                        .map_err(|error| format!("a connection cannot be read: {error}"))?;
                    self.connections.push(Connection {
                        stream,
                        lines: Bounded::new(lines),
                        report: None,
                        end: None,
                    });
                }
                // The listener is non-blocking: no connection waits.
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(format!("the guard cannot accept: {error}")),
            }
        }
    }

    /// The reports of the first `count` anchors, once they hold their groups, waited for by `deadline`.
    ///
    /// # Errors
    /// A wrapper or an anchor failed, or fewer than `count` anchors reported by the deadline.
    pub fn anchors(&mut self, count: usize, deadline: Deadline) -> Result<Vec<Report>, String> {
        loop {
            self.accept()?;
            if let Some(failure) = self.connections.iter().find_map(|c| match &c.end {
                Some(Ok(Line::Error { stage, reason })) => {
                    Some(format!("the {stage} stage failed: {reason}"))
                }
                Some(Err(why)) if c.report.is_none() && !why.is_empty() => Some(why.clone()),
                _ => None,
            }) {
                return Err(failure);
            }
            let reports: Vec<Report> = self.connections.iter().filter_map(|c| c.report).collect();
            if reports.len() >= count {
                return Ok(reports);
            }
            let late = format!(
                "{} of {count} anchors reported within {:?}",
                reports.len(),
                deadline.limit()
            );
            // No wait starts after the deadline, so the loop ends at it even when a read makes no progress.
            if deadline.expired() {
                return Err(late);
            }
            let waiting: Vec<usize> = (0..self.connections.len())
                .filter(|&i| self.connections[i].end.is_none())
                .collect();
            let ready = {
                let mut fds = Vec::new();
                if let Some(listener) = &self.listener {
                    fds.push(rustix::event::PollFd::new(
                        listener,
                        rustix::event::PollFlags::IN,
                    ));
                }
                for &i in &waiting {
                    fds.push(rustix::event::PollFd::new(
                        &self.connections[i].stream,
                        rustix::event::PollFlags::IN,
                    ));
                }
                // timer: deadline — bounds the wait for the anchors' reports.
                match rustix::event::poll(&mut fds, Some(&deadline.timespec())) {
                    Ok(0) => return Err(late),
                    Ok(_) => fds
                        .iter()
                        .map(|fd| !fd.revents().is_empty())
                        .collect::<Vec<bool>>(),
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(error) => return Err(format!("the guard cannot wait: {error}")),
                }
            };
            let skip = usize::from(self.listener.is_some());
            for (n, &i) in waiting.iter().enumerate() {
                if ready[skip + n] {
                    self.connections[i].read(deadline)?;
                }
            }
        }
    }

    /// Sends the cleanup request to every anchor, and returns at once: an owner whose production must clean up after the
    /// anchors started (macOS can hold a member's exit in a terminal drain until production closes the PTY master) releases
    /// first and drops the guard after production. No wrapper can connect after it.
    pub fn release(&mut self) {
        if let Err(why) = self.accept() {
            fail(why);
        }
        self.listener = None;
        let _ = std::fs::remove_file(&self.socket);
        for connection in &self.connections {
            let _ = connection.stream.shutdown(std::net::Shutdown::Write);
        }
    }

    /// The outcome of every anchor: `Ok` when each group was proved empty. An anchor that ended without a report was ended
    /// by production's own group kill; its group must then be empty within the cleanup bound, which is observed without a
    /// signal (the guard holds no reservation of that group).
    fn outcome(&mut self) -> Result<(), String> {
        // An anchor's rounds take its grace and the cleanup bound; its report takes at most the cleanup bound more.
        let deadline = Deadline::after(self.grace + self.cleanup + CLEANUP);
        let mut failures = Vec::new();
        for connection in &mut self.connections {
            while connection.end.is_none() {
                // No read starts after the deadline, so the loop ends at it even when a read makes no progress.
                if deadline.expired() {
                    connection.end = Some(Err(format!("no report within {:?}", deadline.limit())));
                } else if let Err(why) = connection.read(deadline) {
                    connection.end = Some(Err(format!("no report: {why}")));
                }
            }
            let failure = match (connection.report, connection.end.take()) {
                (_, Some(Ok(Line::Ok))) => None,
                (_, Some(Ok(Line::Fail(why)))) => Some(why),
                (_, Some(Ok(Line::Refused(why)))) => Some(format!("refused: {why}")),
                (_, Some(Ok(Line::Error { stage, reason }))) => {
                    Some(format!("the {stage} stage failed: {reason}"))
                }
                // A wrapper that ended before its anchor existed left nothing to clean up.
                (None, Some(Err(why))) if why.is_empty() => None,
                (Some(report), Some(Err(why))) if why.is_empty() => pid(report.group)
                    .map_err(|e| e.to_string())
                    .and_then(|group| crate::rounds::await_group_end(group, Deadline::cleanup()))
                    .err()
                    .map(|why| {
                        format!(
                            "the anchor of group {} ended without a report: {why}",
                            report.group
                        )
                    }),
                (_, Some(Err(why))) => Some(why),
                (_, Some(Ok(line))) => Some(format!("an unexpected line {line:?}")),
                (_, None) => Some("no outcome".into()),
            };
            failures.extend(failure);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "the group guard's cleanup failed: {}",
                failures.join("; ")
            ))
        }
    }
}

impl Drop for Guard {
    /// Releases every anchor, then reads each outcome, and fails the test unless every group was proved empty (or reports the
    /// failure when the test already panics).
    fn drop(&mut self) {
        self.release();
        if let Err(report) = self.outcome() {
            fail(report);
        }
    }
}

/// Writes one line to a guard connection.
///
/// # Errors
/// The write failed.
pub fn send(mut to: impl Write, line: &Line) -> io::Result<()> {
    writeln!(to, "{}", line.encode())?;
    to.flush()
}

/// The identity of a live process.
///
/// # Errors
/// The process has no start time (it is gone), or its start time could not be read.
pub fn identity(raw: u32) -> io::Result<Identity> {
    let start_time = start_time(pid(raw)?)?
        .ok_or_else(|| io::Error::other(format!("{raw} has no start time")))?;
    Ok(Identity {
        pid: raw,
        start_time,
    })
}

/// The verdict on a signal to the reported group (ruling item 13), from what the anchor observes now: its own group, the
/// leader's start time (`None`: the leader has ended), and the leader's group, which is read only while the leader lives.
/// A leader that has ended (its pid is gone, or names a process with another start time) leaves only the reserved group id
/// to signal. An observation that failed refuses: an identity or a membership that cannot be read is not verified (#171
/// TP2).
///
/// # Errors
/// The refusal: the anchor left the group, the leader moved to another group, or an observation failed.
pub fn verdict(
    report: &Report,
    own_group: u32,
    leader_start: io::Result<Option<u64>>,
    leader_group: impl FnOnce() -> rustix::io::Result<u32>,
) -> Result<(), String> {
    if own_group != report.group {
        return Err("the anchor left the group".into());
    }
    let leader = report.leader.pid;
    match leader_start {
        Err(error) => {
            return Err(format!(
                "the identity of the leader {leader} cannot be read: {error}"
            ))
        }
        Ok(start) if start != Some(report.leader.start_time) => return Ok(()),
        Ok(_) => {}
    }
    match leader_group() {
        Ok(group) if group != report.group => Err(format!(
            "the leader {leader} moved from group {} to group {group}",
            report.group
        )),
        Ok(_) => Ok(()),
        // ESRCH: the leader ended between the two reads.
        Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(error) => Err(format!(
            "the group of the leader {leader} cannot be read: {error}"
        )),
    }
}

/// The verdict on the members that were in `group` when the anchor sent `TERM`, after the grace and before any `KILL` (#171
/// TP6): each one is still in the group, or gone. A member that moved to another group refuses: a `KILL` of the old group
/// would leave it live. A member whose group cannot be read is not verified, and it refuses too. (A member that ended while
/// another process took its pid can refuse as well; a refusal signals nothing, so that is the safe side.)
///
/// # Errors
/// The refusal: the first member that moved, or whose group could not be read.
pub fn stayed(
    group: u32,
    members: &[u32],
    mut group_of: impl FnMut(u32) -> rustix::io::Result<u32>,
) -> Result<(), String> {
    for &member in members {
        match group_of(member) {
            Ok(now) if now != group => {
                return Err(format!(
                    "the member {member} moved from group {group} to group {now}"
                ))
            }
            Ok(_) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => {
                return Err(format!(
                    "the group of the member {member} cannot be read: {error}"
                ))
            }
        }
    }
    Ok(())
}

/// The wrapper's start of the anchor (#171 TP4). `intermediate` is the intermediate stage, which starts the anchor and
/// exits. It is an owned child: on every path it ends and is reaped, by its exact pid, within a bound (when this returns
/// early, the drop of [`crate::OwnedChild`] kills it and reaps it within the cleanup bound). Then the anchor's one line on the
/// intermediate's stdout, which the anchor writes only once it holds the group, proves the start.
///
/// # Errors
/// The intermediate did not end by `deadline` or failed, or the anchor ended or was silent before it held the group.
pub fn start_anchor(
    intermediate: &mut std::process::Command,
    deadline: Deadline,
) -> io::Result<()> {
    let mut stage = crate::OwnedChild::spawn(intermediate.stdout(std::process::Stdio::piped()))?;
    let acknowledgement = stage
        .take_stdout()
        .ok_or_else(|| io::Error::other("no pipe from the anchor"))?;
    match stage.exit_by(deadline)? {
        None => return Err(io::Error::other("the intermediate stage did not end")),
        Some(status) if !status.success() => {
            return Err(io::Error::other(format!(
                "the intermediate stage failed: {status}"
            )))
        }
        Some(_) => {}
    }
    match Bounded::new(acknowledgement).line(deadline) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(io::Error::other(
            "the anchor ended before it held the group",
        )),
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        Report {
            anchor: Identity {
                pid: 11,
                start_time: 12,
            },
            group: 13,
            leader: Identity {
                pid: 13,
                start_time: 14,
            },
        }
    }

    /// The verdict with the anchor in the reported group 13 and the leader 13 started at 14.
    fn verdict_of(
        leader_start: io::Result<Option<u64>>,
        leader_group: rustix::io::Result<u32>,
    ) -> Result<(), String> {
        verdict(&report(), 13, leader_start, || leader_group)
    }

    #[test]
    fn a_verified_leader_in_its_group_or_an_ended_leader_lets_the_anchor_signal() {
        assert_eq!(verdict_of(Ok(Some(14)), Ok(13)), Ok(()));
        assert_eq!(
            verdict_of(Ok(None), Err(rustix::io::Errno::PERM)),
            Ok(()),
            "gone"
        );
        assert_eq!(
            verdict_of(Ok(Some(99)), Err(rustix::io::Errno::PERM)),
            Ok(()),
            "pid reused"
        );
        assert_eq!(
            verdict_of(Ok(Some(14)), Err(rustix::io::Errno::SRCH)),
            Ok(()),
            "ended between the reads"
        );
    }

    #[test]
    fn a_moved_leader_or_an_anchor_out_of_the_group_is_refused() {
        assert_eq!(
            verdict_of(Ok(Some(14)), Ok(20)),
            Err("the leader 13 moved from group 13 to group 20".into())
        );
        assert_eq!(
            verdict(&report(), 20, Ok(Some(14)), || Ok(13)),
            Err("the anchor left the group".into())
        );
    }

    /// #171 TP2: an identity or a membership that cannot be read is not verified, so it refuses.
    #[test]
    fn an_identity_or_a_group_that_cannot_be_read_is_refused() {
        assert_eq!(
            verdict_of(Err(io::Error::other("refused")), Ok(13)),
            Err("the identity of the leader 13 cannot be read: refused".into())
        );
        assert_eq!(
            verdict_of(Ok(Some(14)), Err(rustix::io::Errno::PERM)),
            Err(format!(
                "the group of the leader 13 cannot be read: {}",
                rustix::io::Errno::PERM
            ))
        );
    }

    /// #171 TP6: after the grace, every member that `TERM` reached is still in the group or gone; a member that moved, or
    /// whose group cannot be read, refuses.
    #[test]
    fn a_member_that_moved_or_cannot_be_read_after_the_grace_is_refused() {
        let groups = |member: u32| match member {
            1 => Ok(13),
            2 => Err(rustix::io::Errno::SRCH),
            3 => Ok(30),
            _ => Err(rustix::io::Errno::PERM),
        };
        assert_eq!(stayed(13, &[], groups), Ok(()));
        assert_eq!(stayed(13, &[1, 2], groups), Ok(()));
        assert_eq!(
            stayed(13, &[1, 3, 4], groups),
            Err("the member 3 moved from group 13 to group 30".into())
        );
        assert_eq!(
            stayed(13, &[2, 4, 3], groups),
            Err(format!(
                "the group of the member 4 cannot be read: {}",
                rustix::io::Errno::PERM
            ))
        );
    }

    #[test]
    fn every_line_round_trips_and_a_reason_stays_on_one_line() {
        for line in [
            Line::Anchor(report()),
            Line::Ok,
            Line::Fail("members left after 10s: 7 (state S)".into()),
            Line::Refused("the leader moved".into()),
            Line::Error {
                stage: "wrap".into(),
                reason: "exec /x: not found".into(),
            },
        ] {
            let text = format!("{}\n", line.encode());
            assert_eq!(Line::decode(&text), Some(line));
        }
        assert_eq!(Line::Fail("a\nb".into()).encode(), "fail a b");
    }

    #[test]
    fn a_malformed_line_decodes_to_nothing() {
        for text in [
            "",
            "anchor 1 2 3 4",
            "anchor 1 2 3 4 5 6",
            "anchor 1 2 x 4 5",
            "anchor 99999999999 2 3 4 5",
            "ok now",
            "error wrap",
            "hello",
        ] {
            assert_eq!(Line::decode(text), None, "{text:?}");
        }
    }

    #[test]
    fn this_process_has_an_identity_and_pid_zero_has_none() {
        let me = identity(std::process::id()).unwrap();
        assert_eq!(me.pid, std::process::id());
        assert_eq!(identity(std::process::id()).unwrap(), me);
        assert!(identity(0).is_err());
    }

    #[test]
    fn a_wrapper_script_execs_the_anchor_with_the_socket_the_grace_and_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let guard = Guard::with_binary(dir.path(), PathBuf::from("/b/anchor"))
            .unwrap()
            .grace(Duration::from_millis(3))
            .cleanup(Duration::from_millis(4));
        let script = dir.path().join("w.sh");
        guard
            .wrapper(&script, Path::new("/bin/sh"), &["body it's.sh"])
            .unwrap();
        let text = std::fs::read_to_string(&script).unwrap();
        assert_eq!(
            text,
            format!(
                "#!/bin/sh\nexec '/b/anchor' wrap {} 3000000 4000000 '/bin/sh' 'body it'\\''s.sh' \"$@\"\n",
                quoted(&dir.path().join("anchor.sock"))
            )
        );
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn a_guard_that_no_wrapper_reached_drops_cleanly_and_reports_no_anchor_in_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut guard = Guard::with_binary(dir.path(), PathBuf::from("/b/anchor")).unwrap();
        assert_eq!(guard.anchors(0, Deadline::cleanup()), Ok(Vec::new()));
        assert_eq!(
            guard.anchors(1, Deadline::after(Duration::ZERO)),
            Err("0 of 1 anchors reported within 0ns".into())
        );
        guard.release();
        assert!(!dir.path().join("anchor.sock").exists());
        drop(guard);
    }

    /// The guard's side of the protocol over real connections, with the test playing the anchors: the reports, the cleanup
    /// request, and every outcome, failures included.
    #[test]
    fn the_guard_reads_reports_and_outcomes_and_fails_on_a_failed_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let mut guard = Guard::with_binary(dir.path(), PathBuf::from("/b/anchor")).unwrap();
        let socket = dir.path().join("anchor.sock");
        let mut ok = UnixStream::connect(&socket).unwrap();
        let mut failing = UnixStream::connect(&socket).unwrap();
        let early = UnixStream::connect(&socket).unwrap();
        send(&mut ok, &Line::Anchor(report())).unwrap();
        send(&mut failing, &Line::Anchor(report())).unwrap();
        drop(early);
        assert_eq!(
            guard.anchors(2, Deadline::cleanup()),
            Ok(vec![report(), report()])
        );
        guard.release();
        let request = Bounded::new(ok.try_clone().unwrap())
            .to_eof(Deadline::cleanup())
            .unwrap();
        assert!(request.is_empty(), "the cleanup request is the end of file");
        send(&mut ok, &Line::Ok).unwrap();
        send(
            &mut failing,
            &Line::Fail("members left after 10s: 7".into()),
        )
        .unwrap();
        drop((ok, failing));
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
            .expect_err("a failed cleanup fails the test");
        let message = failed.downcast_ref::<String>().expect("a report");
        assert_eq!(
            message,
            "the group guard's cleanup failed: members left after 10s: 7"
        );
    }

    #[test]
    fn a_wrapper_error_fails_the_wait_for_anchors_and_the_drop() {
        let dir = tempfile::tempdir().unwrap();
        let mut guard = Guard::with_binary(dir.path(), PathBuf::from("/b/anchor")).unwrap();
        let mut wrapper = UnixStream::connect(dir.path().join("anchor.sock")).unwrap();
        send(
            &mut wrapper,
            &Line::Error {
                stage: "wrap".into(),
                reason: "exec /x: not found".into(),
            },
        )
        .unwrap();
        drop(wrapper);
        assert_eq!(
            guard.anchors(1, Deadline::cleanup()),
            Err("the wrap stage failed: exec /x: not found".into())
        );
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
            .expect_err("a wrapper error fails the test");
        assert_eq!(
            failed.downcast_ref::<String>().map(String::as_str),
            Some("the group guard's cleanup failed: the wrap stage failed: exec /x: not found")
        );
    }

    /// The text of an anchor's report line, as an anchor sends it.
    fn anchor_line(group: u32) -> String {
        let mut report = report();
        report.group = group;
        format!("{}\n", Line::Anchor(report).encode())
    }

    /// A guard whose one connection sent `text` and closed.
    fn guard_after(text: &str) -> (tempfile::TempDir, Guard) {
        let dir = tempfile::tempdir().unwrap();
        let guard = Guard::with_binary(dir.path(), PathBuf::from("/b/anchor")).unwrap();
        let mut anchor = UnixStream::connect(dir.path().join("anchor.sock")).unwrap();
        anchor.write_all(text.as_bytes()).unwrap();
        drop(anchor);
        (dir, guard)
    }

    #[test]
    fn a_refusal_an_unexpected_line_or_a_second_report_fails_the_drop() {
        for (text, expected) in [
            (
                format!("{}refused moved\n", anchor_line(13)),
                "refused: moved",
            ),
            ("ok\n".to_string(), r#"an unexpected line "ok\n""#),
            (
                format!("{}hello\n", anchor_line(13)),
                r#"an unexpected line "hello\n""#,
            ),
            (
                format!("{}{}", anchor_line(13), anchor_line(14)),
                "an unexpected line Anchor(",
            ),
        ] {
            let (_dir, guard) = guard_after(&text);
            let message = drop_failure(guard);
            assert!(
                message.starts_with(&format!("the group guard's cleanup failed: {expected}")),
                "{text:?}: {message}"
            );
        }
    }

    /// An anchor that reported and then ended with no outcome was ended by production's group kill: the drop observes that
    /// its group is empty, and succeeds.
    #[test]
    fn an_anchor_that_ended_after_its_report_passes_when_its_group_is_empty() {
        // A group id that no process uses: above every pid that macOS gives, and unused on Linux.
        let (_dir, guard) = guard_after(&anchor_line(4_194_000));
        drop(guard);
    }

    /// A line other than a report before any report fails the wait for anchors at once.
    /// A report is ready, but the deadline passed: no wait or read starts after it.
    #[test]
    fn no_wait_for_anchors_starts_after_its_deadline() {
        let (_dir, mut guard) = guard_after(&format!("{}ok\n", anchor_line(13)));
        assert_eq!(
            guard.anchors(1, Deadline::after(Duration::ZERO)),
            Err("0 of 1 anchors reported within 0ns".into())
        );
        assert_eq!(
            guard.anchors(1, Deadline::cleanup()).map(|r| r.len()),
            Ok(1)
        );
    }

    #[test]
    fn an_unexpected_line_before_the_report_fails_the_wait_for_anchors() {
        let (_dir, mut guard) = guard_after("hello\n");
        assert_eq!(
            guard.anchors(1, Deadline::cleanup()),
            Err(r#"an unexpected line "hello\n""#.into())
        );
        assert_eq!(
            drop_failure(guard),
            r#"the group guard's cleanup failed: an unexpected line "hello\n""#
        );
    }

    fn drop_failure(guard: Guard) -> String {
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
            .expect_err("a failure");
        failed.downcast_ref::<String>().expect("a report").clone()
    }

    #[test]
    fn the_candidate_anchor_is_beside_the_profile_directory_of_the_test_binary() {
        assert_eq!(
            candidate(Path::new("/t/target/debug/deps/slow-1a2b")),
            Some(PathBuf::from("/t/target/candidate/botster-test-anchor"))
        );
        assert_eq!(candidate(Path::new("/deps/x")), None);
    }

    /// The prebuild may or may not have run: either way, the binary is the candidate of this test binary, or its absence
    /// fails with the command that builds it.
    #[test]
    fn the_binary_is_the_candidate_or_its_absence_names_the_prebuild() {
        let expected = candidate(&std::env::current_exe().unwrap()).unwrap();
        match std::panic::catch_unwind(binary) {
            Ok(path) => assert_eq!(path, expected),
            Err(panic) => {
                let message = panic.downcast_ref::<String>().cloned().unwrap_or_default();
                assert_eq!(
                    message,
                    format!(
                        "{} is missing: run `cargo xtask prebuild-worker` first",
                        expected.display()
                    )
                );
            }
        }
    }
}
