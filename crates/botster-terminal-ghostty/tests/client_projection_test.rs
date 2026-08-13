//! Client projection proofs: Hub-shaped install, live apply, scroll, OSC, and
//! a non-owning Ratatui-shaped public consumer gate.

#![cfg(feature = "libghostty-vt")]

use botster_core::contract::terminal_screen::TerminalScreenSize;
use botster_core::Rgb;
use botster_terminal_ghostty::{
    CursorStyle, GhosttyAdapterConfig, GhosttyClientProjection, GhosttySnapshotDecodeProgress,
    GhosttySnapshotFrameKind, GhosttyTerminal, ProjectedWide, ScrollOp, COLOR_INDEX_BACKGROUND,
    COLOR_INDEX_CURSOR, COLOR_INDEX_FOREGROUND, GHOSTSNP_MAGIC,
};

/// Local Ratatui-shaped cell: mirrors buffer cell fields without botster-tui.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RatatuiCellLike {
    symbol: String,
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
    bold: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    faint: bool,
    strikethrough: bool,
}

fn map_projection_to_ratatui(
    projection: &botster_terminal_ghostty::ViewportProjection,
) -> Vec<RatatuiCellLike> {
    projection
        .cells
        .iter()
        .map(|cell| RatatuiCellLike {
            symbol: cell.grapheme.clone(),
            fg: (cell.fg.r, cell.fg.g, cell.fg.b),
            bg: (cell.bg.r, cell.bg.g, cell.bg.b),
            bold: cell.bold,
            italic: cell.italic,
            underline: cell.underline,
            inverse: cell.inverse,
            faint: cell.faint,
            strikethrough: cell.strikethrough,
        })
        .collect()
}

fn producer(size: TerminalScreenSize) -> GhosttyTerminal {
    GhosttyTerminal::with_config(
        size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(512 * 1024),
    )
    .expect("producer GhosttyTerminal")
}

fn client(size: TerminalScreenSize) -> GhosttyClientProjection {
    GhosttyClientProjection::with_config(
        size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(512 * 1024),
    )
    .expect("client projection")
}

fn export_ghostsnp(size: TerminalScreenSize, bytes: &[u8]) -> Vec<u8> {
    let mut source = producer(size);
    source.write_output_bytes(bytes);
    source
        .export_snapshot_bytes()
        .expect("export GHOSTSNP producer bytes")
}

fn viewport_contains(
    projection: &botster_terminal_ghostty::ViewportProjection,
    needle: &str,
) -> bool {
    let mut row = String::new();
    for (i, cell) in projection.cells.iter().enumerate() {
        if i > 0 && i % projection.cols as usize == 0 {
            if row.contains(needle) {
                return true;
            }
            row.clear();
        }
        row.push_str(&cell.grapheme);
    }
    row.contains(needle)
}

fn find_cell_with_grapheme<'a>(
    projection: &'a botster_terminal_ghostty::ViewportProjection,
    needle: &str,
) -> Option<&'a botster_terminal_ghostty::ProjectedCell> {
    projection.cells.iter().find(|c| c.grapheme == needle)
}

#[test]
fn incremental_frames_restore_ready_then_one_page_steps_then_finish() {
    let size = TerminalScreenSize::new(2, 215);
    let mut source = producer(size);
    for index in 0..1000 {
        source.write_output_bytes(format!("history-{index:04}\r\n").as_bytes());
    }
    source.write_output_bytes(b"visible-ready-marker");

    let mut frames = Vec::new();
    let mut encode_returned = false;
    source
        .export_snapshot_frames(|frame| {
            assert!(!encode_returned, "frames must arrive before encode returns");
            frames.push(frame);
            true
        })
        .expect("stream one real Ghostty snapshot");
    encode_returned = true;
    assert!(encode_returned);
    assert_eq!(
        frames.first().map(|frame| frame.kind),
        Some(GhosttySnapshotFrameKind::Ready)
    );
    assert_eq!(
        frames.last().map(|frame| frame.kind),
        Some(GhosttySnapshotFrameKind::Finish)
    );
    assert!(frames
        .iter()
        .any(|frame| frame.kind == GhosttySnapshotFrameKind::History));

    let mut client = client(TerminalScreenSize::new(24, 80));
    let ready = frames.remove(0);
    assert_eq!(
        client
            .install_ghostsnp_ready(ready.bytes)
            .expect("decode through READY"),
        GhosttySnapshotDecodeProgress::Ready
    );
    assert!(viewport_contains(
        &client.project_viewport().expect("paint at READY"),
        "visible-ready-marker"
    ));

    for frame in frames {
        let progress = client
            .apply_ghostsnp_history(frame.bytes)
            .expect("decode one transport frame");
        match frame.kind {
            GhosttySnapshotFrameKind::History => {
                assert_eq!(progress, GhosttySnapshotDecodeProgress::History)
            }
            GhosttySnapshotFrameKind::Finish => {
                assert_eq!(progress, GhosttySnapshotDecodeProgress::Finish)
            }
            GhosttySnapshotFrameKind::Ready => panic!("READY must appear once"),
        }
    }
    assert!(!client.snapshot_history_pending());
}

