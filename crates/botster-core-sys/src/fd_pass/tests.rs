use super::*;
use rustix::io::fcntl_getfd;
use std::io::Write;
use std::os::unix::net::UnixStream;

/// A connected pair, and a second pair whose one end is the descriptor to pass: the test writes through the received copy and
/// reads at the other end, so a received descriptor is shown to be the same stream.
fn pairs() -> (UnixStream, UnixStream, UnixStream, UnixStream) {
    let (a, b) = sending_pair();
    let (passed, peer) = UnixStream::pair().unwrap();
    (a, b, passed, peer)
}

/// A connected pair whose first end can send a descriptor: on macOS it has `SO_NOSIGPIPE`, as a Mio socket has.
fn sending_pair() -> (UnixStream, UnixStream) {
    let (a, b) = UnixStream::pair().unwrap();
    #[cfg(target_os = "macos")]
    rustix::net::sockopt::set_socket_nosigpipe(&a, true).unwrap();
    (a, b)
}

/// The tag is written before the read, so a nonblocking read of the peer takes it at once: the read never waits.
fn assert_same_stream(received: OwnedFd, peer: &mut UnixStream, tag: &[u8]) {
    let mut stream = UnixStream::from(received);
    stream.write_all(tag).unwrap();
    peer.set_nonblocking(true).unwrap();
    let mut got = vec![0u8; tag.len() + 1];
    let read = recv_with_fds(&*peer, &mut got).unwrap();
    assert_eq!(&got[..read.len], tag);
}

#[test]
fn a_descriptor_arrives_with_the_bytes_and_with_close_on_exec() {
    let (a, b, passed, mut peer) = pairs();
    assert_eq!(send_with_fd(&a, b"attach", passed.as_fd()).unwrap(), 6);
    let mut buf = [0u8; 64];
    let received = recv_with_fds(&b, &mut buf).unwrap();
    assert_eq!(&buf[..received.len], b"attach");
    assert_eq!(received.fds.len(), 1);
    let fd = received.fds.into_iter().next().unwrap();
    assert!(
        fcntl_getfd(&fd).unwrap().contains(FdFlags::CLOEXEC),
        "the received descriptor is close-on-exec"
    );
    assert_same_stream(fd, &mut peer, b"same");
}

/// Two sends back to back, each with a descriptor, and a read buffer large enough for both: each is a read of its own with
/// its own descriptor, on both kernels (a read holds at most one descriptor segment).
#[test]
fn two_sends_with_descriptors_are_two_reads_each_with_its_own() {
    let (a, b, first, mut first_peer) = pairs();
    let (second, mut second_peer) = UnixStream::pair().unwrap();
    assert_eq!(send_with_fd(&a, b"one", first.as_fd()).unwrap(), 3);
    assert_eq!(send_with_fd(&a, b"two!", second.as_fd()).unwrap(), 4);
    let mut buf = [0u8; 64];
    let one = recv_with_fds(&b, &mut buf).unwrap();
    assert_eq!(
        &buf[..one.len],
        b"one",
        "the read stops at the second descriptor"
    );
    assert_eq!(one.fds.len(), 1);
    let two = recv_with_fds(&b, &mut buf).unwrap();
    assert_eq!(&buf[..two.len], b"two!");
    assert_eq!(two.fds.len(), 1);
    assert_same_stream(one.fds.into_iter().next().unwrap(), &mut first_peer, b"1");
    assert_same_stream(two.fds.into_iter().next().unwrap(), &mut second_peer, b"2");
}

/// Where a read stops around a descriptor's segment (plain bytes, a send with a descriptor, plain bytes; all sent before the
/// reads). The two kernels differ, and both keep at most one descriptor segment in a read:
/// - macOS stops a read before a segment with descriptors once it has copied data, and goes on after it: the descriptor
///   comes with the read's first byte.
/// - Linux goes on into a segment with descriptors and stops after it: the descriptor's segment is the end of the read.
#[test]
fn a_read_holds_one_descriptor_segment_at_its_start_on_macos_and_at_its_end_on_linux() {
    let (mut a, b, passed, _peer) = pairs();
    a.write_all(b"plain").unwrap();
    send_with_fd(&a, b"attach", passed.as_fd()).unwrap();
    a.write_all(b"after").unwrap();
    b.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 64];
    let mut reads = Vec::new();
    while let Ok(read) = recv_with_fds(&b, &mut buf) {
        reads.push((buf[..read.len].to_vec(), read.fds.len()));
    }
    let expected: [(&[u8], usize); 2] = if cfg!(target_os = "macos") {
        [(b"plain", 0), (b"attachafter", 1)]
    } else {
        [(b"plainattach", 1), (b"after", 0)]
    };
    let expected: Vec<(Vec<u8>, usize)> = expected.iter().map(|(b, n)| (b.to_vec(), *n)).collect();
    assert_eq!(reads, expected);
}

