//! Conformance assertions for downstream consumers.

use botster_core::session::{SessionId, SubscriptionId};
use botster_core::session_protocol::{encode_frame, FrameDecoder, FRAME_PTY_OUTPUT};
use botster_core::transport::TransportEgress;

/// Assert that terminal output chunks round-trip through public core contracts.
///
/// This exercises the session protocol frame encoder/decoder and the
/// transport-neutral terminal egress shape without depending on private runtime
/// implementation details.
pub fn assert_terminal_output_round_trips(
    session_id: SessionId,
    subscription_id: SubscriptionId,
    chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
) -> Vec<TransportEgress> {
    let chunks: Vec<Vec<u8>> = chunks
        .into_iter()
        .map(|chunk| chunk.as_ref().to_vec())
        .collect();

    let mut encoded = Vec::new();
    for chunk in &chunks {
        let frame = encode_frame(FRAME_PTY_OUTPUT, chunk)
            .expect("terminal output chunks should encode as PTY output frames");
        encoded.extend_from_slice(&frame);
    }

    let mut decoder = FrameDecoder::new();
    let frames = decoder
        .feed(&encoded)
        .expect("encoded terminal output frames should decode");

    assert_eq!(frames.len(), chunks.len());
    for (frame, chunk) in frames.iter().zip(&chunks) {
        assert_eq!(frame.frame_type, FRAME_PTY_OUTPUT);
        assert_eq!(&frame.payload, chunk);
    }

    let egress: Vec<TransportEgress> = chunks
        .into_iter()
        .map(|chunk| TransportEgress::TerminalOutput {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
            data: chunk,
        })
        .collect();
    let json = serde_json::to_string(&egress)
        .expect("terminal output egress should serialize through public serde contract");
    let decoded: Vec<TransportEgress> = serde_json::from_str(&json)
        .expect("terminal output egress should deserialize through public serde contract");
    assert_eq!(decoded, egress);

    egress
}