#[test]
fn blank_incremental_snapshot_is_ready_then_finish() {
    let size = TerminalScreenSize::new(24, 80);
    let source = producer(size);
    let mut frames = Vec::new();
    source
        .export_snapshot_frames(|frame| {
            frames.push(frame);
            true
        })
        .expect("stream blank snapshot");
    assert_eq!(
        frames.iter().map(|frame| frame.kind).collect::<Vec<_>>(),
        vec![
            GhosttySnapshotFrameKind::Ready,
            GhosttySnapshotFrameKind::Finish
        ]
    );

    let mut client = client(size);
    assert_eq!(
        client
            .install_ghostsnp_ready(frames.remove(0).bytes)
            .expect("blank READY"),
        GhosttySnapshotDecodeProgress::Ready
    );
    assert_eq!(
        client
            .apply_ghostsnp_history(frames.remove(0).bytes)
            .expect("blank FINISH returns NO_VALUE"),
        GhosttySnapshotDecodeProgress::Finish
    );
}

#[test]
fn abort_incremental_history_retains_ready_terminal_and_allows_resize() {
    let size = TerminalScreenSize::new(24, 80);
    let mut source = producer(size);
    source.write_output_bytes(b"ready-state-survives");
    let mut frames = Vec::new();
    source
        .export_snapshot_frames(|frame| {
            frames.push(frame);
            true
        })
        .expect("stream snapshot");

    let mut client = client(size);
    client
        .install_ghostsnp_ready(frames.remove(0).bytes)
        .expect("install READY");
    assert!(client.abort_ghostsnp_history());
    assert!(!client.snapshot_history_pending());
    assert!(viewport_contains(
        &client.project_viewport().expect("paint retained READY"),
        "ready-state-survives"
    ));
    client
        .resize(TerminalScreenSize::new(30, 100))
        .expect("resize after abort");
    assert_eq!(client.dimensions(), TerminalScreenSize::new(30, 100));
}

#[test]
fn hub_shaped_install_accepts_bytes_only_and_derives_dimensions() {
    // TerminalScreenSize::new(rows, cols)
    let size = TerminalScreenSize::new(10, 24);
    let ghostsnp = export_ghostsnp(size, b"hub install marker");
    assert!(ghostsnp.starts_with(GHOSTSNP_MAGIC));

    // Hub 89dae7e shape: only decoded opaque history bytes.
    let hub_decoded_bytes = ghostsnp.as_slice();

    let mut client = client(TerminalScreenSize::new(24, 80));
    client
        .install_ghostsnp(hub_decoded_bytes)
        .expect("install GHOSTSNP bytes");

    assert_eq!(client.dimensions(), size);
    let projection = client.project_viewport().expect("project");
    assert_eq!(projection.cols, size.cols);
    assert_eq!(projection.rows, size.rows);
    assert_eq!(projection.cells.len(), (size.cols * size.rows) as usize);
    assert!(viewport_contains(&projection, "hub install marker"));
}