/// Sends one message with `count` descriptors.
fn send_many(socket: &UnixStream, count: usize) {
    let extra: Vec<UnixStream> = (0..count).map(|_| UnixStream::pair().unwrap().0).collect();
    let fds: Vec<BorrowedFd<'_>> = extra.iter().map(AsFd::as_fd).collect();
    let mut space = vec![MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(count))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    assert!(control.push(SendAncillaryMessage::ScmRights(&fds)));
    sendmsg(
        socket,
        &[IoSlice::new(b"x")],
        &mut control,
        SendFlags::empty(),
    )
    .unwrap();
}

/// Lead condition 3: more descriptors in one message than one read takes is an error, never a read with descriptors missing.
/// Exactly the room is a normal read.
#[test]
fn more_descriptors_than_the_room_is_an_error() {
    let mut buf = [0u8; 8];
    for count in [MAX_FDS + 1, 8 * MAX_FDS] {
        let (a, b) = UnixStream::pair().unwrap();
        send_many(&a, count);
        let error = recv_with_fds(&b, &mut buf).unwrap_err();
        assert!(
            error.to_string().contains("more descriptors"),
            "{count}: {error}"
        );
    }
    let (a, b) = UnixStream::pair().unwrap();
    send_many(&a, MAX_FDS);
    assert_eq!(recv_with_fds(&b, &mut buf).unwrap().fds.len(), MAX_FDS);
}

