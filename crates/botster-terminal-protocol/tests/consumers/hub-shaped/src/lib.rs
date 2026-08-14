//! Isolated Hub-shaped consumer. Depends only on the opaque protocol crate.

use botster_terminal_protocol::{Attach, TerminalFrame};

pub fn forward_attach(session_id: &str, subscription_id: &str) -> String {
    let request = Attach {
        session_id: session_id.to_string(),
        subscription_id: subscription_id.to_string(),
    };
    serde_json::to_string(&request).expect("serialize attach")
}

pub fn forward_frame(bytes: &[u8]) -> Vec<u8> {
    TerminalFrame::from_bytes(bytes)
        .expect("opaque frame")
        .to_bytes()
        .expect("emit frame")
}
