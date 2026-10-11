//! The real route transport (DP-2, OU-3, DP-5; plan section 6.3 item 1): the bound route streams and their bounded I/O.
//!
//! The machine binds a stream to a route (`BindRoute`), asks for writes (`RouteWrite`, one out per route), lets the route
//! read a number of client bytes (`RouteReadAllowance`), and closes it (`RouteClose`). This module holds those streams and
//! their readiness, and turns each read and write into the machine's input. It is generic over the stream, so its tests
//! use a scripted stream; the driver registers each stream with its poll under the token that [`Routes::bind`] gives.
//!
//! **Bounded turns (plan 2.4, 2.5).** The readiness is edge-triggered, as for the control socket: a flag per direction is
//! set by an event and cleared only when a read or write finds `WouldBlock`, or when a write of some bytes takes none
//! (F122). Each turn writes at most one pending write and
//! reads at most one chunk of each route, and a read takes no more than the allowance, so a client that writes without
//! end gets transport backpressure (DP-5) and cannot hold back the other sources. The driver gives the read bound: the one
//! bound of its control socket and PTY reads.

use crate::io_decisions::{failure, IoFailure, EIO};
use botster_core_contract::prelude::RouteId;
use botster_worker_core::Input;
use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Write};
use std::num::NonZeroUsize;

/// The poll token bit of the route streams. A candidate's token never has it, so the two kinds of token never meet.
pub const ROUTE_TOKEN: usize = 1 << (usize::BITS - 1);

/// Whether `token` is a route stream's token.
pub fn is_route_token(token: usize) -> bool {
    token & ROUTE_TOKEN != 0
}

/// One bound route stream.
#[derive(Debug)]
struct RouteIo<S> {
    stream: S,
    token: usize,
    readable: bool,
    writable: bool,
    /// The one write that the machine asked for and the stream has not answered yet.
    write: Option<Vec<u8>>,
    /// The stream took no byte of the last write: the next writable event gives `RouteWritable`.
    wait_writable: bool,
    /// The client closed the stream, or a read failed: the machine was told once, and the stream is not read again.
    ended: bool,
    /// The client bytes that the route may still read (`RouteReadAllowance`): none before the first allowance.
    allowance: usize,
}

impl<S> RouteIo<S> {
    fn reads(&self) -> bool {
        self.readable && !self.ended && self.allowance > 0
    }

    fn writes(&self) -> bool {
        self.writable && self.write.is_some()
    }
}

/// The bound route streams, by route.
#[derive(Debug)]
pub struct Routes<S> {
    /// The most bytes that one read of a route's stream takes.
    read_chunk: NonZeroUsize,
    next_token: usize,
    routes: BTreeMap<RouteId, RouteIo<S>>,
}

impl<S: Read + Write> Routes<S> {
    /// No bound route yet. One read of a route takes at most `read_chunk` bytes.
    pub fn new(read_chunk: NonZeroUsize) -> Self {
        Routes {
            read_chunk,
            next_token: 0,
            routes: BTreeMap::new(),
        }
    }

    /// `BindRoute`: the stream is the route's transport from now. Returns its poll token: a new one for each bind. Both
    /// directions start as ready, so the first turn finds out what the stream holds.
    pub fn bind(&mut self, route: RouteId, stream: S) -> usize {
        self.next_token += 1;
        let token = ROUTE_TOKEN + self.next_token;
        self.routes.insert(
            route,
            RouteIo {
                stream,
                token,
                readable: true,
                writable: true,
                write: None,
                wait_writable: false,
                ended: false,
                allowance: 0,
            },
        );
        token
    }

    /// `RouteClose`: the route leaves the transport. The driver deregisters the stream, and its drop closes it.
    pub fn close(&mut self, route: RouteId) -> Option<S> {
        self.routes.remove(&route).map(|io| io.stream)
    }

    /// `RouteWrite`: the bytes wait for the route's next turn.
    pub fn write(&mut self, route: RouteId, bytes: Vec<u8>) {
        if let Some(io) = self.routes.get_mut(&route) {
            io.write = Some(bytes);
        }
    }

    /// `RouteReadAllowance`: the route may read `bytes` more client bytes until the next allowance.
    pub fn allow(&mut self, route: RouteId, bytes: usize) {
        if let Some(io) = self.routes.get_mut(&route) {
            io.allowance = bytes;
        }
    }

    /// A poll event of `token`. A closed or failed stream is readable and writable, so the next read or write meets its
    /// end. Returns `RouteWritable` when the stream takes bytes again after a write that it did not take.
    pub fn event(
        &mut self,
        token: usize,
        readable: bool,
        writable: bool,
        closed: bool,
        error: bool,
    ) -> Option<Input> {
        let (&route, io) = self.routes.iter_mut().find(|(_, io)| io.token == token)?;
        io.readable |= readable || closed || error;
        io.writable |= writable || error;
        if io.writable && io.wait_writable {
            io.wait_writable = false;
            return Some(Input::RouteWritable { route });
        }
        None
    }

    /// One turn of every route: at most one write and one read each. Each answer is one input.
    pub fn turn(&mut self, inputs: &mut VecDeque<Input>) {
        for (&route, io) in &mut self.routes {
            if io.writes() {
                write_once(route, io, inputs);
            }
            if io.reads() {
                read_once(route, io, self.read_chunk, inputs);
            }
        }
    }

    /// A route can write or read in the next turn, so the poll must not wait.
    pub fn busy(&self) -> bool {
        self.routes.values().any(|io| io.writes() || io.reads())
    }
}

/// The route's pending write, once. `Interrupted` keeps it for the next turn; `WouldBlock` (or a write that took nothing)
/// is `Ok(0)` and waits for writable; another error is terminal.
fn write_once<S: Write>(route: RouteId, io: &mut RouteIo<S>, inputs: &mut VecDeque<Input>) {
    let Some(bytes) = io.write.take() else {
        return;
    };
    let result = match io.stream.write(&bytes) {
        Ok(n) => Ok(n),
        Err(error) => match failure(&error) {
            IoFailure::Retry => {
                io.write = Some(bytes);
                return;
            }
            IoFailure::Blocked => {
                io.writable = false;
                Ok(0)
            }
            IoFailure::Closed => Err(error.raw_os_error().unwrap_or(EIO)),
        },
    };
    if result == Ok(0) && !bytes.is_empty() {
        // The stream took no byte, so this write used up the readiness, as `WouldBlock` does (F122).
        io.writable = false;
        io.wait_writable = true;
    }
    inputs.push_back(Input::RouteWritten { route, result });
}

/// One read of the route, bounded by `read_chunk` and its allowance. The end of the stream, or an error other than `Interrupted` and
/// `WouldBlock`, ends the route's reads (OU-5): `RouteEnded`, once.
fn read_once<S: Read>(
    route: RouteId,
    io: &mut RouteIo<S>,
    read_chunk: NonZeroUsize,
    inputs: &mut VecDeque<Input>,
) {
    let mut buf = vec![0u8; read_chunk.get().min(io.allowance)];
    match io.stream.read(&mut buf) {
        Ok(0) => {
            io.ended = true;
            inputs.push_back(Input::RouteEnded { route });
        }
        Ok(n) => {
            buf.truncate(n);
            io.allowance -= n;
            inputs.push_back(Input::RouteRead { route, bytes: buf });
        }
        Err(error) => match failure(&error) {
            IoFailure::Retry => {}
            IoFailure::Blocked => io.readable = false,
            IoFailure::Closed => {
                io.ended = true;
                inputs.push_back(Input::RouteEnded { route });
            }
        },
    }
}

#[cfg(test)]
mod tests;
