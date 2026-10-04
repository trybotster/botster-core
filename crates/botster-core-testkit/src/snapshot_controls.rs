//! Capture controls for Core ST-6 and ST-6b. Libghostty supplies all terminal values.
//! The dispatch adapter reads every Core capture page before it calls these functions.

use botster_core_contract::prelude::CellPx;
use botster_terminal_ghostty::{
    snapshot_format, CellAttributes, Error, History, SnapshotDecodeError, SnapshotError, Terminal,
};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    Library(Error),
    Decode(SnapshotDecodeError),
    Snapshot(SnapshotError),
    InvalidEnvelope,
}

impl From<Error> for CaptureError {
    fn from(error: Error) -> Self {
        Self::Library(error)
    }
}

/// The bytes from every `read_page`, in page order, and the source configuration.
/// The adapter must use Core's pages. A snapshot of a separate oracle is no substitute.
pub struct CapturePages<'a> {
    pub bytes: &'a [u8],
    pub history: History,
    pub cell_px: Option<CellPx>,
}

impl CapturePages<'_> {
    pub fn restore(&self) -> Result<Terminal, CaptureError> {
        Terminal::from_snapshot(self.bytes, self.history, self.cell_px)
            .map_err(CaptureError::Decode)
    }
}

/// Compare the capture with the actual session model at the capture revision.
/// Each result checks its own ST-6b item. A failure in another item does not change that result.
pub fn oracle_restore(
    model: &mut Terminal,
    pages: &CapturePages<'_>,
) -> Result<Value, CaptureError> {
    let mut restored = pages.restore()?;
    let model_modes = model.modes();
    let restored_modes = restored.modes();
    let screen_equal = model.screen_text(true)? == restored.screen_text(true)?
        && cells(model)? == cells(&restored)?;
    let hyperlinks_equal = hyperlinks(model)? == hyperlinks(&restored)?;
    let cursor_equal = model.cursor() == restored.cursor()
        && model.cursor_appearance()? == restored.cursor_appearance()?;
    Ok(json!({
        "palette_equal": model.colors()? == restored.colors()?,
        "cursor_equal": cursor_equal,
        "hyperlinks_equal": hyperlinks_equal,
        "modes_equal": model_modes == restored_modes,
        "kitty_flags_equal": model_modes.kitty_flags == restored_modes.kitty_flags,
        "size_equal": model.rows() == restored.rows() && model.cols() == restored.cols()
            && model_modes.alt_screen == restored_modes.alt_screen,
        "title_cwd_equal": model.title() == restored.title() && model.cwd() == restored.cwd(),
        "screen_equal": screen_equal,
    }))
}

fn cells(model: &Terminal) -> Result<Vec<Vec<CellAttributes>>, Error> {
    let mut rows = Vec::new();
    for row in 0..u32::MAX {
        match model.cell_attributes(row, 0, true) {
            Err(Error::InvalidValue) => break,
            Err(error) => return Err(error),
            Ok(first) => {
                let mut cells = vec![first];
                for col in 1..u32::from(model.cols()) {
                    cells.push(model.cell_attributes(row, col, true)?);
                }
                rows.push(cells);
            }
        }
    }
    Ok(rows)
}

fn hyperlinks(model: &Terminal) -> Result<Vec<Vec<String>>, Error> {
    let mut rows = Vec::new();
    for row in 0..u32::MAX {
        match model.cell_hyperlink_uri(row, 0, true) {
            Err(Error::InvalidValue) => break,
            Err(error) => return Err(error),
            Ok(first) => {
                let mut cells = vec![first];
                for col in 1..u32::from(model.cols()) {
                    cells.push(model.cell_hyperlink_uri(row, col, true)?);
                }
                rows.push(cells);
            }
        }
    }
    Ok(rows)
}

/// Restore Core's pages and apply all output consumed after the capture revision, in order.
/// The native snapshot comparison includes pending parser input and other state that changes later output.
pub fn oracle_resume(
    model: &Terminal,
    pages: &CapturePages<'_>,
    consumed_after: &[&[u8]],
) -> Result<Value, CaptureError> {
    let mut restored = pages.restore()?;
    for output in consumed_after {
        restored.vt_write(output);
    }
    let expected = model.snapshot().map_err(CaptureError::Snapshot)?;
    let actual = restored.snapshot().map_err(CaptureError::Snapshot)?;
    Ok(json!({"equal": expected == actual}))
}

/// Read the storage limit on both actual instances (ST-6b graphics, revised lead ruling).
/// Both constructors set the limit to zero before input. Libghostty enforces that limit, so neither can store images.
/// Constructor tests verify the invariant with native lookups. This control requires no image parser or stimulus record.
pub fn oracle_graphics(model: &Terminal, pages: &CapturePages<'_>) -> Result<Value, CaptureError> {
    let restored = pages.restore()?;
    Ok(
        json!({"model_holds_images": model.image_storage_limit()? != 0,
        "images_in_baseline": restored.image_storage_limit()? != 0}),
    )
}

