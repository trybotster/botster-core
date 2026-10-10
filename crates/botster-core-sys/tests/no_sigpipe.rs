//! Core A49: a descriptor send to a socket whose reader is gone fails with `EPIPE` and raises no `SIGPIPE`
//! (`botster_core_sys::fd_pass`, "No `SIGPIPE`").
//!
//! The Rust runtime ignores `SIGPIPE`, and an ignored signal is discarded, so an ignored disposition cannot show that no
//! signal came. This target installs a `SIGPIPE` handler that records each `SIGPIPE`, for its whole process, so no other
//! test runs in it. It has no test harness (`harness = false`): its one thread is the main thread, so the kernel gives a
//! `SIGPIPE` that a write raises to that thread before the write returns. In a process with more threads, macOS may give
//! it to another thread, later. A raw write to the same kind of socket is the control: the handler sees its `SIGPIPE` at
//! once, in each round. The descriptor send raises none.

use botster_core_sys::fd_pass::send_with_fd;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Rounds of the control and the send: one round could pass by chance if the delivery came late.
const ROUNDS: usize = 100;

fn main() {
    let raised = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGPIPE, Arc::clone(&raised)).unwrap();
    for round in 0..ROUNDS {
        // The control: a raw write, with no flag, to a socket whose reader is gone raises `SIGPIPE` (the socket has no
        // `SO_NOSIGPIPE`).
        let (a, b) = UnixStream::pair().unwrap();
        drop(b);
        assert_eq!(rustix::io::write(&a, b"x"), Err(rustix::io::Errno::PIPE));
        assert!(
            raised.swap(false, Ordering::SeqCst),
            "round {round}: the handler sees the control's SIGPIPE before the write returns"
        );

        // The descriptor send, on a socket as the host's link is (on macOS Mio sets `SO_NOSIGPIPE` on each socket).
        let (a, b) = UnixStream::pair().unwrap();
        #[cfg(target_os = "macos")]
        rustix::net::sockopt::set_socket_nosigpipe(&a, true).unwrap();
        let (passed, _peer) = UnixStream::pair().unwrap();
        drop(b);
        let error = send_with_fd(&a, b"attach", passed.as_fd()).unwrap_err();
        assert_eq!(
            error.kind(),
            io::ErrorKind::BrokenPipe,
            "round {round}: {error}"
        );
        assert!(!raised.load(Ordering::SeqCst), "round {round}: no SIGPIPE");
    }
    println!("no_sigpipe: {ROUNDS} rounds: each control raised SIGPIPE; no descriptor send did");
}
