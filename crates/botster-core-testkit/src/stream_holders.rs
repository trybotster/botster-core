//! The identity of a route's stream socket (`route_stream_holders`, plan 23bf): `fstat`'s device and inode. On Linux and
//! macOS, a socket has its own inode, and every descriptor of that socket gives the same one. The edge tap counts the
//! bytes that Core writes to a route's stream by it; the real harness counts the socket's descriptors (`real::holders`).

use std::io;
use std::os::fd::BorrowedFd;

/// The identity of one socket: equal for every descriptor of that socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SocketId {
    pub device: u64,
    pub inode: u64,
}

/// The identity of the socket of `fd`.
///
/// # Errors
/// `fstat` failed.
pub fn socket_id(fd: BorrowedFd<'_>) -> io::Result<SocketId> {
    let stat = rustix::fs::fstat(fd)?;
    Ok(SocketId {
        // The device of a socket is a placeholder (-1 on macOS): only its bits matter here.
        #[allow(clippy::cast_sign_loss, clippy::unnecessary_cast)]
        device: stat.st_dev as u64,
        #[allow(clippy::unnecessary_cast)]
        inode: stat.st_ino as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;

    /// A `dup` has the identity of its socket; the peer and another pair have others.
    #[test]
    fn a_socket_has_one_identity_for_all_its_descriptors() {
        let (a, b) = UnixStream::pair().unwrap();
        let (c, _d) = UnixStream::pair().unwrap();
        let id = socket_id(a.as_fd()).unwrap();
        assert_eq!(socket_id(a.try_clone().unwrap().as_fd()).unwrap(), id);
        assert_ne!(socket_id(b.as_fd()).unwrap(), id);
        assert_ne!(socket_id(c.as_fd()).unwrap(), id);
    }
}