#[test]
fn install_fails_closed_on_empty_magic_garbage_and_scrollback_like_body() {
    let size = TerminalScreenSize::new(5, 20);
    let mut client = client(size);
    client.apply_terminal_output(b"keep me");
    let before = client
        .project_viewport()
        .expect("project before")
        .cells
        .iter()
        .map(|c| c.grapheme.clone())
        .collect::<String>();

    assert!(client.install_ghostsnp(&[]).is_err());
    assert!(client.install_ghostsnp(b"NOTGHOST").is_err());
    assert!(client.install_ghostsnp(b"GHOSTSNPx").is_err());
    // Scrollback-like non-GHOSTSNP body must never install.
    assert!(client
        .install_ghostsnp(b"scrollback-history-not-ghostsnp")
        .is_err());

    let after = client
        .project_viewport()
        .expect("project after failed install")
        .cells
        .iter()
        .map(|c| c.grapheme.clone())
        .collect::<String>();
    assert_eq!(before, after);
    assert!(after.contains("keep me"));
}

#[test]
fn apply_live_updates_projected_graphemes_after_install() {
    let size = TerminalScreenSize::new(6, 40);
    let ghostsnp = export_ghostsnp(size, b"seed");
    let mut client = client(size);
    client.install_ghostsnp(&ghostsnp).expect("install");
    client.apply_terminal_output(b"\r\nlive-after-install");
    let projection = client.project_viewport().expect("project");
    assert!(
        viewport_contains(&projection, "live-after-install"),
        "projected text={:?}",
        projection
            .cells
            .iter()
            .map(|c| c.grapheme.as_str())
            .collect::<String>()
    );
}

#[test]
fn projects_grapheme_wide_resolved_rgb_attributes_and_cursor() {
    let size = TerminalScreenSize::new(8, 40);
    let mut client = client(size);
    // Bold truecolor green "Hi", inverse "X", truecolor rgb, wide fullwidth "Ａ".
    client.apply_terminal_output(
        b"\x1b[1;38;2;0;128;0mHi\x1b[0m \x1b[7mX\x1b[0m \x1b[38;2;10;20;30mrgb\x1b[0m \xef\xbc\xa1",
    );
    // DECSCUSR 5 = blinking bar; style still projects as Bar.
    client.apply_terminal_output(b"\x1b[5 q");
    // Move cursor to column 3 of row 0 (1-based CUP).
    client.apply_terminal_output(b"\x1b[1;3H");

    let projection = client.project_viewport().expect("project");
    assert!(projection.cells.iter().any(|c| c.grapheme == "H"));
    assert!(projection.cells.iter().any(|c| c.grapheme == "i"));

    let h = find_cell_with_grapheme(&projection, "H").expect("H cell");
    assert!(h.bold, "bold should round-trip");
    assert_eq!(
        h.fg,
        Rgb {
            r: 0,
            g: 0x80,
            b: 0
        },
        "truecolor green resolved RGB"
    );

    let inv = find_cell_with_grapheme(&projection, "X").expect("inverse cell");
    assert!(inv.inverse, "inverse should round-trip");

    let rgb_cell = find_cell_with_grapheme(&projection, "r").expect("rgb cell");
    assert_eq!(
        rgb_cell.fg,
        Rgb {
            r: 10,
            g: 20,
            b: 30
        }
    );

    let wide_kinds: Vec<_> = projection
        .cells
        .iter()
        .map(|c| c.wide)
        .filter(|w| *w != ProjectedWide::Narrow)
        .collect();
    assert!(
        !wide_kinds.is_empty(),
        "wide fullwidth character should report non-Narrow wide kind, got all Narrow"
    );

    assert!(projection.cursor.visible);
    assert!(projection.cursor.in_viewport);
    assert_eq!(projection.cursor.x, 2);
    assert_eq!(projection.cursor.y, 0);
    assert_eq!(projection.cursor.style, CursorStyle::Bar);
}

