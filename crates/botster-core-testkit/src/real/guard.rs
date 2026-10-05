//! The test's side of `botster-test-anchor` (protocol in [`crate::anchor`]): a listening socket that every anchor connects to,
//! and the launch wrappers that point at it.
//!
//! - **Ownership.** The guard lives in the test process. Each anchor blocks on its connection, so the end of the connection is
//!   the cleanup order. The guard's drop ends every connection; the death of the test ends them too, because the kernel closes
//!   the test's descriptors. A connection that the guard never accepted is reset when the listener closes, and the anchor
//!   takes the reset as the same order.
//! - **Registration.** The guard accepts its connections without blocking whenever it is asked for the anchors, and reads the
//!   report line, which each anchor wrote before its wrapper exec'd the real binary.
//! - **Cleanup.** [`AnchorGuard::finish`] half-closes every connection and reads each one to its end. An anchor's end comes only
//!   with its death, after its group `KILL`, or after its refusal. So when `finish` returns, no anchor of the guard is waiting.
//!
//! - **Bounded waits.** Every read has a deadline: [`SETTLE`] for a report, the longest grace plus [`SETTLE`] for an anchor's
//!   end. An anchor that misses it is stuck, and the call fails with `TimedOut` instead of hanging the run (BUILD.md testing
//!   rule 5; audit A11).
//!
//! Processes are reached only through the identities that the anchors report. Nothing is found by name or pattern.

use crate::anchor::{Config, Line, Report, CONFIG_FILE};
use botster_test_support::tempdir::TempRoot;
use std::io::{self, BufRead, BufReader};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long an anchor may take to write a line beyond the grace that it waits.
// timer: deadline — the limit of a wait for a real process's line; not a contract value.
pub const SETTLE: Duration = Duration::from_secs(10);

/// One accepted anchor connection and the lines read from it.
#[derive(Debug)]
struct Connection {
    reader: BufReader<UnixStream>,
    lines: Vec<Line>,
}

impl Connection {
    /// Reads one line within `deadline`. `Ok(false)` at the end of the connection: the anchor's death, or a reset.
    ///
    /// # Errors
    /// `TimedOut` when no line and no end came within the deadline.
    fn read_line(&mut self, deadline: Duration) -> io::Result<bool> {
        self.reader.get_ref().set_read_timeout(Some(deadline))?;
        let mut text = String::new();
        match self.reader.read_line(&mut text) {
            Ok(0) => Ok(false),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("an anchor wrote nothing within {deadline:?} (lines so far: {:?})", self.lines),
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => self.read_line(deadline),
            // A reset is the end of the connection, as an end of file is.
            Err(_) => Ok(false),
            Ok(_) => {
                self.lines.push(Line::decode(text.trim_end()).unwrap_or_else(|why| {
                    Line::Error {
                        stage: "guard".into(),
                        error: format!("an unreadable line ({why}): {text}"),
                    }
                }));
                Ok(true)
            }
        }
    }

    fn report(&self) -> Option<Report> {
        self.lines.iter().find_map(|line| match line {
            Line::Anchor(report) => Some(report.clone()),
            _ => None,
        })
    }
}

/// What one connection carried, read to its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    /// The group that the anchor held, or `None` when its wrapper failed before the anchor reported.
    pub report: Option<Report>,
    /// Every line, in order.
    pub lines: Vec<Line>,
}

impl Finished {
    /// True when the anchor sent its group `KILL`.
    pub fn killed(&self) -> bool {
        self.lines.last() == Some(&Line::Kill)
    }
}

/// The guard of the anchors of one harness.
#[derive(Debug)]
pub struct AnchorGuard {
    connections: Vec<Connection>,
    listener: Option<UnixListener>,
    socket: PathBuf,
    anchor_binary: PathBuf,
    wrappers: usize,
    /// The longest grace of the guard's wrappers.
    grace: Duration,
    /// Last field: it is removed after the connections are finished.
    root: TempRoot,
}

impl AnchorGuard {
    /// A guard whose wrappers link to `anchor_binary`, the verified prebuilt `botster-test-anchor`.
    pub fn new(anchor_binary: &Path) -> io::Result<AnchorGuard> {
        let root = TempRoot::new()?;
        let socket = root.socket_path("guard")?;
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        Ok(AnchorGuard {
            connections: Vec::new(),
            listener: Some(listener),
            socket,
            anchor_binary: anchor_binary.to_path_buf(),
            wrappers: 0,
            grace: Duration::ZERO,
            root,
        })
    }

    /// The temporary root of the guard: short and canonical, so a socket path under it fits (plan section 5).
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// A new wrapper named `file_name` that execs `binary` and gives its group `grace` between `TERM` and `KILL`.
    pub fn wrapper(&mut self, file_name: &str, binary: &Path, grace: Duration) -> io::Result<PathBuf> {
        self.wrappers += 1;
        self.grace = self.grace.max(grace);
        let dir = self.root.path().join(format!("w{}", self.wrappers));
        std::fs::create_dir(&dir)?;
        let config = Config {
            socket: self.socket.clone(),
            grace,
            binary: binary.to_path_buf(),
        };
        std::fs::write(dir.join(CONFIG_FILE), config.encode().map_err(io::Error::other)?)?;
        let path = dir.join(file_name);
        std::os::unix::fs::symlink(&self.anchor_binary, &path)?;
        Ok(path)
    }

    /// Accepts the connections that are waiting and reads the report of each new one.
    fn accept(&mut self) -> io::Result<()> {
        let Some(listener) = &self.listener else {
            return Ok(());
        };
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    // On macOS an accepted socket inherits the listener's nonblocking flag.
                    stream.set_nonblocking(false)?;
                    let mut connection = Connection {
                        reader: BufReader::new(stream),
                        lines: Vec::new(),
                    };
                    // The anchor wrote its report before the real binary started; a wrapper that failed wrote its error.
                    let read = connection.read_line(SETTLE);
                    self.connections.push(connection);
                    read?;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    /// The groups that the anchors hold, in the order of their connections. A real binary whose wrapper has exec'd always
    /// has its anchor here: the wrapper connected and the anchor reported before the exec.
    pub fn reports(&mut self) -> io::Result<Vec<Report>> {
        self.accept()?;
        Ok(self.connections.iter().filter_map(Connection::report).collect())
    }

    /// Ends every connection and waits for each anchor's end: its group `KILL`, its refusal or its failure. A wrapper that
    /// connects after this is refused, and so never execs.
    ///
    /// # Errors
    /// The first failure: an accept that failed, or an anchor that did not end within its grace and [`SETTLE`]. Every
    /// connection is ended and read even then.
    pub fn finish(&mut self) -> io::Result<Vec<Finished>> {
        let mut result = self.accept();
        self.listener = None;
        let _ = std::fs::remove_file(&self.socket);
        for connection in &self.connections {
            let _ = connection.reader.get_ref().shutdown(Shutdown::Write);
        }
        let deadline = self.grace + SETTLE;
        let mut finished = Vec::new();
        for mut connection in self.connections.drain(..) {
            loop {
                match connection.read_line(deadline) {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(error) => {
                        result = result.and(Err(error));
                        break;
                    }
                }
            }
            finished.push(Finished {
                report: connection.report(),
                lines: connection.lines,
            });
        }
        result.map(|()| finished)
    }
}

impl Drop for AnchorGuard {
    fn drop(&mut self) {
        // A test reads the result through `finish`; the drop only makes sure that no anchor outlives the guard.
        let _ = self.finish();
    }
}
