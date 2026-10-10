//! The snapshot of a terminal and the identity of the emulator. libghostty writes both; this crate copies the bytes.

use crate::{sys, Error, Terminal};

/// A failure of the snapshot decoder (Core ST-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotDecodeError {
    /// The library refused the envelope's version.
    UnsupportedVersion { version: u16 },
    /// The library refused the snapshot for another reason.
    Library(Error),
}

/// The replay input that libghostty retains for its pending parser state (Core ST-6b and A8-2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Continuation {
    /// The replay input. An empty buffer means that the parser is at ground.
    Retained(Vec<u8>),
    /// The library cannot export the pending input under the configured retention limit.
    Unavailable,
}

/// The largest unfinished parser input that the terminal keeps so that a snapshot can carry it (steward ruling A8-2).
/// A terminal whose parser holds more than this has no snapshot (`SnapshotError::ContinuationUnavailable`).
pub const CONTINUATION_LIMIT: usize = 1024 * 1024;

/// Why a terminal gave no snapshot. A snapshot is never partial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// The parser is not at ground and its unfinished input is not kept (it is longer than `CONTINUATION_LIMIT`). The
    /// worker reports `SnapshotTooLarge`.
    ContinuationUnavailable,
    /// The library failed in another way.
    Library(Error),
}

/// The snapshot format that this binding writes. The [GHOSTSNP spec](../GHOSTSNP.md) states its layout, continuation limit,
/// exclusions, refusal rules, and current paging status.
/// **The snapshot holds no Kitty graphics state** (no image and no
/// placement), and `snapshot_graphics` is absent from the worker's feature list. To keep that true after a restore, the
/// model turns image storage off before any write, and a restore must set the decoder's
/// `GHOSTTY_SNAPSHOT_DECODER_OPT_KITTY_IMAGE_STORAGE_LIMIT` to zero, so that the restored model ignores image sequences
/// as the original did (ST-6b).
/// (`Launched.formats`, ST-6). The name and the version are the envelope
/// that the library writes at the start of every snapshot (an 8-byte magic and a little-endian `u16` version), read
/// from a snapshot of a scratch terminal, so they are the library's own and never invented here.
pub fn snapshot_format() -> botster_core_contract::prelude::SnapshotFormat {
    let size = botster_core_contract::prelude::Size {
        rows: 1,
        cols: 1,
        cell_px: None,
    };
    let snapshot = crate::Terminal::new(&size, crate::History::Off)
        .ok()
        .and_then(|terminal| terminal.snapshot().ok())
        .unwrap_or_default();
    let name = snapshot
        .get(..8)
        .map(|magic| String::from_utf8_lossy(magic).into_owned())
        .unwrap_or_default();
    let version = snapshot
        .get(8..10)
        .map_or(0, |v| u32::from(u16::from_le_bytes([v[0], v[1]])));
    botster_core_contract::prelude::SnapshotFormat { name, version }
}

impl Terminal {
    /// Restore the library's snapshot with retained parser input and image storage disabled (Core ST-6b).
    ///
    /// `history` describes the source configuration. `cell_px` supplies geometry for callbacks and mouse encoding.
    /// Both arguments must match the source. The decoder restores the snapshot's scrollback policy and pixel dimensions.
    /// This method keeps the decoded cells, modes, size, colors, title, cwd and parser state unchanged.
    pub fn from_snapshot(
        bytes: &[u8],
        history: crate::History,
        cell_px: Option<botster_core_contract::prelude::CellPx>,
    ) -> Result<Self, SnapshotDecodeError> {
        let mut decoder = std::ptr::null_mut();
        // SAFETY: the slice stays live until the decoder is freed, and the output pointer is valid.
        crate::check(unsafe {
            sys::ghostty_snapshot_decoder_new_buf(
                std::ptr::null(),
                &mut decoder,
                bytes.as_ptr(),
                bytes.len(),
            )
        })
        .map_err(SnapshotDecodeError::Library)?;
        let result = decode(decoder);
        // SAFETY: the decoder is live and is freed once, after its last use.
        unsafe { sys::ghostty_snapshot_decoder_free(decoder) };
        let handle = result.map_err(|error| decode_error(bytes, error))?;
        let shared = std::ptr::NonNull::from(Box::leak(Box::<crate::events::Shared>::default()));
        let mut terminal = Self {
            handle,
            shared,
            history,
            cell_px,
            color_profile: None,
            ack_backlog_limit: crate::DEFAULT_ACK_BACKLOG_BYTES,
            _not_sync: std::marker::PhantomData,
        };
        terminal.set_clipboard_limit(crate::DEFAULT_CLIPBOARD_BYTES);
        terminal
            .register_callbacks()
            .map_err(SnapshotDecodeError::Library)?;
        terminal
            .set_query_request_limit_default()
            .map_err(SnapshotDecodeError::Library)?;
        terminal.set_cell_px(cell_px);
        Ok(terminal)
    }

