//! The snapshot of a terminal and the identity of the emulator. libghostty writes both; this crate copies the bytes.

use crate::{sys, Error, Terminal};

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

/// What the worker names in `ProtocolManifest` for the snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotFormat {
    pub continuation_limit: usize,
}

pub const SNAPSHOT_FORMAT: SnapshotFormat = SnapshotFormat { continuation_limit: CONTINUATION_LIMIT };

impl Terminal {
    pub(crate) fn set_continuation_limit(&self) -> Result<(), Error> {
        let limit = CONTINUATION_LIMIT;
        // SAFETY: the handle is live, and the option takes a `size_t` pointer.
        crate::check(unsafe {
            sys::ghostty_terminal_set(self.handle.as_ptr(), sys::opt::CONTINUATION_MAX_BYTES, (&limit as *const usize).cast())
        })
    }

    /// The complete snapshot: screen, scrollback, modes and the unfinished parser input. It is the library's own
    /// format, and `Terminal::new` is not its reader.
    pub fn snapshot(&self) -> Result<Vec<u8>, SnapshotError> {
        let mut needed = 0usize;
        // SAFETY: a null buffer of length zero asks for the size, and `needed` is a valid out pointer.
        let code = unsafe { sys::ghostty_snapshot_encode_buf(self.handle.as_ptr(), std::ptr::null_mut(), 0, &mut needed) };
        match code {
            sys::OUT_OF_SPACE => {}
            sys::SUCCESS => return Ok(Vec::new()),
            sys::INVALID_VALUE => return Err(SnapshotError::ContinuationUnavailable),
            other => return Err(SnapshotError::Library(Error::from_code(other))),
        }
        let mut out = Vec::new();
        out.try_reserve_exact(needed).map_err(|_| SnapshotError::Library(Error::OutOfMemory))?;
        out.resize(needed, 0);
        let mut written = 0usize;
        // SAFETY: the buffer holds `needed` bytes, and `&self` means that nothing changes the terminal between the two
        // calls.
        let code = unsafe { sys::ghostty_snapshot_encode_buf(self.handle.as_ptr(), out.as_mut_ptr(), out.len(), &mut written) };
        match code {
            sys::SUCCESS => {
                out.truncate(written);
                Ok(out)
            }
            sys::INVALID_VALUE => Err(SnapshotError::ContinuationUnavailable),
            other => Err(SnapshotError::Library(Error::from_code(other))),
        }
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
    let mut out = sys::GString { ptr: std::ptr::null(), len: 0 };
    // SAFETY: `out` is a valid out pointer; the library's string is static for the life of the process.
    unsafe {
        call(&mut out);
        String::from_utf8_lossy(out.bytes()).into_owned()
    }
}

/// `TERM` and the terminfo source of the emulator, from the library (it renders the entry when it is built).
pub fn terminal_identity() -> TerminalIdentityParts {
    TerminalIdentityParts { term: text(sys::ghostty_terminfo_name), terminfo_source: text(sys::ghostty_terminfo_source) }
}
