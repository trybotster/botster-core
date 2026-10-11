use super::*;
use crate::io_decisions::candidate_token;
use botster_worker_core::CandidateId;
use std::io;

/// A stream whose reads and writes give the scripted results in order. A read result `Ok(n)` delivers `n` bytes of `b'x'`
/// (at most the buffer). A call with no result left is a test failure.
#[derive(Debug, Default)]
struct Scripted {
    reads: VecDeque<io::Result<usize>>,
    writes: VecDeque<io::Result<usize>>,
    /// The buffer length of each read, and the bytes of each write.
    read_lens: Vec<usize>,
    written: Vec<Vec<u8>>,
}

impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_lens.push(buf.len());
        let n = self.reads.pop_front().expect("a scripted read")?;
        let n = n.min(buf.len());
        buf[..n].fill(b'x');
        Ok(n)
    }
}

impl Write for Scripted {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.push(buf.to_vec());
        self.writes.pop_front().expect("a scripted write")
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const R: RouteId = RouteId(7);

fn routes(read_chunk: usize) -> Routes<Scripted> {
    Routes::new(NonZeroUsize::new(read_chunk).unwrap())
}

fn bound(stream: Scripted) -> (Routes<Scripted>, usize) {
    let mut routes = routes(64 * 1024);
    let token = routes.bind(R, stream);
    (routes, token)
}

fn turn(routes: &mut Routes<Scripted>) -> Vec<Input> {
    let mut inputs = VecDeque::new();
    routes.turn(&mut inputs);
    inputs.into()
}

fn err(kind: io::ErrorKind) -> io::Result<usize> {
    Err(io::Error::from(kind))
}

/// A route reads nothing before its first allowance, and never more than its allowance: DP-5's transport backpressure.
#[test]
fn a_route_reads_only_within_its_allowance() {
    let (mut routes, _) = bound(Scripted {
        reads: VecDeque::from([Ok(3), Ok(100)]),
        ..Scripted::default()
    });
    assert!(!routes.busy());
    assert_eq!(turn(&mut routes), []);
    routes.allow(R, 5);
    assert!(routes.busy());
    assert_eq!(
        turn(&mut routes),
        [Input::RouteRead {
            route: R,
            bytes: b"xxx".to_vec()
        }]
    );
    // Two bytes are left of the allowance: the read's buffer is two bytes.
    assert_eq!(
        turn(&mut routes),
        [Input::RouteRead {
            route: R,
            bytes: b"xx".to_vec()
        }]
    );
    assert!(!routes.busy());
    assert_eq!(turn(&mut routes), []);
    assert_eq!(routes.close(R).unwrap().read_lens, [5, 2]);
}

/// One read of a route takes at most the driver's read bound, whatever its allowance: at the driver's bound and at the
/// other bounds of its read tests.
#[test]
fn a_read_takes_at_most_one_chunk() {
    for chunk in [65536, 1088, 1] {
        let mut routes = routes(chunk);
        routes.bind(
            R,
            Scripted {
                reads: VecDeque::from([Ok(usize::MAX)]),
                ..Scripted::default()
            },
        );
        routes.allow(R, usize::MAX);
        let inputs = turn(&mut routes);
        assert!(
            matches!(&inputs[..], [Input::RouteRead { bytes, .. }] if bytes.len() == chunk),
            "{chunk}: {}",
            inputs.len()
        );
        assert_eq!(routes.close(R).unwrap().read_lens, [chunk]);
    }
}

/// `WouldBlock` clears the read readiness until the next readable event; `Interrupted` reads again in the next turn.
#[test]
fn a_blocked_read_waits_for_its_event_and_an_interrupted_read_retries() {
    let (mut routes, token) = bound(Scripted {
        reads: VecDeque::from([
            err(io::ErrorKind::Interrupted),
            err(io::ErrorKind::WouldBlock),
            Ok(1),
        ]),
        ..Scripted::default()
    });
    routes.allow(R, 10);
    assert_eq!(turn(&mut routes), []);
    assert!(routes.busy(), "an interrupted read reads again");
    assert_eq!(turn(&mut routes), []);
    assert!(!routes.busy(), "a blocked read waits");
    assert_eq!(turn(&mut routes), []);
    assert_eq!(routes.event(token, true, false, false, false), None);
    assert_eq!(
        turn(&mut routes),
        [Input::RouteRead {
            route: R,
            bytes: b"x".to_vec()
        }]
    );
}

/// OU-5: the end of the stream, and a read error, end the route's reads once: no read follows.
#[test]
fn the_end_of_a_stream_or_a_read_error_ends_the_route_once() {
    for end in [Ok(0), err(io::ErrorKind::ConnectionReset)] {
        let (mut routes, token) = bound(Scripted {
            reads: VecDeque::from([end]),
            ..Scripted::default()
        });
        routes.allow(R, 10);
        assert_eq!(turn(&mut routes), [Input::RouteEnded { route: R }]);
        assert_eq!(routes.event(token, true, false, true, false), None);
        assert!(!routes.busy());
        assert_eq!(turn(&mut routes), []);
        assert_eq!(routes.close(R).unwrap().read_lens, [10]);
    }
}

/// A write goes out in the route's turn, once, and its answer is the bytes that the stream took (OU-3a).
#[test]
fn a_write_is_answered_with_the_bytes_taken() {
    let (mut routes, _) = bound(Scripted {
        writes: VecDeque::from([Ok(2)]),
        ..Scripted::default()
    });
    assert!(!routes.busy());
    routes.write(R, b"abc".to_vec());
    assert!(routes.busy());
    assert_eq!(
        turn(&mut routes),
        [Input::RouteWritten {
            route: R,
            result: Ok(2)
        }]
    );
    assert!(!routes.busy());
    assert_eq!(turn(&mut routes), []);
    assert_eq!(routes.close(R).unwrap().written, [b"abc".to_vec()]);
}

/// A write that the stream does not take (`WouldBlock`) is `Ok(0)`; the next writable event gives `RouteWritable` once.
/// `Interrupted` writes the same bytes again in the next turn.
#[test]
fn a_blocked_write_waits_for_writable_and_an_interrupted_write_retries() {
    let (mut routes, token) = bound(Scripted {
        writes: VecDeque::from([
            err(io::ErrorKind::Interrupted),
            err(io::ErrorKind::WouldBlock),
        ]),
        ..Scripted::default()
    });
    routes.write(R, b"ab".to_vec());
    assert_eq!(turn(&mut routes), []);
    assert!(routes.busy(), "an interrupted write writes again");
    assert_eq!(
        turn(&mut routes),
        [Input::RouteWritten {
            route: R,
            result: Ok(0)
        }]
    );
    assert!(!routes.busy());
    // A readable event is not the end of the wait.
    assert_eq!(routes.event(token, true, false, false, false), None);
    assert_eq!(
        routes.event(token, false, true, false, false),
        Some(Input::RouteWritable { route: R })
    );
    assert_eq!(routes.event(token, false, true, false, false), None);
    assert_eq!(
        routes.close(R).unwrap().written,
        [b"ab".to_vec(), b"ab".to_vec()]
    );
}

/// A write that took no byte without `WouldBlock` waits for writable too; a write error is terminal and does not wait.
#[test]
fn a_write_of_no_byte_waits_and_a_write_error_is_terminal() {
    let (mut routes, token) = bound(Scripted {
        writes: VecDeque::from([Ok(0), err(io::ErrorKind::BrokenPipe)]),
        ..Scripted::default()
    });
    routes.write(R, b"a".to_vec());
    assert_eq!(
        turn(&mut routes),
        [Input::RouteWritten {
            route: R,
            result: Ok(0)
        }]
    );
    assert_eq!(
        routes.event(token, false, true, false, false),
        Some(Input::RouteWritable { route: R })
    );
    routes.write(R, b"a".to_vec());
    let inputs = turn(&mut routes);
    assert!(
        matches!(
            &inputs[..],
            [Input::RouteWritten { route: R, result: Err(errno) }] if *errno != 0
        ),
        "{inputs:?}"
    );
    assert_eq!(routes.event(token, false, true, false, false), None);
}

/// An error event makes both directions ready, so the next turn meets the stream's error.
#[test]
fn an_error_event_makes_both_directions_ready() {
    let (mut routes, token) = bound(Scripted {
        reads: VecDeque::from([err(io::ErrorKind::WouldBlock), Ok(0)]),
        writes: VecDeque::from([
            err(io::ErrorKind::WouldBlock),
            err(io::ErrorKind::BrokenPipe),
        ]),
        ..Scripted::default()
    });
    routes.allow(R, 1);
    routes.write(R, b"a".to_vec());
    turn(&mut routes);
    assert!(!routes.busy());
    assert_eq!(
        routes.event(token, false, false, false, true),
        Some(Input::RouteWritable { route: R })
    );
    routes.write(R, b"a".to_vec());
    let inputs = turn(&mut routes);
    assert!(
        inputs.contains(&Input::RouteEnded { route: R }),
        "{inputs:?}"
    );
    assert!(
        inputs
            .iter()
            .any(|i| matches!(i, Input::RouteWritten { result: Err(_), .. })),
        "{inputs:?}"
    );
}

/// Each bind gives a new route token, and no route token is a candidate's.
#[test]
fn route_tokens_are_new_and_never_a_candidates() {
    let mut routes = routes(1);
    let a = routes.bind(RouteId(1), Scripted::default());
    let b = routes.bind(RouteId(2), Scripted::default());
    routes.close(RouteId(1));
    let c = routes.bind(RouteId(1), Scripted::default());
    assert!(a != b && b != c && a != c);
    for token in [a, b, c] {
        assert!(is_route_token(token));
    }
    for id in [0, 1, 1 << 40] {
        assert!(!is_route_token(candidate_token(CandidateId(id))));
    }
    assert_eq!(routes.event(a, true, true, false, false), None);
}

/// After `RouteClose`, the route reads and writes nothing, and its events give nothing.
#[test]
fn a_closed_route_does_nothing() {
    let (mut routes, token) = bound(Scripted::default());
    routes.allow(R, 10);
    routes.write(R, b"a".to_vec());
    assert!(routes.close(R).is_some());
    assert!(routes.close(R).is_none());
    routes.allow(R, 10);
    routes.write(R, b"a".to_vec());
    assert!(!routes.busy());
    assert_eq!(turn(&mut routes), []);
    assert_eq!(routes.event(token, true, true, true, true), None);
}

/// A closed event alone makes the stream readable, so the next read meets the end of the stream.
#[test]
fn a_closed_event_makes_the_stream_readable() {
    let (mut routes, token) = bound(Scripted {
        reads: VecDeque::from([err(io::ErrorKind::WouldBlock), Ok(0)]),
        ..Scripted::default()
    });
    routes.allow(R, 1);
    assert_eq!(turn(&mut routes), []);
    assert!(!routes.busy());
    assert_eq!(routes.event(token, false, false, true, false), None);
    assert!(routes.busy());
    assert_eq!(turn(&mut routes), [Input::RouteEnded { route: R }]);
}

/// Only a write that took no byte of its bytes waits: after a write that took bytes, or a write of no bytes, a writable
/// event gives nothing.
#[test]
fn only_a_write_that_took_no_byte_waits_for_writable() {
    let (mut routes, token) = bound(Scripted {
        writes: VecDeque::from([Ok(1), Ok(0)]),
        ..Scripted::default()
    });
    routes.write(R, b"ab".to_vec());
    assert_eq!(
        turn(&mut routes),
        [Input::RouteWritten {
            route: R,
            result: Ok(1)
        }]
    );
    assert_eq!(routes.event(token, false, true, false, false), None);
    routes.write(R, Vec::new());
    assert_eq!(
        turn(&mut routes),
        [Input::RouteWritten {
            route: R,
            result: Ok(0)
        }]
    );
    assert_eq!(routes.event(token, false, true, false, false), None);
}