    /// Export the exact replay input from libghostty (Core ST-6b). This method parses no terminal bytes.
    pub fn continuation(&self) -> Result<Continuation, Error> {
        let mut needed = 0;
        // SAFETY: the terminal is live. A null buffer asks for the size.
        let code = unsafe {
            sys::ghostty_terminal_continuation_buf(
                self.handle.as_ptr(),
                std::ptr::null_mut(),
                0,
                &mut needed,
            )
        };
        if code == sys::INVALID_VALUE {
            return Ok(Continuation::Unavailable);
        }
        if code != sys::OUT_OF_SPACE {
            return Err(Error::from_code(code));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(needed)
            .map_err(|_| Error::OutOfMemory)?;
        bytes.resize(needed, 0);
        // SAFETY: the buffer holds the required size. The terminal cannot change between these calls.
        crate::check(unsafe {
            sys::ghostty_terminal_continuation_buf(
                self.handle.as_ptr(),
                bytes.as_mut_ptr(),
                bytes.len(),
                &mut needed,
            )
        })?;
        bytes.truncate(needed);
        Ok(Continuation::Retained(bytes))
    }

    /// Read the library's failure flag after VT processing (Core ST-6b).
    /// Configured limits and malformed input do not set this flag.
    pub fn vt_processing_error(&self) -> Result<bool, Error> {
        let mut failed = false;
        // SAFETY: the terminal is live and the data key writes a bool.
        crate::check(unsafe {
            sys::ghostty_terminal_get(
                self.handle.as_ptr(),
                sys::data::VT_PROCESSING_ERROR,
                (&mut failed as *mut bool).cast(),
            )
        })?;
        Ok(failed)
    }

    pub(crate) fn set_continuation_limit(&self) -> Result<(), Error> {
        self.set_continuation_max_bytes(CONTINUATION_LIMIT)
    }

    /// Set libghostty's replay retention bound (Core ST-6b and A8-2).
    /// The default is `CONTINUATION_LIMIT`. Zero disables retention.
    /// The library can lose pending input if the new bound is below the retained size.
    pub fn set_continuation_max_bytes(&self, limit: usize) -> Result<(), Error> {
        // SAFETY: the handle is live, and the option takes a `size_t` pointer.
        crate::check(unsafe {
            sys::ghostty_terminal_set(
                self.handle.as_ptr(),
                sys::opt::CONTINUATION_MAX_BYTES,
                (&limit as *const usize).cast(),
            )
        })
    }

    /// The complete snapshot: screen, scrollback, modes and the unfinished parser input. It is the library's own
    /// format, and `Terminal::new` is not its reader.
    pub fn snapshot(&self) -> Result<Vec<u8>, SnapshotError> {
        self.snapshot_at_most(usize::MAX)
            .map(|snapshot| snapshot.expect("no snapshot is over usize::MAX"))
    }

    /// The snapshot when it is at most `max` bytes, else `None`. The library gives the size first, so a snapshot over `max`
    /// allocates nothing (Core DP-3: the frame limit is checked before allocation).
    pub fn snapshot_at_most(&self, max: usize) -> Result<Option<Vec<u8>>, SnapshotError> {
        let mut needed = 0usize;
        // SAFETY: a null buffer of length zero asks for the size, and `needed` is a valid out pointer.
        let code = unsafe {
            sys::ghostty_snapshot_encode_buf(
                self.handle.as_ptr(),
                std::ptr::null_mut(),
                0,
                &mut needed,
            )
        };
        match code {
            sys::OUT_OF_SPACE => {}
            sys::SUCCESS => return Ok(Some(Vec::new())),
            sys::INVALID_VALUE => return Err(SnapshotError::ContinuationUnavailable),
            other => return Err(SnapshotError::Library(Error::from_code(other))),
        }
        if needed > max {
            return Ok(None);
        }
        let mut out = Vec::new();
        out.try_reserve_exact(needed)
            .map_err(|_| SnapshotError::Library(Error::OutOfMemory))?;
        out.resize(needed, 0);
        let mut written = 0usize;
        // SAFETY: the buffer holds `needed` bytes, and `&self` means that nothing changes the terminal between the two
        // calls.
        let code = unsafe {
            sys::ghostty_snapshot_encode_buf(
                self.handle.as_ptr(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
            )
        };
        match code {
            sys::SUCCESS => {
                out.truncate(written);
                Ok(Some(out))
            }
            sys::INVALID_VALUE => Err(SnapshotError::ContinuationUnavailable),
            other => Err(SnapshotError::Library(Error::from_code(other))),
        }
    }
}

fn decode(decoder: sys::SnapshotDecoder) -> Result<std::ptr::NonNull<std::ffi::c_void>, Error> {
    let (limit, retain, images) = (CONTINUATION_LIMIT, true, 0u64);
    let mut handle = std::ptr::null_mut();
    // SAFETY: the decoder is live. Each option receives the type specified by the C header.
    unsafe {
        crate::check(sys::ghostty_snapshot_decoder_set(
            decoder,
            sys::snapshot_opt::MAX_CONTINUATION_BYTES,
            (&limit as *const usize).cast(),
        ))?;
        crate::check(sys::ghostty_snapshot_decoder_set(
            decoder,
            sys::snapshot_opt::RETAIN_CONTINUATION,
            (&retain as *const bool).cast(),
        ))?;
        crate::check(sys::ghostty_snapshot_decoder_set(
            decoder,
            sys::snapshot_opt::KITTY_IMAGE_STORAGE_LIMIT,
            (&images as *const u64).cast(),
        ))?;
        crate::check(sys::ghostty_snapshot_decoder_decode(decoder, &mut handle))?;
    }
    std::ptr::NonNull::new(handle).ok_or(Error::InvalidValue)
}

fn decode_error(bytes: &[u8], error: Error) -> SnapshotDecodeError {
    let format = snapshot_format();
    if error == Error::InvalidValue && bytes.get(..8) == Some(format.name.as_bytes()) {
        if let Some(version) = bytes.get(8..10) {
            let version = u16::from_le_bytes([version[0], version[1]]);
            if u32::from(version) != format.version {
                return SnapshotDecodeError::UnsupportedVersion { version };
            }
        }
    }
    SnapshotDecodeError::Library(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_refused_matching_envelope_gets_a_version_error() {
        let size = crate::Size {
            rows: 1,
            cols: 1,
            cell_px: None,
        };
        let mut bytes = Terminal::new(&size, crate::History::Off)
            .unwrap()
            .snapshot()
            .unwrap();
        assert_eq!(
            decode_error(&bytes, Error::InvalidValue),
            SnapshotDecodeError::Library(Error::InvalidValue)
        );
        let version = u16::from_le_bytes([bytes[8], bytes[9]]).wrapping_add(1);
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            decode_error(&bytes, Error::InvalidValue),
            SnapshotDecodeError::UnsupportedVersion { version }
        );
        assert_eq!(
            decode_error(&bytes, Error::OutOfMemory),
            SnapshotDecodeError::Library(Error::OutOfMemory)
        );
        assert_eq!(
            decode_error(&bytes[..8], Error::InvalidValue),
            SnapshotDecodeError::Library(Error::InvalidValue)
        );
        bytes[0] ^= 1;
        assert_eq!(
            decode_error(&bytes, Error::InvalidValue),
            SnapshotDecodeError::Library(Error::InvalidValue)
        );
    }
}

/// The identity that the worker gives to the programs it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalIdentityParts {
    /// The value of `TERM`.
    pub term: String,
    /// The terminfo source that `tic` reads.
    pub terminfo_source: String,
}

fn text(call: unsafe extern "C" fn(*mut sys::GString)) -> String {
    let mut out = sys::GString {
        ptr: std::ptr::null(),
        len: 0,
    };
    // SAFETY: `out` is a valid out pointer; the library's string is static for the life of the process.
    unsafe {
        call(&mut out);
        String::from_utf8_lossy(out.bytes()).into_owned()
    }
}

/// `TERM` and the terminfo source of the emulator, from the library (it renders the entry when it is built).
pub fn terminal_identity() -> TerminalIdentityParts {
    TerminalIdentityParts {
        term: text(sys::ghostty_terminfo_name),
        terminfo_source: text(sys::ghostty_terminfo_source),
    }
}
