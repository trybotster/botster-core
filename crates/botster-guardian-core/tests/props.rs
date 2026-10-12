//! Decoder fuzzing for the guardian wire (plan 8; Core AD-6, SV-9).
use botster_guardian_core::wire::{Command, LogChunk};

#[test]
fn guardian_command_decoder() {
    bolero::check!().with_iterations(256).for_each(|bytes| {
        if let Ok(command) = Command::decode(bytes) {
            let encoded = serde_json::to_vec(&command).unwrap();
            assert_eq!(Command::decode(&encoded).unwrap(), command);
        }
    });
}

#[test]
fn guardian_log_decoder() {
    bolero::check!().with_iterations(256).for_each(|bytes| {
        if let Some(chunk) = LogChunk::decode(bytes) {
            assert_eq!(chunk.encode(), bytes);
        }
    });
}

// This test binary was built in the worktree where it runs (bug catalog B13).
botster_test_process::built_in_test!();
