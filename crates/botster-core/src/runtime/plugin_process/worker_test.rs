//! The child's receive bound is the negotiated frame size, not the codec's.

use std::io::Write;
use std::os::unix::net::UnixStream;

use super::Channel;
use crate::contract::session_protocol::{encode_frame, ProtocolError, MAX_FRAME_LEN};

const BOUND: usize = 64;

#[test]
fn a_frame_over_the_negotiated_bound_is_refused_below_the_codec_cap() {
    let (mut parent, child) = UnixStream::pair().expect("socketpair");
    let mut channel = Channel::new(child, BOUND);
    let oversize = BOUND + 36;
    assert!(oversize < MAX_FRAME_LEN);

    parent
        .write_all(&encode_frame(0x02, &vec![b'x'; oversize - 1]).expect("encode"))
        .expect("write");

    assert!(matches!(
        channel.recv(),
        Err(ProtocolError::FrameLengthTooLarge { len, max: BOUND }) if len == oversize
    ));
}

#[test]
fn a_frame_at_the_negotiated_bound_is_received() {
    let (mut parent, child) = UnixStream::pair().expect("socketpair");
    let mut channel = Channel::new(child, BOUND);

    parent
        .write_all(&encode_frame(0x02, &[b'x'; BOUND - 1]).expect("encode"))
        .expect("write");
    drop(parent);

    let frame = channel.recv().expect("received").expect("a frame");
    assert_eq!(frame.payload.len(), BOUND - 1);
    assert!(channel.recv().expect("clean EOF").is_none());
}
