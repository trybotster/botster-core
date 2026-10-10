//! A stand-in at a worker's endpoint path: the real form of `impostor_worker`, `withhold_control_link` and
//! `announce_protocol` (steward ruling R-49 Q4, and its follow-up for `announce_protocol` on contracts `main` `a1a7ae9`).
//! The endpoint path is the boundary that Core uses to find the worker (Core E1-1), so a process-shaped peer that listens at
//! that path is "an endpoint that is not the session's worker" (A10-1). Core and the worker run unchanged.
//!
//! The R-49 conditions, as this module keeps them:
//!
//! 1. [`StandIn::install`] renames the real endpoint aside and binds the stand-in at its path. The harness calls it only
//!    while no Core holds the data directory: after the host closed and before it reopens.
//! 2. The drop of a [`StandIn`] renames the real endpoint back over the stand-in's path, also on a panic or a failed step.
//!    An install that cannot bind renames it back at once.
//! 3. The real worker keeps running: its listener is not touched, only the name of its socket file.
//!
//! The stand-in has no thread and never waits. The tap steps it ([`StandIn::step`]) after each call of Core that can reach it
//! (a connect, a send). A Unix stream delivers a write before the call returns, so one step reads all that Core sent.

use crate::worker::{impostor_answer, ImpostorPlan};
use botster_core_contract::prelude::SessionId;
use botster_core_link::frame::{FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use std::io::{self, Read as _, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// What the stand-in does with a host's connection.
pub(crate) enum Role {
    /// `impostor_worker` (Core A10-1, A11-1) and `announce_protocol` (Core A6-2, AD-4): it answers the host's first hello
    /// with the plan's hello and scripted frames, in one write (steward ruling R-42), and then sends nothing.
    Answer(ImpostorPlan),
    /// `withhold_control_link` (Core A6-1): it accepts the connection and never answers, so Core's own handshake deadline
    /// decides the outcome.
    Withhold,
}

/// A connection that the stand-in accepted.
struct PeerLink {
    /// `None` after the host closed the link: the stand-in then closed its end.
    stream: Option<UnixStream>,
    decoder: FrameDecoder,
    answered: bool,
}

/// A stand-in at the endpoint path of one session's worker, until it is dropped.
pub struct StandIn {
    session: SessionId,
    role: Role,
    /// The endpoint path, where the stand-in listens.
    path: PathBuf,
    /// Where the real endpoint waits until the drop.
    aside: PathBuf,
    listener: UnixListener,
    links: Vec<PeerLink>,
}

impl StandIn {
    /// Renames the real endpoint at `path` to `aside`, and binds the stand-in at `path`.
    ///
    /// # Errors
    /// The rename fails (for example, no endpoint is at `path`), or the bind fails. After a failed bind, the real endpoint is
    /// back at `path`.
    pub(crate) fn install(
        session: SessionId,
        role: Role,
        path: &Path,
        aside: &Path,
    ) -> io::Result<StandIn> {
        std::fs::rename(path, aside)?;
        let listener = match UnixListener::bind(path).and_then(|l| {
            l.set_nonblocking(true)?;
            Ok(l)
        }) {
            Ok(listener) => listener,
            Err(error) => {
                // A bind that failed may have left a socket file at `path`; the rename replaces it.
                let _ = std::fs::rename(aside, path);
                return Err(error);
            }
        };
        Ok(StandIn {
            session,
            role,
            path: path.to_path_buf(),
            aside: aside.to_path_buf(),
            listener,
            links: Vec::new(),
        })
    }

    /// The session whose endpoint the stand-in answers.
    pub fn session(&self) -> &SessionId {
        &self.session
    }

    /// Accepts every connection that waits, and reads and answers what the host sent on each one. True when the stand-in
    /// has answered a host's hello, now or before.
    ///
    /// # Panics
    /// When the link does not take the whole answer: R-42 makes a short or failed write a setup failure, never a pass.
    pub fn step(&mut self) -> bool {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    // A stream that cannot be made non-blocking is never read, so a step never waits on it.
                    let stream = stream.set_nonblocking(true).ok().map(|()| stream);
                    self.links.push(PeerLink {
                        stream,
                        decoder: FrameDecoder::new(DEFAULT_MAX_PAYLOAD),
                        answered: false,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        if let Role::Answer(plan) = &self.role {
            for link in &mut self.links {
                link.read_and_answer(plan);
            }
        }
        self.links.iter().any(|l| l.answered)
    }
}

impl PeerLink {
    /// Reads what the host sent, and answers its first hello. At the end of the stream, the host has closed the link, and
    /// the stand-in closes its end.
    fn read_and_answer(&mut self, plan: &ImpostorPlan) {
        let mut buf = [0u8; 4096];
        while let Some(stream) = self.stream.as_mut() {
            let n = match stream.read(&mut buf) {
                Ok(0) => {
                    self.stream = None;
                    return;
                }
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(_) => {
                    self.stream = None;
                    return;
                }
            };
            let mut rest = &buf[..n];
            while !rest.is_empty() {
                // The decoder takes bytes up to the end of the current frame.
                let took = self.decoder.push(rest);
                rest = &rest[took..];
                while let Ok(Some(frame)) = self.decoder.next_frame() {
                    if frame.kind != FrameType::HELLO || self.answered {
                        continue;
                    }
                    if let Ok(hello) = Hello::decode(&frame.payload) {
                        let answer = impostor_answer(plan, &hello);
                        let sent = stream.write(&answer).ok();
                        assert_eq!(
                            sent,
                            Some(answer.len()),
                            "R-42: the stand-in's hello and its script are one whole write"
                        );
                        self.answered = true;
                    }
                }
                if took == 0 {
                    break;
                }
            }
        }
    }
}

impl Drop for StandIn {
    /// R-49 condition 2: the real endpoint goes back to its path. The rename replaces the stand-in's socket file, or takes
    /// the path again if Core removed that file (`remove_endpoint`). The stand-in's listener closes after this.
    fn drop(&mut self) {
        let _ = std::fs::rename(&self.aside, &self.path);
    }
}

#[cfg(test)]
mod tests;
