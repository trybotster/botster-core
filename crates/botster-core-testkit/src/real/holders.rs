//! The descriptor count of `route_stream_holders` on the real tier (Core DP-1, DP-2, OU-1, OU-11; plan 23bf): the number of
//! open descriptors of a route's worker-end socket in every process that this user can read.
//!
//! - Linux: a descriptor of the socket reads `socket:[<inode>]` through `/proc/<pid>/fd` (the inode of
//!   [`crate::stream_holders::socket_id`]).
//! - macOS: `lsof` (`/usr/sbin/lsof`, part of the OS) gives the kernel's address of each Unix socket (field `d`), the same
//!   for every descriptor of one socket. The harness reads the address of the worker end before Core takes it, and later
//!   counts the descriptors with that address in every process. It needs no unsafe code: the workspace forbids it.
//!
//! A process that this user cannot read is not counted. Only a process that got the descriptor from the harness, Core or the
//! worker can hold it, and all of them run as this user. A process that ends during the count is not counted.

use crate::stream_holders::SocketId;
use std::io;
use std::os::fd::BorrowedFd;

/// What the count of one socket needs: its identity, and on macOS the kernel's address that `lsof` gives.
#[derive(Debug, Clone)]
pub(super) struct Socket {
    pub(super) id: SocketId,
    #[cfg(target_os = "macos")]
    address: String,
}

/// The socket of `fd`, read while this process holds `fd`.
///
/// # Errors
/// `fstat` failed, or (macOS) `lsof` gave no address for `fd`.
pub(super) fn socket(fd: BorrowedFd<'_>) -> io::Result<Socket> {
    let id = crate::stream_holders::socket_id(fd)?;
    #[cfg(target_os = "macos")]
    let address = darwin::address(fd)?;
    Ok(Socket {
        id,
        #[cfg(target_os = "macos")]
        address,
    })
}

/// The number of open descriptors of `socket` in every process that this user can read.
///
/// # Errors
/// The list of the processes could not be read.
#[cfg(target_os = "linux")]
pub(super) fn holders(socket: &Socket) -> io::Result<usize> {
    linux::holders(socket.id)
}

/// The number of open descriptors of `socket` in every process that this user can read.
///
/// # Errors
/// `lsof` failed.
#[cfg(target_os = "macos")]
pub(super) fn holders(socket: &Socket) -> io::Result<usize> {
    darwin::holders(&socket.address)
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::stream_holders::SocketId;
    use std::io;
    use std::path::Path;

    pub(super) fn holders(id: SocketId) -> io::Result<usize> {
        let target = format!("socket:[{}]", id.inode);
        let mut count = 0;
        for entry in std::fs::read_dir("/proc")? {
            let name = entry?.file_name();
            if !name
                .to_str()
                .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
            {
                continue;
            }
            // A process that ended or that this user cannot read gives no list.
            let Ok(fds) = std::fs::read_dir(Path::new("/proc").join(&name).join("fd")) else {
                continue;
            };
            count += fds
                .flatten()
                .filter(|fd| {
                    std::fs::read_link(fd.path())
                        .is_ok_and(|link| link.as_os_str() == target.as_str())
                })
                .count();
        }
        Ok(count)
    }
}

#[cfg(target_os = "macos")]
mod darwin {
    use botster_test_process::{run_to_completion, Deadline};
    use std::io;
    use std::os::fd::{AsRawFd, BorrowedFd};
    use std::process::Command;

    /// `lsof`'s lines for the Unix sockets that `args` select, in the field form: `p<pid>`, `f<fd>` and `d<address>`.
    fn lsof(args: &[&str]) -> io::Result<String> {
        let mut command = Command::new("/usr/sbin/lsof");
        command.args(["-n", "-P", "-U", "-F", "d"]).args(args);
        // timer: deadline — a stuck lsof must fail the control, not hang the run
        let output = run_to_completion(&mut command, Deadline::cleanup())?;
        // lsof exits 1 when a selection matched nothing; its output is still complete.
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(io::Error::other(format!(
                "lsof failed: {:?}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        String::from_utf8(output.stdout).map_err(io::Error::other)
    }

    /// The addresses in `lsof` field output.
    pub(super) fn addresses(out: &str) -> impl Iterator<Item = &str> {
        out.lines().filter_map(|line| line.strip_prefix('d'))
    }

    pub(super) fn address(fd: BorrowedFd<'_>) -> io::Result<String> {
        let pid = std::process::id().to_string();
        let fd = fd.as_raw_fd().to_string();
        let out = lsof(&["-a", "-p", &pid, "-d", &fd])?;
        let mut found = addresses(&out);
        match (found.next(), found.next()) {
            (Some(address), None) => Ok(address.to_string()),
            _ => Err(io::Error::other(format!(
                "lsof gave no single address for descriptor {fd}: {out:?}"
            ))),
        }
    }

    pub(super) fn holders(address: &str) -> io::Result<usize> {
        Ok(addresses(&lsof(&[])?).filter(|a| *a == address).count())
    }
}

#[cfg(test)]
mod slow_tests {
    use super::*;
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;

    /// The count on this OS (the Linux gate and the Mac slow tier): each open and close of a descriptor of the socket
    /// changes it by one, a descriptor of the peer is not counted, and the count is 0 when the socket has no descriptor,
    /// while its peer lives.
    #[test]
    fn the_count_follows_each_descriptor_of_the_socket() {
        let (a, b) = UnixStream::pair().unwrap();
        let of_a = socket(a.as_fd()).unwrap();
        assert_eq!(holders(&of_a).unwrap(), 1);
        let copy = a.try_clone().unwrap();
        assert_eq!(holders(&of_a).unwrap(), 2);
        drop(a);
        assert_eq!(holders(&of_a).unwrap(), 1);
        drop(copy);
        assert_eq!(holders(&of_a).unwrap(), 0);
        let peer = socket(b.as_fd()).unwrap();
        assert_eq!(holders(&peer).unwrap(), 1, "the peer lives");
    }
}