#[test]
fn default_new_client_preserves_imported_scrollback_history() {
    // Regression for review finding: GhosttyClientProjection::new uses a zero
    // live scrollback budget. install must not re-apply that zero limit after
    // decode, or retained history from the GHOSTSNP is erased.
    let size = TerminalScreenSize::new(5, 40);
    let mut producer = producer(size);
    producer.write_output_bytes(b"TOP_MARKER\r\n");
    for i in 0..20 {
        producer.write_output_bytes(format!("mid line {i}\r\n").as_bytes());
    }
    producer.write_output_bytes(b"JUST_ABOVE_VIEWPORT\r\n");
    for i in 0..4 {
        producer.write_output_bytes(format!("live edge {i}\r\n").as_bytes());
    }
    producer.write_output_bytes(b"BOTTOM_LIVE");
    let ghostsnp = producer
        .export_snapshot_bytes()
        .expect("export retained-history GHOSTSNP");

    let mut client = GhosttyClientProjection::new(size).expect("default client");
    client
        .install_ghostsnp(&ghostsnp)
        .expect("install into default client");

    let bar = client.scrollbar().expect("scrollbar after install");
    assert!(
        bar.total > bar.len,
        "default install must retain history: total={} len={}",
        bar.total,
        bar.len
    );

    client.scroll(ScrollOp::Top);
    let top = client.project_viewport().expect("top");
    assert!(
        viewport_contains(&top, "TOP_MARKER"),
        "Top must surface imported history on default client"
    );

    client.scroll(ScrollOp::Bottom);
    let bottom = client.project_viewport().expect("bottom");
    assert!(
        viewport_contains(&bottom, "BOTTOM_LIVE"),
        "Bottom must return to live edge on default client"
    );

    let before = client.scrollbar().expect("before delta").offset;
    client.scroll(ScrollOp::Delta(-4));
    let after = client.scrollbar().expect("after delta").offset;
    assert_ne!(
        after, before,
        "Delta must move viewport after default-client install"
    );

    // Inherited producer budget remains after default install: later live
    // output continues to grow retained history under the decoded snapshot
    // policy (no client-side override when max_scrollback is 0).
    client.scroll(ScrollOp::Bottom);
    let total_before_live = client.scrollbar().expect("before live").total;
    for i in 0..12 {
        client.apply_terminal_output(format!("post-import growth {i}\r\n").as_bytes());
    }
    client.apply_terminal_output(b"POST_IMPORT_LIVE");
    let bar_after_live = client.scrollbar().expect("after live growth");
    assert!(
        bar_after_live.total > total_before_live,
        "default client must inherit producer scrollback budget so later output grows history: before={total_before_live} after={}",
        bar_after_live.total
    );
    let live = client
        .project_viewport()
        .expect("post-import live projection");
    assert!(
        viewport_contains(&live, "POST_IMPORT_LIVE"),
        "later apply_terminal_output must project on default client"
    );
    client.scroll(ScrollOp::Top);
    let top_after_growth = client.project_viewport().expect("top after growth");
    assert!(
        viewport_contains(&top_after_growth, "TOP_MARKER"),
        "inherited budget must keep pre-import history while growing"
    );
}

#[test]
fn full_scrollback_navigation_top_bottom_and_delta() {
    let size = TerminalScreenSize::new(5, 40);
    let mut client = client(size);

    // Distinct markers at different history depths.
    client.apply_terminal_output(b"TOP_MARKER\r\n");
    for i in 0..20 {
        client.apply_terminal_output(format!("mid line {i}\r\n").as_bytes());
    }
    client.apply_terminal_output(b"JUST_ABOVE_VIEWPORT\r\n");
    for i in 0..4 {
        client.apply_terminal_output(format!("live edge {i}\r\n").as_bytes());
    }
    client.apply_terminal_output(b"BOTTOM_LIVE");

    let bar = client.scrollbar().expect("scrollbar");
    assert!(
        bar.total > bar.len,
        "retained history should exceed viewport: total={} len={}",
        bar.total,
        bar.len
    );

    client.scroll(ScrollOp::Top);
    let top_bar = client.scrollbar().expect("top scrollbar");
    let top = client.project_viewport().expect("top projection");
    assert_eq!(top_bar.offset, 0, "Top should pin offset at 0");
    assert!(
        viewport_contains(&top, "TOP_MARKER"),
        "ScrollOp::Top must surface far history marker; text={:?} bar={:?}",
        top.cells
            .iter()
            .map(|c| c.grapheme.as_str())
            .collect::<String>(),
        top_bar
    );

    client.scroll(ScrollOp::Bottom);
    let bottom = client.project_viewport().expect("bottom projection");
    assert!(
        viewport_contains(&bottom, "BOTTOM_LIVE"),
        "ScrollOp::Bottom must return to live edge"
    );

    // Delta up into history from bottom, then further up.
    client.scroll(ScrollOp::Delta(-3));
    let mid = client.project_viewport().expect("delta mid");
    let mid_bar = client.scrollbar().expect("mid scrollbar");
    assert!(
        mid_bar.offset < mid_bar.total.saturating_sub(mid_bar.len),
        "after Delta(-3) should not be pinned at absolute bottom"
    );
    // A second delta should move again (not a one-shot scroll-up).
    let offset_after_first = mid_bar.offset;
    client.scroll(ScrollOp::Delta(-5));
    let after_second = client.scrollbar().expect("second delta scrollbar");
    assert_ne!(
        after_second.offset, offset_after_first,
        "ScrollOp::Delta must move between positions"
    );
    let _ = mid;
}

