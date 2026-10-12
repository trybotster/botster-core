//! The unread bytes of a route's worker-end socket, for `route_frames_taken` on the real tier (Core AM-2; steward ruling R-93
//! Q1, the real-tier serving point and its conditions). A read-only kernel query of the socket's receive buffer: it holds no
//! reference to the socket and adds no wait, lock or synchronization point (R-14.3). The socket is found by the identity that
//! the harness read before Core took the worker's end ([`super::holders::Socket`]), and the client's end is a cross-check.
//!
//! - Linux: `NETLINK_SOCK_DIAG`, an exact `unix_diag` lookup of the worker end's inode with `UDIAG_SHOW_PEER` and
//!   `UDIAG_SHOW_RQLEN`. The reply's peer must be the client's end, and `udiag_rqueue` is the unread bytes. The request and
//!   the reply are built and parsed by [`crate::sock_diag`], in safe code (evidence `pc-route-frames-taken/probe-linux.out`).
//! - macOS: `/usr/sbin/netstat -f unix -n` with `LC_ALL=C` (the ruling's netstat path: the workspace forbids unsafe code, so
//!   no `proc_pidfdinfo`). Exactly one row has the worker end's address in `Address` and the client end's in `Conn`. Its
//!   `Recv-Q` is the socket's `so_rcv.sb_cc` (evidence `pc-route-frames-taken/nsprobe-mac.out`).
//!
//! Every failure is an error, which the control reports as a harness error: never `unsupported_control`, never a pass
//! (condition (b)). That includes a socket that is not found, which is what a worker end that closed gives.

use super::holders::Socket;
use std::io;

/// The bytes that the worker's end of the route holds unread: the client wrote them, and the worker has not read them.
///
/// # Errors
/// The query failed, the worker's end was not found, its peer is not the client's end, or the answer did not parse.
#[cfg(target_os = "linux")]
pub(super) fn unread(worker: &Socket, client: &Socket) -> io::Result<u64> {
    let worker_ino = crate::sock_diag::inode(worker.id.inode)?;
    let client_ino = crate::sock_diag::inode(client.id.inode)?;
    let reply = linux::exchange(&crate::sock_diag::request(worker_ino))?;
    let found = crate::sock_diag::parse(&reply, worker_ino)?;
    if found.peer != client_ino {
        return Err(io::Error::other(format!(
            "the peer of the worker's end is socket {}, not the client's end {client_ino}",
            found.peer
        )));
    }
    Ok(u64::from(found.unread))
}

/// The bytes that the worker's end of the route holds unread: the client wrote them, and the worker has not read them.
///
/// # Errors
/// `netstat` failed or timed out, its output did not parse, or not exactly one row names both ends.
#[cfg(target_os = "macos")]
pub(super) fn unread(worker: &Socket, client: &Socket) -> io::Result<u64> {
    let worker = netstat::address(worker.address())?;
    let client = netstat::address(client.address())?;
    netstat::unread(&netstat::run()?, worker, client)
}

#[cfg(target_os = "linux")]
mod linux {
    use rustix::net::{
        netlink, recv, sendto, socket_with, AddressFamily, RecvFlags, SendFlags, SocketFlags,
        SocketType,
    };
    use std::io;

    /// The largest reply of one exact lookup: one message with two small attributes.
    const REPLY_BYTES: usize = 1024;

    /// Sends one `NETLINK_SOCK_DIAG` request and takes its reply. The kernel answers an exact `unix_diag` lookup inside
    /// `sendto`, so the reply waits on the socket when `sendto` returns, and the read does not wait (`MSG_DONTWAIT`).
    pub(super) fn exchange(request: &[u8]) -> io::Result<Vec<u8>> {
        let socket = socket_with(
            AddressFamily::NETLINK,
            SocketType::RAW,
            SocketFlags::CLOEXEC,
            Some(netlink::SOCK_DIAG),
        )?;
        let kernel = netlink::SocketAddrNetlink::new(0, 0);
        let sent = sendto(&socket, request, SendFlags::empty(), &kernel)?;
        if sent != request.len() {
            return Err(io::Error::other(format!(
                "sock_diag took {sent} of {} request bytes",
                request.len()
            )));
        }
        let mut reply = vec![0u8; REPLY_BYTES];
        let (n, _) = recv(&socket, &mut reply[..], RecvFlags::DONTWAIT)?;
        reply.truncate(n);
        Ok(reply)
    }
}

