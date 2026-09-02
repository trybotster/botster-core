//! Isolated TUI-shaped consumer over the semantic client crate.

use botster_terminal_protocol_client::{
    encode_paste, AttachState, AttachStateKind, ProcessExit, Snapshot, SnapshotPhase,
    TerminalEvent, TerminalInputKind, TerminalInputResult, TerminalModeFlags, TerminalOutput,
};

pub fn encode_representative_events() -> Vec<Vec<u8>> {
    let snapshot = Snapshot::from_bytes("s", "sub", b"ready", SnapshotPhase::Ready)
        .to_frame()
        .expect("snapshot frame")
        .to_bytes()
        .expect("snapshot bytes");
    let live = TerminalOutput::from_bytes("s", "sub", b"out")
        .to_frame()
        .expect("live frame")
        .to_bytes()
        .expect("live bytes");
    let exit = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: None,
    }
    .to_frame()
    .expect("exit frame")
    .to_bytes()
    .expect("exit bytes");
    let attach = AttachState {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        state: AttachStateKind::Attached,
    }
    .to_frame()
    .expect("attach frame")
    .to_bytes()
    .expect("attach bytes");
    let paste_frames = encode_paste(1, 2, 3, b"tui paste").expect("paste frames");
    let paste_result = TerminalEvent::InputResult(TerminalInputResult {
        subscription_id: "sub".into(),
        kind: TerminalInputKind::Paste,
        operation_id: Some(1),
        admitted: true,
        bytes_written: 9,
        mode_generation: 2,
        mode_revision: 3,
        mode_flags: TerminalModeFlags {
            kitty_enabled: false,
            cursor_visible: true,
            bracketed_paste: false,
            mouse_mode: 0,
            alt_screen: false,
            focus_reporting: false,
            application_cursor: false,
        },
        rejection: None,
    })
    .to_frame()
    .expect("paste result")
    .to_bytes()
    .expect("paste result bytes");
    let mut events = vec![snapshot, live, exit, attach, paste_result];
    events.extend(paste_frames.into_iter().map(|frame| frame.to_bytes()));
    events
}