#[test]
fn real_osc_mutates_palette_and_special_colors() {
    let size = TerminalScreenSize::new(4, 20);
    let mut client = client(size);

    // Real OSC sequences only (not set_color_profile).
    // OSC 4 ; 1 ; rgb:ffff/0000/0000  palette index 1 = red
    client.apply_terminal_output(b"\x1b]4;1;rgb:ffff/0000/0000\x1b\\");
    // OSC 10 / 11 / 12 specials
    client.apply_terminal_output(b"\x1b]10;rgb:1111/2222/3333\x1b\\");
    client.apply_terminal_output(b"\x1b]11;rgb:4444/5555/6666\x1b\\");
    client.apply_terminal_output(b"\x1b]12;rgb:7777/8888/9999\x1b\\");

    let profile = client.color_profile().expect("color profile after OSC");
    assert_eq!(
        profile.colors.get(&1),
        Some(&Rgb {
            r: 0xff,
            g: 0x00,
            b: 0x00
        }),
        "OSC 4 palette entry"
    );
    assert_eq!(
        profile.colors.get(&COLOR_INDEX_FOREGROUND),
        Some(&Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        })
    );
    assert_eq!(
        profile.colors.get(&COLOR_INDEX_BACKGROUND),
        Some(&Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66
        })
    );
    assert_eq!(
        profile.colors.get(&COLOR_INDEX_CURSOR),
        Some(&Rgb {
            r: 0x77,
            g: 0x88,
            b: 0x99
        })
    );
}

#[test]
fn client_projection_does_not_install_write_pty_product_path() {
    // GhosttyClientProjection has no drain_pty_writes; constructing and
    // feeding OSC queries must not require a write_pty product API.
    let mut client = client(TerminalScreenSize::new(3, 20));
    client.apply_terminal_output(b"\x1b]10;?\x1b\\");
    let projection = client.project_viewport().expect("still projects");
    assert_eq!(
        projection.cells.len(),
        (projection.cols * projection.rows) as usize
    );
}

#[test]
fn downstream_shaped_ratatui_mapper_consumes_public_projection() {
    let size = TerminalScreenSize::new(6, 40);
    let ghostsnp = export_ghostsnp(size, b"consumer seed");
    let mut client = client(size);
    client
        .install_ghostsnp(&ghostsnp)
        .expect("Hub-shaped install");
    client.apply_terminal_output(b"\r\n\x1b[1;38;2;128;0;0mMAPME\x1b[0m");

    let projection = client.project_viewport().expect("public project");
    let mapped = map_projection_to_ratatui(&projection);

    assert_eq!(mapped.len(), projection.cells.len());
    let mapped_text: String = mapped.iter().map(|c| c.symbol.as_str()).collect();
    assert!(
        mapped_text.contains("MAPME"),
        "mapped Ratatui-shaped buffer must carry text"
    );
    let mapme = mapped
        .iter()
        .find(|c| c.symbol == "M" && c.bold)
        .expect("bold M cell");
    assert_eq!(
        mapme.fg,
        (0x80, 0x00, 0x00),
        "truecolor red maps to RGB tuple"
    );
    assert!(mapme.bold, "style attribute maps into consumer cell");
}
