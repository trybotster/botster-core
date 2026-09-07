#![allow(missing_docs)]

use botster_terminal_protocol_client::{
    decode_terminal_input, encode_paste, encode_paste_abort, encode_terminal_input,
    TerminalInputCommand, TerminalInputEncodeError, TerminalInputKind, TerminalKey,
    TerminalKeyAction, TerminalMouseAction, TerminalMouseButton, INPUT_HEADER_BYTES,
    MAX_PASTE_BYTES, MAX_PASTE_CHUNK_DATA_BYTES, MAX_RAW_INPUT_BYTES,
    TERMINAL_INPUT_SCHEME_VERSION,
};

#[test]
fn encode_decode_round_trips_all_kinds_including_non_utf8() {
    let commands = vec![
        TerminalInputCommand::RawBytes {
            operation_id: 1,
            data: vec![0x00, 0xff, 0x1b, b'a'],
        },
        TerminalInputCommand::Key {
            operation_id: 2,
            action: TerminalKeyAction::Press,
            key: TerminalKey::Backquote,
            mods: 0x0003,
            consumed_mods: 0x0001,
            composing: true,
            unshifted_codepoint: u32::MAX,
            text: "é".to_string(),
        },
        TerminalInputCommand::Mouse {
            operation_id: 3,
            action: TerminalMouseAction::Press,
            button: Some(TerminalMouseButton::Left),
            mods: 0,
            col: u16::MAX,
            row: 7,
            x_px: 9,
            y_px: u32::MAX,
        },
        TerminalInputCommand::Mouse {
            operation_id: 4,
            action: TerminalMouseAction::Release,
            button: None,
            mods: 0,
            col: 0,
            row: 0,
            x_px: 0,
            y_px: 0,
        },
        TerminalInputCommand::Focus {
            operation_id: 5,
            focused: false,
        },
        TerminalInputCommand::Resize {
            operation_id: u64::MAX,
            rows: u16::MAX,
            cols: 0,
            width_px: 1,
            height_px: u32::MAX,
        },
        TerminalInputCommand::PasteBegin {
            operation_id: 7,
            total_len: 3,
            allow_unsafe: true,
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
        assert_eq!(frame.operation_id(), command.operation_id());
        assert_eq!(frame.kind(), command.kind());
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
        let frames = encode_paste(42, false, &data).expect("paste encodes");
        let commands: Vec<_> = frames
            .iter()
            .map(|frame| decode_terminal_input(frame).expect("paste frame decodes"))
            .collect();
        assert_eq!(
            commands.first(),
            Some(&TerminalInputCommand::PasteBegin {
                operation_id: 42,
                total_len: size as u32,
                allow_unsafe: false,
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
            assert!(command.continues_paste());
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
        encode_paste(1, false, &[]),
        Err(TerminalInputEncodeError::EmptyPaste)
    );
    assert!(matches!(
        encode_paste(1, false, &vec![0; MAX_PASTE_BYTES + 1]),
        Err(TerminalInputEncodeError::PayloadTooLarge {
            kind: TerminalInputKind::PasteBegin,
            max: MAX_PASTE_BYTES,
            actual
        }) if actual == MAX_PASTE_BYTES + 1
    ));
    let abort = encode_paste_abort(99).expect("abort encodes");
    assert_eq!(
        decode_terminal_input(&abort).expect("abort decodes"),
        TerminalInputCommand::PasteAbort { operation_id: 99 }
    );
}

#[test]
fn encode_is_fallible_at_exact_per_kind_ceilings() {
    let raw_ok = TerminalInputCommand::RawBytes {
        operation_id: 1,
        data: vec![0; MAX_RAW_INPUT_BYTES],
    };
    encode_terminal_input(&raw_ok).expect("raw ceiling encodes");
    let raw_over = TerminalInputCommand::RawBytes {
        operation_id: 1,
        data: vec![0; MAX_RAW_INPUT_BYTES + 1],
    };
    match encode_terminal_input(&raw_over) {
        Err(TerminalInputEncodeError::PayloadTooLarge { kind, max, actual }) => {
            assert_eq!(kind, TerminalInputKind::RawBytes);
            assert_eq!(max, MAX_RAW_INPUT_BYTES);
            assert_eq!(actual, MAX_RAW_INPUT_BYTES + 1);
        }
        other => panic!("expected raw PayloadTooLarge, got {other:?}"),
    }

    let chunk_ok = TerminalInputCommand::PasteChunk {
        operation_id: 1,
        index: 0,
        data: vec![0; MAX_PASTE_CHUNK_DATA_BYTES],
    };
    encode_terminal_input(&chunk_ok).expect("chunk ceiling encodes");
    let chunk_over = TerminalInputCommand::PasteChunk {
        operation_id: 1,
        index: 0,
        data: vec![0; MAX_PASTE_CHUNK_DATA_BYTES + 1],
    };
    match encode_terminal_input(&chunk_over) {
        Err(TerminalInputEncodeError::PayloadTooLarge { kind, max, actual }) => {
            assert_eq!(kind, TerminalInputKind::PasteChunk);
            assert_eq!(max, MAX_PASTE_CHUNK_DATA_BYTES);
            assert_eq!(actual, MAX_PASTE_CHUNK_DATA_BYTES + 1);
        }
        other => panic!("expected chunk PayloadTooLarge, got {other:?}"),
    }
}

#[test]
fn decode_rejects_resize_body_that_is_not_twelve_bytes() {
    let mut bytes = vec![
        TERMINAL_INPUT_SCHEME_VERSION,
        TerminalInputKind::Resize.as_byte(),
    ];
    bytes.extend_from_slice(&3u16.to_be_bytes());
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.extend_from_slice(&[0, 24, 80]);
    assert_eq!(bytes.len(), INPUT_HEADER_BYTES + 3);
    let frame =
        botster_terminal_protocol::TerminalInputFrame::from_bytes(&bytes).expect("header is valid");
    let error = decode_terminal_input(&frame).expect_err("resize body must be 12");
    assert!(error.to_string().contains("12"), "{error}");
}
