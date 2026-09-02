#![allow(missing_docs)]

use botster_terminal_protocol_client::{
    decode_terminal_input, encode_paste, encode_paste_abort, encode_terminal_input,
    TerminalInputCommand, TerminalInputEncodeError, TerminalInputKind, MAX_INPUT_DATA_BYTES,
    MAX_MODE_GATED_DATA_BYTES, MAX_PASTE_BYTES, MAX_PASTE_CHUNK_DATA_BYTES,
};

#[test]
fn encode_decode_round_trips_all_kinds_including_non_utf8() {
    let commands = vec![
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
        TerminalInputCommand::PasteBegin {
            operation_id: 7,
            mode_generation: u64::MAX,
            mode_revision: 9,
            total_len: 3,
        },
        TerminalInputCommand::PasteChunk {
            operation_id: 7,
            index: 0,
            data: vec![0, 0xff, 3],
        },
        TerminalInputCommand::PasteCommit { operation_id: 7 },
        TerminalInputCommand::PasteAbort { operation_id: 8 },
    ];
    for command in commands {
        let frame = encode_terminal_input(&command).expect("encode");
        let decoded = decode_terminal_input(&frame).expect("decode");
        assert_eq!(decoded, command);
    }
}

#[test]
fn encode_paste_uses_fixed_ordered_chunks_and_exact_bounds() {
    for size in [
        1,
        MAX_PASTE_CHUNK_DATA_BYTES,
        MAX_PASTE_CHUNK_DATA_BYTES + 1,
        MAX_PASTE_BYTES,
    ] {
        let data: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
        let frames = encode_paste(42, 11, 12, &data).expect("paste encodes");
        let commands: Vec<_> = frames
            .iter()
            .map(|frame| decode_terminal_input(frame).expect("paste frame decodes"))
            .collect();
        assert_eq!(
            commands.first(),
            Some(&TerminalInputCommand::PasteBegin {
                operation_id: 42,
                mode_generation: 11,
                mode_revision: 12,
                total_len: size as u32,
            })
        );
        let chunks = &commands[1..commands.len() - 1];
        assert_eq!(chunks.len(), size.div_ceil(MAX_PASTE_CHUNK_DATA_BYTES));
        let mut assembled = Vec::new();
        for (index, command) in chunks.iter().enumerate() {
            let TerminalInputCommand::PasteChunk {
                operation_id,
                index: actual_index,
                data,
            } = command
            else {
                panic!("expected paste chunk, got {command:?}");
            };
            assert_eq!(*operation_id, 42);
            assert_eq!(*actual_index, index as u32);
            if index + 1 < chunks.len() {
                assert_eq!(data.len(), MAX_PASTE_CHUNK_DATA_BYTES);
            }
            assembled.extend_from_slice(data);
        }
        assert_eq!(assembled, data);
        assert_eq!(
            commands.last(),
            Some(&TerminalInputCommand::PasteCommit { operation_id: 42 })
        );
    }

    assert_eq!(
        encode_paste(1, 1, 1, &[]),
        Err(TerminalInputEncodeError::EmptyPaste)
    );
    assert!(matches!(
        encode_paste(1, 1, 1, &vec![0; MAX_PASTE_BYTES + 1]),
        Err(TerminalInputEncodeError::PayloadTooLarge {
            kind: TerminalInputKind::Paste,
            max: MAX_PASTE_BYTES,
            actual
        }) if actual == MAX_PASTE_BYTES + 1
    ));
    assert_eq!(
        decode_terminal_input(&encode_paste_abort(99)).expect("abort decodes"),
        TerminalInputCommand::PasteAbort { operation_id: 99 }
    );
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
