//! Fake terminal screen runtime for consumer conformance tests.

use botster_core::{
    ModeFlags, TerminalBackendError, TerminalColorProfile, TerminalOutputChunk,
    TerminalScreenRuntime, TerminalScreenSize, TerminalScreenState, TerminalSnapshotPayload,
};

/// Fake runtime for terminal screen boundary tests.
#[derive(Debug, Clone)]
pub struct FakeTerminalScreenRuntime {
    size: TerminalScreenSize,
    bytes: Vec<u8>,
    plain_text: String,
    title: Option<String>,
    cwd: Option<String>,
    mode_flags: ModeFlags,
    mode_flags_configured: bool,
    color_profile: Option<TerminalColorProfile>,
    format: Option<String>,
}

impl Default for FakeTerminalScreenRuntime {
    fn default() -> Self {
        Self {
            size: TerminalScreenSize::new(24, 80),
            bytes: Vec::new(),
            plain_text: String::new(),
            title: None,
            cwd: None,
            mode_flags: ModeFlags::default(),
            mode_flags_configured: false,
            color_profile: None,
            format: Some("fake-opaque-v1".to_string()),
        }
    }
}

impl FakeTerminalScreenRuntime {
    /// Build an empty fake runtime.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current raw bytes held by the fake runtime.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Set synced terminal metadata on the fake runtime.
    pub fn set_synced_state(
        &mut self,
        title: Option<String>,
        cwd: Option<String>,
        mode_flags: ModeFlags,
        color_profile: Option<TerminalColorProfile>,
    ) {
        self.title = title;
        self.cwd = cwd;
        self.mode_flags = mode_flags;
        self.mode_flags_configured = true;
        self.color_profile = color_profile;
    }

    fn refresh_plain_text(&mut self) {
        self.plain_text = String::from_utf8_lossy(&self.bytes).into_owned();
    }
}

impl TerminalScreenRuntime for FakeTerminalScreenRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        self.bytes.extend_from_slice(bytes);
        self.refresh_plain_text();
        TerminalOutputChunk::new(bytes.to_vec())
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.size = size;
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        TerminalSnapshotPayload::new(self.bytes.clone(), self.size, self.format.clone())
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.bytes = payload.bytes;
        self.size = payload.size;
        self.format = payload.format;
        self.refresh_plain_text();
    }

    fn screen_state(&self) -> TerminalScreenState {
        TerminalScreenState {
            size: self.size,
            plain_text: self.plain_text.clone(),
            title: self.title.clone(),
            cwd: self.cwd.clone(),
            mode_flags: self.mode_flags.clone(),
            color_profile: self.color_profile.clone(),
        }
    }

    fn mode_flags(&self) -> Result<ModeFlags, TerminalBackendError> {
        self.mode_flags_configured
            .then(|| self.mode_flags.clone())
            .ok_or_else(|| TerminalBackendError::unsupported("mode_flags"))
    }
}