#[test]
fn a_descriptor_needs_a_byte() {
    let (a, _b, passed, _peer) = pairs();
    assert_eq!(
        send_with_fd(&a, b"", passed.as_fd()).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

/// A full socket takes nothing: `WouldBlock`, and the descriptor is not sent (the reader finds none).
#[test]
fn a_full_socket_takes_neither_the_bytes_nor_the_descriptor() {
    let (mut a, b, passed, _peer) = pairs();
    a.set_nonblocking(true).unwrap();
    let chunk = [0u8; 4096];
    let mut filled = 0;
    loop {
        match a.write(&chunk) {
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(
        send_with_fd(&a, b"attach", passed.as_fd())
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    b.set_nonblocking(true).unwrap();
    let mut buf = vec![0u8; 1 << 16];
    let mut drained = 0;
    while drained < filled {
        let received = recv_with_fds(&b, &mut buf).unwrap();
        assert!(received.fds.is_empty(), "no descriptor went");
        drained += received.len;
    }
    assert_eq!(drained, filled);
    // With room again, the same send goes, with its descriptor.
    assert_eq!(send_with_fd(&a, b"attach", passed.as_fd()).unwrap(), 6);
    let attach = loop {
        match recv_with_fds(&b, &mut buf) {
            Ok(received) => break received,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("{e}"),
        }
    };
    assert_eq!((attach.len, attach.fds.len()), (6, 1));
}

/// A send that the socket takes in part: the descriptor goes once, with the first byte, and the rest of the bytes follow
/// with none.
#[test]
fn a_short_send_carries_the_descriptor_once() {
    let (a, b, passed, _peer) = pairs();
    a.set_nonblocking(true).unwrap();
    let big = vec![7u8; 1 << 20];
    let sent = send_with_fd(&a, &big, passed.as_fd()).unwrap();
    assert!(sent >= 1 && sent < big.len(), "a short send: {sent}");
    let mut buf = vec![0u8; 1 << 20];
    let (mut total, mut fds) = (0, 0);
    while total < sent {
        let received = recv_with_fds(&b, &mut buf).unwrap();
        if received.fds.len() == 1 {
            assert_eq!(total, 0, "the descriptor comes with the first byte");
        }
        fds += received.fds.len();
        total += received.len;
    }
    assert_eq!((total, fds), (sent, 1));
}

#[test]
fn the_end_of_the_stream_is_a_read_of_nothing() {
    let (a, b) = UnixStream::pair().unwrap();
    drop(a);
    let mut buf = [0u8; 8];
    let received = recv_with_fds(&b, &mut buf).unwrap();
    assert_eq!((received.len, received.fds.len()), (0, 0));
}

/// Lead condition 3: a message that the kernel cuts (`MSG_CTRUNC`; here a control space for one descriptor, and 32 sent) is
/// an error, and it is not parsed, so the process does not abort on macOS (see `CONTROL_BYTES`).
#[test]
fn a_message_that_the_kernel_cuts_is_an_error_and_is_not_parsed() {
    let (a, b) = UnixStream::pair().unwrap();
    // rustix's space for one descriptor holds a few (it rounds up for the alignment): far more is cut on both kernels.
    send_many(&a, 8 * MAX_FDS);
    let mut buf = [0u8; 8];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let error = recv_into(&b, &mut buf, &mut space).unwrap_err();
    assert!(error.to_string().contains("MSG_CTRUNC"), "{error}");
}

/// The control space is larger than the most descriptors that this kernel takes in one message: a send of one more than
/// `CONTROL_BYTES` holds is refused by the sender's kernel, so a read never has to cut.
#[test]
fn the_kernel_refuses_a_message_with_more_descriptors_than_the_control_space_holds() {
    let (_, keep) = UnixStream::pair().unwrap();
    let mut n = 1;
    while rustix::cmsg_space!(ScmRights(n + 1)) <= CONTROL_BYTES {
        n += 1;
    }
    // `n` descriptors fill the space; one more does not fit, and neither does any count the kernel takes above its cap.
    let (a, _b) = UnixStream::pair().unwrap();
    let fds: Vec<BorrowedFd<'_>> = (0..=n).map(|_| keep.as_fd()).collect();
    let mut space = vec![MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(n + 1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    assert!(control.push(SendAncillaryMessage::ScmRights(&fds)));
    let refused = sendmsg(&a, &[IoSlice::new(b"x")], &mut control, SendFlags::empty());
    assert_eq!(
        refused,
        Err(rustix::io::Errno::INVAL),
        "{} descriptors",
        n + 1
    );
}

/// A send to a socket whose reader is gone is `BrokenPipe` (`EPIPE`). The test binary ignores `SIGPIPE` (the Rust runtime
/// does), so this test shows the error only; the real-process test with the default disposition shows that no signal comes.
#[test]
fn a_send_to_a_gone_reader_is_a_broken_pipe() {
    let (a, b, passed, _peer) = pairs();
    drop(b);
    assert_eq!(
        send_with_fd(&a, b"attach", passed.as_fd())
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}

/// On macOS a socket without `SO_NOSIGPIPE` is refused before the send: the reader gets neither the bytes nor the
/// descriptor.
#[cfg(target_os = "macos")]
#[test]
fn a_socket_without_no_sigpipe_is_refused_and_sends_nothing() {
    let (a, b) = UnixStream::pair().unwrap();
    let (passed, _peer) = UnixStream::pair().unwrap();
    let error = send_with_fd(&a, b"attach", passed.as_fd()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{error}");
    b.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 8];
    assert_eq!(
        recv_with_fds(&b, &mut buf).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

/// The guard of `EMSGSIZE` (macOS): a message that fits the send buffer waits for room; a larger one is an error, never
/// a retry.
#[cfg(target_os = "macos")]
#[test]
fn emsgsize_waits_only_for_a_message_that_fits_the_send_buffer() {
    assert!(emsgsize_waits(3, 4));
    assert!(emsgsize_waits(4, 4));
    assert!(!emsgsize_waits(5, 4), "it can never fit");
}

/// A frame larger than the socket's send buffer: the send takes at most the buffer's size, with the descriptor, and the
/// rest goes as plain bytes. The reader gets the descriptor once, with the first byte, and every byte.
#[test]
fn a_frame_larger_than_the_send_buffer_sends_a_prefix_with_the_descriptor() {
    let (mut a, b, passed, _peer) = pairs();
    rustix::net::sockopt::set_socket_send_buffer_size(&a, 2048).unwrap();
    let send_buffer = rustix::net::sockopt::socket_send_buffer_size(&a).unwrap();
    let frame: Vec<u8> = (0..4 * send_buffer).map(|i| i as u8).collect();
    let reader = std::thread::spawn(move || {
        let mut buf = vec![0u8; 1 << 16];
        let (mut got, mut fds) = (Vec::new(), Vec::new());
        loop {
            let received = recv_with_fds(&b, &mut buf).unwrap();
            if received.len == 0 {
                break (got, fds);
            }
            fds.extend(received.fds.into_iter().map(|_| got.len()));
            got.extend_from_slice(&buf[..received.len]);
        }
    });
    let sent = send_with_fd(&a, &frame, passed.as_fd()).unwrap();
    assert!(sent >= 1 && sent <= send_buffer, "{sent} of {send_buffer}");
    a.write_all(&frame[sent..]).unwrap();
    drop(a);
    let (got, fds) = reader.join().unwrap();
    assert_eq!(got, frame);
    assert_eq!(fds, [0], "one descriptor, with the first byte");
}

/// Lead ruling on PR A (plan 23ac): a real send of a message of exactly the send buffer's size, on an empty socket, goes
/// with its descriptor (the largest send that `send_with_fd` makes), and the reader gets every byte and the descriptor once.
#[test]
fn a_message_of_exactly_the_send_buffer_size_goes_with_its_descriptor() {
    let (mut a, b, passed, _peer) = pairs();
    let send_buffer = rustix::net::sockopt::socket_send_buffer_size(&a).unwrap();
    let frame: Vec<u8> = (0..send_buffer).map(|i| i as u8).collect();
    let reader = std::thread::spawn(move || {
        let mut buf = vec![0u8; 1 << 16];
        let (mut got, mut fds) = (Vec::new(), Vec::new());
        loop {
            let received = recv_with_fds(&b, &mut buf).unwrap();
            if received.len == 0 {
                break (got, fds);
            }
            fds.extend(received.fds.into_iter().map(|_| got.len()));
            got.extend_from_slice(&buf[..received.len]);
        }
    });
    let sent = send_with_fd(&a, &frame, passed.as_fd()).unwrap();
    assert!(sent >= 1, "{sent} of {send_buffer}");
    a.write_all(&frame[sent..]).unwrap();
    drop(a);
    let (got, fds) = reader.join().unwrap();
    assert_eq!(got, frame);
    assert_eq!(fds, [0], "one descriptor, with the first byte");
}
