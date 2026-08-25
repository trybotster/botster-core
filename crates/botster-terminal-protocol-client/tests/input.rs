#![allow(missing_docs)]

use botster_terminal_protocol_client::{
    decode_terminal_input, encode_terminal_input, TerminalInputCommand, TerminalInputEncodeError,
    TerminalInputKind, MAX_INPUT_DATA_BYTES, MAX_MODE_GATED_DATA_BYTES,
};

#[test]
fn encode_decode_round_trips_all_kinds_including_non_utf8() {
    let commands = [
        TerminalInputCommand::Input {
            data: vec![0x00, 0xff, 0x1b, b'a'],
        },
        TerminalInputCommand::ModeGatedInput {
            mode_generation: u64::MAX,
            mode_revision: 0,
            data: b"paste".to_vec(),
        },
        TerminalInputCommand::Resize {
            rows: u16::MAX,
            cols: 0,
        },
    ];
    for command in commands {
        let frame = encode_terminal_input(&command).expect("encode");
        let decoded = decode_terminal_input(&frame).expect("decode");
        assert_eq!(decoded, command);
    }
}

#[test]
fn encode_is_fallible_at_exact_per_kind_ceilings() {
    let input_ok = TerminalInputCommand::Input {
        data: vec![0; MAX_INPUT_DATA_BYTES],
    };
    encode_terminal_input(&input_ok).expect("input ceiling encodes");
    let input_over = TerminalInputCommand::Input {
        data: vec![0; MAX_INPUT_DATA_BYTES + 1],
    };
    match encode_terminal_input(&input_over) {
        Err(TerminalInputEncodeError::PayloadTooLarge { kind, max, actual }) => {
            assert_eq!(kind, TerminalInputKind::Input);
            assert_eq!(max, MAX_INPUT_DATA_BYTES);
            assert_eq!(actual, MAX_INPUT_DATA_BYTES + 1);
        }
        other => panic!("expected input PayloadTooLarge, got {other:?}"),
    }

    let gated_ok = TerminalInputCommand::ModeGatedInput {
        mode_generation: 1,
        mode_revision: 1,
        data: vec![0; MAX_MODE_GATED_DATA_BYTES],
    };
    encode_terminal_input(&gated_ok).expect("mode-gated ceiling encodes");
    let gated_over = TerminalInputCommand::ModeGatedInput {
        mode_generation: 1,
        mode_revision: 1,
        data: vec![0; MAX_MODE_GATED_DATA_BYTES + 1],
    };
    match encode_terminal_input(&gated_over) {
        Err(TerminalInputEncodeError::PayloadTooLarge { kind, max, actual }) => {
            assert_eq!(kind, TerminalInputKind::ModeGatedInput);
            assert_eq!(max, MAX_MODE_GATED_DATA_BYTES);
            assert_eq!(actual, MAX_MODE_GATED_DATA_BYTES + 1);
        }
        other => panic!("expected mode-gated PayloadTooLarge, got {other:?}"),
    }
}

#[test]
fn decode_rejects_resize_body_that_is_not_four_bytes() {
    let mut bytes = vec![1, 3, 0, 3];
    bytes.extend_from_slice(&[0, 24, 80]);
    let frame =
        botster_terminal_protocol::TerminalInputFrame::from_bytes(&bytes).expect("header is valid");
    let error = decode_terminal_input(&frame).expect_err("resize body must be 4");
    assert!(error.to_string().contains("4"), "{error}");
}