/// `netstat -f unix -n` on macOS: run it, and find the one row of the route's socket pair. The parse is pure text, so the
/// tests run on every OS.
#[cfg(any(target_os = "macos", test))]
pub(super) mod netstat {
    use std::io;

    /// The columns that the header must name, in order, from its first column.
    const HEADER: [&str; 6] = ["Address", "Type", "Recv-Q", "Send-Q", "Inode", "Conn"];

    fn bad(what: String) -> io::Error {
        io::Error::other(format!("netstat: {what}"))
    }

    /// A kernel address as `lsof` (`0x…`) or `netstat` (bare hex) prints it.
    pub(crate) fn address(text: &str) -> io::Result<u64> {
        let hex = text.strip_prefix("0x").unwrap_or(text);
        u64::from_str_radix(hex, 16).map_err(|e| bad(format!("address {text:?}: {e}")))
    }

    /// The `Recv-Q` of the one row whose `Address` is `worker` and whose `Conn` is `client`. The output must be the title,
    /// the header that [`HEADER`] names, and rows that each parse. No such row, or more than one, is an error.
    pub(crate) fn unread(output: &str, worker: u64, client: u64) -> io::Result<u64> {
        let mut lines = output
            .lines()
            .skip_while(|line| !line.starts_with("Address"));
        let header: Vec<&str> = lines
            .next()
            .ok_or_else(|| bad("no header".into()))?
            .split_whitespace()
            .collect();
        if header.get(..HEADER.len()) != Some(&HEADER[..]) {
            return Err(bad(format!("an unknown header {header:?}")));
        }
        let mut found = vec![];
        for line in lines.filter(|line| !line.trim().is_empty()) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            let [at, _, recv_q, _, _, conn, ..] = cols[..] else {
                return Err(bad(format!("a row with too few columns: {line:?}")));
            };
            let recv_q: u64 = recv_q
                .parse()
                .map_err(|e| bad(format!("Recv-Q {recv_q:?}: {e}")))?;
            if address(at)? == worker && address(conn)? == client {
                found.push(recv_q);
            }
        }
        match found[..] {
            [unread] => Ok(unread),
            [] => Err(bad("no row has the worker's end and the client's end: the worker's end is closed, or the pair is not this route's".into())),
            _ => Err(bad(format!("{} rows name the route's socket pair", found.len()))),
        }
    }

    /// The output of `/usr/sbin/netstat -f unix -n` with `LC_ALL=C`, under the cleanup deadline. A failed run is an error.
    #[cfg(target_os = "macos")]
    pub(crate) fn run() -> io::Result<String> {
        use botster_test_process::{run_to_completion, Deadline};
        use std::process::Command;
        let mut command = Command::new("/usr/sbin/netstat");
        command.args(["-f", "unix", "-n"]).env("LC_ALL", "C");
        // timer: deadline — a stuck netstat must fail the control, not hang the run
        let output = run_to_completion(&mut command, Deadline::cleanup())?;
        if !output.status.success() {
            return Err(bad(format!(
                "exited {:?}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        String::from_utf8(output.stdout).map_err(|e| bad(e.to_string()))
    }
}

#[cfg(test)]
mod slow_tests {
    use super::netstat;
    use crate::sock_diag as diag;

    /// One reply message: a header of `kind`, then `body`.
    fn message(kind: u16, body: &[u8]) -> Vec<u8> {
        let mut out = ((16 + body.len()) as u32).to_ne_bytes().to_vec();
        out.extend_from_slice(&kind.to_ne_bytes());
        out.extend_from_slice(&[0; 10]);
        out.extend_from_slice(body);
        out
    }

    /// One attribute of `kind`, padded to 4 bytes.
    fn attribute(kind: u16, data: &[u8]) -> Vec<u8> {
        let mut out = ((4 + data.len()) as u16).to_ne_bytes().to_vec();
        out.extend_from_slice(&kind.to_ne_bytes());
        out.extend_from_slice(data);
        out.resize(out.len().next_multiple_of(4), 0);
        out
    }

    /// A `unix_diag_msg` of socket `ino`, then `attributes`.
    fn answer(ino: u32, attributes: &[Vec<u8>]) -> Vec<u8> {
        let mut body = vec![1, 1, 1, 0];
        body.extend_from_slice(&ino.to_ne_bytes());
        body.extend_from_slice(&[0xff; 8]);
        attributes.iter().for_each(|a| body.extend_from_slice(a));
        message(20, &body)
    }

    /// The request is `struct nlmsghdr` and `struct unix_diag_req` for an exact lookup: `SOCK_DIAG_BY_FAMILY`, a request,
    /// `AF_UNIX`, every state, the inode, the peer and the queue, no cookie.
    #[test]
    fn the_diag_request_is_an_exact_lookup_of_the_inode() {
        let r = diag::request(0x0102_0304);
        assert_eq!(r.len(), 40);
        assert_eq!(&r[0..4], &40u32.to_ne_bytes());
        assert_eq!(&r[4..6], &20u16.to_ne_bytes());
        assert_eq!(&r[6..8], &1u16.to_ne_bytes());
        assert_eq!(&r[8..16], &[0; 8]);
        assert_eq!(&r[16..20], &[1, 0, 0, 0]);
        assert_eq!(&r[20..24], &u32::MAX.to_ne_bytes());
        assert_eq!(&r[24..28], &0x0102_0304u32.to_ne_bytes());
        assert_eq!(&r[28..32], &0x14u32.to_ne_bytes());
        assert_eq!(&r[32..40], &[0xff; 8]);
        assert!(diag::inode(u64::from(u32::MAX)).is_ok());
        assert!(diag::inode(u64::from(u32::MAX) + 1).is_err());
    }

    /// The reply gives the peer and the unread bytes (`unix_diag_rqlen.udiag_rqueue`), in either attribute order and
    /// beside other attributes. A missing attribute, another socket, an error, or a cut message is an error.
    #[test]
    fn the_diag_reply_gives_the_peer_and_the_unread_bytes() {
        let peer = attribute(2, &7u32.to_ne_bytes());
        let mut rq = 70u32.to_ne_bytes().to_vec();
        rq.extend_from_slice(&0u32.to_ne_bytes());
        let rqlen = attribute(4, &rq);
        let other = attribute(6, &[1]);
        let found = diag::Found {
            peer: 7,
            unread: 70,
        };
        let both = answer(9, &[peer.clone(), other.clone(), rqlen.clone()]);
        assert_eq!(diag::parse(&both, 9).unwrap(), found);
        assert_eq!(
            diag::parse(&answer(9, &[rqlen.clone(), peer.clone()]), 9).unwrap(),
            found
        );
        assert!(diag::parse(&both, 8)
            .unwrap_err()
            .to_string()
            .contains("another socket"));
        assert!(diag::parse(&answer(9, &[peer]), 9)
            .unwrap_err()
            .to_string()
            .contains("no receive queue"));
        assert!(diag::parse(&answer(9, &[rqlen]), 9)
            .unwrap_err()
            .to_string()
            .contains("no peer"));
        // The error message carries the negative errno, and the error names it (ENOENT is os error 2).
        let enoent = message(2, &(-2i32).to_ne_bytes());
        let error = diag::parse(&enoent, 9).unwrap_err().to_string();
        assert!(error.contains("not found"), "{error}");
        assert!(error.contains("os error 2)"), "{error}");
        // R319-SI-1: an error code with no positive errno (i32::MIN, whose negation overflows, and 0) is a reply that does not
        // parse; the smallest negative errno (-1, EPERM) is a not-found error.
        for errno in [i32::MIN, 0] {
            let error = diag::parse(&message(2, &errno.to_ne_bytes()), 9)
                .unwrap_err()
                .to_string();
            assert!(error.contains("not a negative errno"), "{errno}: {error}");
        }
        let eperm = diag::parse(&message(2, &(-1i32).to_ne_bytes()), 9)
            .unwrap_err()
            .to_string();
        assert!(eperm.contains("os error 1)"), "{eperm}");
        assert!(diag::parse(&message(3, &[]), 9).is_err());
        assert!(
            diag::parse(&both[..both.len() - 1], 9).is_err(),
            "a cut message"
        );
        assert!(diag::parse(&[], 9).is_err());
        let mut long_attribute = answer(9, &[other]);
        let at = long_attribute.len() - 8;
        long_attribute[at..at + 2].copy_from_slice(&64u16.to_ne_bytes());
        assert!(diag::parse(&long_attribute, 9).is_err());
    }

    /// The netstat output of the Mac probe (`nsprobe-mac.out`), with the client's end in `Conn` of the worker's row.
    const NETSTAT: &str = "Active LOCAL (UNIX) domain sockets
Address          Type   Recv-Q Send-Q            Inode             Conn             Refs          Nextref Addr
e9575853a07bc8fe stream     70      0                0  5f17a8d81240c2e                0                0
 5f17a8d81240c2e stream      0      0                0 e9575853a07bc8fe                0                0
d1d2d3d4d5d6d7d8 stream      9      0                0                0                0                0 /var/run/x.sock
";

    /// The row of the worker's end whose `Conn` is the client's end gives `Recv-Q`. Addresses compare as numbers, so
    /// `lsof`'s `0x` form and netstat's padding match.
    #[test]
    fn netstat_gives_the_recv_q_of_the_one_row_of_the_pair() {
        let worker = netstat::address("0xe9575853a07bc8fe").unwrap();
        let client = netstat::address("0x5f17a8d81240c2e").unwrap();
        assert_eq!(netstat::unread(NETSTAT, worker, client).unwrap(), 70);
        assert_eq!(
            netstat::unread(NETSTAT, client, worker).unwrap(),
            0,
            "the client's end"
        );
        let missing = netstat::unread(NETSTAT, worker, 1).unwrap_err().to_string();
        assert!(missing.contains("no row"), "{missing}");
        let twice = format!("{NETSTAT}e9575853a07bc8fe stream 1 0 0 5f17a8d81240c2e 0 0\n");
        assert!(netstat::unread(&twice, worker, client)
            .unwrap_err()
            .to_string()
            .contains("2 rows"));
        assert!(netstat::address("0xzz").is_err());
    }

    /// Anything that does not parse is an error: no header, another header, a short row, a bad `Recv-Q` or address.
    #[test]
    fn netstat_output_that_does_not_parse_is_an_error() {
        let (w, c) = (1, 2);
        assert!(netstat::unread("", w, c).is_err());
        assert!(netstat::unread("Address Type Recv-Q\n", w, c).is_err());
        let header = "Address Type Recv-Q Send-Q Inode Conn Refs Nextref Addr\n";
        assert_eq!(
            netstat::unread(&format!("{header}1 stream 5 0 0 2 0 0\n"), w, c).unwrap(),
            5
        );
        assert!(netstat::unread(&format!("{header}1 stream 5 0 0\n"), w, c).is_err());
        assert!(netstat::unread(&format!("{header}1 stream x 0 0 2 0 0\n"), w, c).is_err());
        assert!(netstat::unread(&format!("{header}g stream 5 0 0 2 0 0\n"), w, c).is_err());
        assert!(netstat::unread(&format!("{header}1 stream 5 0 0 g 0 0\n"), w, c).is_err());
    }
}
