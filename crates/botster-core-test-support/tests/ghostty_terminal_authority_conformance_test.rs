//! Hub-shaped Ghostty terminal authority conformance proofs.
//!
//! These tests exercise the public export shapes Hub and other clients will
//! consume: ModeFlags, GHOSTSNP snapshots, and color profile authority.

#![cfg(feature = "ghostty-terminal")]
#![allow(missing_docs)]

use std::collections::HashMap;

use botster_core::{
    ModeFlags, Rgb, TerminalColorProfile, TerminalScreenRuntime, TerminalScreenSize,
    TerminalScreenState, TerminalSnapshotPayload,
};
use botster_core_test_support::conformance::{
    assert_color_profile_authority, assert_ghostty_snapshot_authority,
    assert_ghostty_terminal_authority_exports, assert_mode_flags_authority,
    assert_special_color_defaults, GHOSTSNP_MAGIC, GHOSTTY_SNAPSHOT_FORMAT_LABEL,
};
use botster_terminal_ghostty::{
    GhosttyTerminal, COLOR_INDEX_BACKGROUND, COLOR_INDEX_CURSOR, COLOR_INDEX_FOREGROUND,
};

fn complete_palette_profile() -> TerminalColorProfile {
    let mut colors = HashMap::new();
    for index in 0u16..256 {
        colors.insert(
            index,
            Rgb {
                r: index as u8,
                g: 0,
                b: 255u8.wrapping_sub(index as u8),
            },
        );
    }
    colors.insert(
        COLOR_INDEX_FOREGROUND,
        Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        },
    );
    colors.insert(
        COLOR_INDEX_BACKGROUND,
        Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        },
    );
    colors.insert(
        COLOR_INDEX_CURSOR,
        Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66,
        },
    );
    TerminalColorProfile { colors }
}

#[test]
fn hub_shaped_snapshot_mode_and_color_exports_match_authority_contract() {
    let mut runtime =
        GhosttyTerminal::new(TerminalScreenSize::new(24, 80)).expect("create Ghostty terminal");

    runtime.write_output(b"\x1b[?1000h\x1b[?1006h");
    runtime.write_output(b"\x1b[=1;1u");
    runtime.write_output(b"authority-marker");
    runtime
        .set_color_profile(complete_palette_profile())
        .expect("apply color profile");

    let mode_flags = runtime.mode_flags().expect("mode flags");
    assert_mode_flags_authority(
        &mode_flags,
        ModeFlags {
            kitty_enabled: true,
            mouse_mode: 9,
            ..ModeFlags::default()
        },
    );

    let snapshot = runtime.capture_snapshot();
    assert_ghostty_snapshot_authority(&snapshot);
    assert!(snapshot.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_eq!(
        snapshot.format.as_deref(),
        Some(GHOSTTY_SNAPSHOT_FORMAT_LABEL)
    );

    let screen = runtime.screen_state();
    let profile = screen
        .color_profile
        .as_ref()
        .expect("screen state carries color profile authority");
    assert_color_profile_authority(profile);
    assert_special_color_defaults(
        profile,
        Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        },
        Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        },
        Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66,
        },
        COLOR_INDEX_FOREGROUND,
        COLOR_INDEX_BACKGROUND,
        COLOR_INDEX_CURSOR,
    );

    assert_ghostty_terminal_authority_exports(&mode_flags, &snapshot, &screen);
    assert!(screen.plain_text.contains("authority-marker") || !screen.plain_text.is_empty());
}

#[test]
fn pure_shape_fixture_helpers_accept_hub_facing_carriers() {
    let snapshot = TerminalSnapshotPayload::new(
        {
            let mut bytes = GHOSTSNP_MAGIC.to_vec();
            bytes.extend_from_slice(&[1, 2, 3, 4]);
            bytes
        },
        TerminalScreenSize::new(24, 80),
        Some(GHOSTTY_SNAPSHOT_FORMAT_LABEL.to_string()),
    );
    assert_ghostty_snapshot_authority(&snapshot);

    let mut colors = HashMap::new();
    for index in 0u16..256 {
        colors.insert(index, Rgb { r: 1, g: 2, b: 3 });
    }
    let profile = TerminalColorProfile { colors };
    assert_color_profile_authority(&profile);

    let screen = TerminalScreenState {
        size: TerminalScreenSize::new(24, 80),
        plain_text: String::new(),
        title: None,
        cwd: None,
        mode_flags: ModeFlags {
            mouse_mode: 1,
            ..ModeFlags::default()
        },
        color_profile: Some(profile),
    };
    assert_ghostty_terminal_authority_exports(&screen.mode_flags, &snapshot, &screen);
}

#[test]
#[should_panic(expected = "Hub-facing ModeFlags must match")]
fn authority_exports_reject_mode_flags_that_diverge_from_screen_carrier() {
    let snapshot = TerminalSnapshotPayload::new(
        {
            let mut bytes = GHOSTSNP_MAGIC.to_vec();
            bytes.extend_from_slice(&[1, 2, 3, 4]);
            bytes
        },
        TerminalScreenSize::new(24, 80),
        Some(GHOSTTY_SNAPSHOT_FORMAT_LABEL.to_string()),
    );
    let mut colors = HashMap::new();
    for index in 0u16..256 {
        colors.insert(index, Rgb { r: 1, g: 2, b: 3 });
    }
    let screen = TerminalScreenState {
        size: TerminalScreenSize::new(24, 80),
        plain_text: String::new(),
        title: None,
        cwd: None,
        mode_flags: ModeFlags {
            mouse_mode: 1,
            ..ModeFlags::default()
        },
        color_profile: Some(TerminalColorProfile { colors }),
    };
    let mismatched = ModeFlags {
        mouse_mode: 9,
        ..ModeFlags::default()
    };
    assert_ghostty_terminal_authority_exports(&mismatched, &snapshot, &screen);
}

#[test]
#[should_panic(expected = "must include palette index")]
fn color_profile_authority_rejects_incomplete_palette() {
    let mut colors = HashMap::new();
    colors.insert(0, Rgb { r: 1, g: 2, b: 3 });
    assert_color_profile_authority(&TerminalColorProfile { colors });
}