/// Change only the protocol envelope version, then ask the actual native decoder to refuse it (ST-6).
/// The supported version comes from a library-generated snapshot. The changed envelope is decoder input.
pub fn snapshot_unsupported_version(pages: &CapturePages<'_>) -> Result<Value, CaptureError> {
    let supported =
        u16::try_from(snapshot_format().version).map_err(|_| CaptureError::InvalidEnvelope)?;
    let version = supported.wrapping_add(1);
    let mut bytes = pages.bytes.to_vec();
    let envelope = bytes.get_mut(8..10).ok_or(CaptureError::InvalidEnvelope)?;
    envelope.copy_from_slice(&version.to_le_bytes());
    match Terminal::from_snapshot(&bytes, pages.history, pages.cell_px) {
        Err(SnapshotDecodeError::UnsupportedVersion { version: refused }) if refused == version => {
            Ok(
                json!({"version": version, "supported": false, "refused": true, "error": "UnsupportedVersion"}),
            )
        }
        Err(error) => Err(CaptureError::Decode(error)),
        Ok(_) => {
            Ok(json!({"version": version, "supported": false, "refused": false, "error": null}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_core_contract::prelude::Size;

    fn model() -> Terminal {
        Terminal::new(
            &Size {
                rows: 3,
                cols: 12,
                cell_px: None,
            },
            History::On,
        )
        .unwrap()
    }

    fn pages(bytes: &[u8]) -> CapturePages<'_> {
        CapturePages {
            bytes,
            history: History::On,
            cell_px: None,
        }
    }

    #[test]
    fn restore_and_resume_use_library_generated_pages() {
        let mut source = model();
        source.vt_write(b"\x1b[1mA\x1b]8;;https://example.test\x1b\\B\x1b]8;;\x1b\\\x1b[5 q\x1b[>15u\x1b[?1003h\x1b]2;title\x1b\\");
        let bytes = source.snapshot().unwrap();
        let result = oracle_restore(&mut source, &pages(&bytes)).unwrap();
        assert!(result
            .as_object()
            .unwrap()
            .values()
            .all(|value| value == true));
        let suffix = b"\x1b[0m suffix";
        source.vt_write(suffix);
        assert_eq!(
            oracle_resume(&source, &pages(&bytes), &[suffix]).unwrap(),
            json!({"equal": true})
        );
        assert_eq!(
            oracle_resume(&source, &pages(&bytes), &[]).unwrap(),
            json!({"equal": false})
        );
    }

    #[test]
    fn restore_reports_each_group_independently() {
        let mut source = model();
        let bytes = source.snapshot().unwrap();
        source.vt_write(b"\x1b]4;2;rgb:12/34/56\x1b\\");
        let result = oracle_restore(&mut source, &pages(&bytes)).unwrap();
        for (key, value) in result.as_object().unwrap() {
            assert_eq!(value, &(key != "palette_equal"), "{key}");
        }
        let bytes = source.snapshot().unwrap();
        let before_modes = source.modes();
        source.vt_write(b"\x1b[5 q");
        let result = oracle_restore(&mut source, &pages(&bytes)).unwrap();
        for (key, value) in result.as_object().unwrap() {
            let expected = match key.as_str() {
                "cursor_equal" => false,
                "modes_equal" => source.modes() == before_modes,
                _ => true,
            };
            assert_eq!(value, &expected, "{key}");
        }
        let bytes = source.snapshot().unwrap();
        source.vt_write(b"\x1b]2;changed\x1b\\");
        let result = oracle_restore(&mut source, &pages(&bytes)).unwrap();
        for (key, value) in result.as_object().unwrap() {
            assert_eq!(value, &(key != "title_cwd_equal"), "{key}");
        }
    }

    #[test]
    fn history_attributes_and_hyperlinks_are_compared() {
        let mut source = model();
        source.vt_write(
            b"\x1b[1m\x1b]8;;https://example.test\x1b\\H\x1b]8;;\x1b\\\x1b[0m\r\n\r\n\r\n\r\n",
        );
        let bytes = source.snapshot().unwrap();
        let mut empty = model();
        let result = oracle_restore(&mut empty, &pages(&bytes)).unwrap();
        assert_eq!(result["screen_equal"], false);
        assert_eq!(result["hyperlinks_equal"], false);
        assert_eq!(result["palette_equal"], true);
    }

    #[test]
    fn resume_keeps_pending_input_at_each_cut() {
        let input = b"A\x1b[31mB\x1b]2;title\x1b\\C";
        for offset in 0..=input.len() {
            let mut source = model();
            source.vt_write(&input[..offset]);
            let bytes = source.snapshot().unwrap();
            source.vt_write(&input[offset..]);
            assert_eq!(
                oracle_resume(&source, &pages(&bytes), &[&input[offset..]]).unwrap()["equal"],
                true
            );
        }
    }

    #[test]
    fn graphics_read_and_version_refusal_use_the_decoded_instance() {
        let mut source = model();
        source.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=7;AAAA\x1b\\");
        let bytes = source.snapshot().unwrap();
        assert_eq!(
            oracle_graphics(&source, &pages(&bytes)).unwrap(),
            json!({"model_holds_images": false, "images_in_baseline": false})
        );
        let result = snapshot_unsupported_version(&pages(&bytes)).unwrap();
        assert_eq!(result["refused"], true);
        assert_eq!(result["supported"], false);
        assert_ne!(result["version"], snapshot_format().version);
        assert_eq!(result["error"], "UnsupportedVersion");
        assert_eq!(
            snapshot_unsupported_version(&pages(&[])),
            Err(CaptureError::InvalidEnvelope)
        );
        assert!(matches!(
            oracle_graphics(&source, &pages(&[])),
            Err(CaptureError::Decode(_))
        ));
    }
}
