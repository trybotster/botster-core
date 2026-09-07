//! Paste payload fixtures shared by every harness layer.
//!
//! The byte rules are stated here so a JavaScript harness can reproduce them
//! without a published fixture: `printable_payload(len)` cycles `0x20..=0x7E`;
//! `control_byte_payload(len)` cycles `0x00..=0xFF`. Smokes use
//! `len = 65_536` so a paste crosses chunk boundaries.

/// `len` bytes cycling through printable ASCII `0x20..=0x7E`, no newline.
///
/// Ghostty accepts it as a safe paste and leaves every byte unchanged.
#[must_use]
pub fn printable_payload(len: usize) -> Vec<u8> {
    (0..len).map(|index| 0x20 + (index % 0x5F) as u8).collect()
}

/// `len` bytes cycling through the full byte range `0x00..=0xFF`.
///
/// It contains `\n`, so Ghostty rejects it as unsafe unless the paste allows
/// unsafe content.
#[must_use]
pub fn control_byte_payload(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index % 0x100) as u8).collect()
}

/// Encode a paste for a new production Ghostty terminal.
///
/// A new terminal has bracketed paste disabled. This function calls the
/// production encoder and does not copy its byte transformation rules.
#[cfg(feature = "ghostty-terminal")]
pub fn encode_unbracketed_paste_for_pty(
    payload: &[u8],
) -> Result<Vec<u8>, botster_terminal_ghostty::GhosttyTerminalError> {
    let terminal = botster_terminal_ghostty::GhosttyTerminal::new(
        botster_core::TerminalScreenSize::new(24, 80),
    )?;
    let mut input = payload.to_vec();
    let mut output = Vec::new();
    terminal.encode_paste(&mut input, &mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_follow_the_stated_cycles() {
        let printable = printable_payload(0x5F + 2);
        assert_eq!(printable[0], 0x20);
        assert_eq!(printable[0x5E], 0x7E);
        assert_eq!(printable[0x5F], 0x20);
        assert!(!printable.contains(&b'\n'));
        let control = control_byte_payload(0x100 + 1);
        assert_eq!(control[0xFF], 0xFF);
        assert_eq!(control[0x100], 0x00);
        assert!(control.contains(&b'\n'));
    }
}
