//! Stateful terminal metadata producer for PTY byte streams.
//!
//! This parser observes a narrow set of OSC/control sequences while leaving
//! raw PTY bytes untouched for the authoritative terminal stream.

use crate::session_protocol::{NotificationPayload, PromptMarkPayload};

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const OSC_MAX_LEN: usize = 4096;

/// Semantic metadata observed while scanning PTY output bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalMetadataObservation {
    /// OSC 0/2 terminal title.
    TitleChanged(String),
    /// OSC 7 current working directory.
    CwdChanged(String),
    /// OSC 133 semantic prompt mark.
    PromptMark(PromptMarkPayload),
    /// Terminal bell.
    Bell,
    /// OSC 9/777 notification.
    Notification(NotificationPayload),
}

/// Stateful producer for semantic terminal metadata.
#[derive(Debug, Clone)]
pub struct TerminalMetadataProducer {
    state: ParserState,
    osc: Vec<u8>,
}

impl Default for TerminalMetadataProducer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalMetadataProducer {
    /// Build an empty producer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ParserState::Ground,
            osc: Vec::new(),
        }
    }

    /// Observe one PTY output chunk and return semantic metadata.
    pub fn observe(&mut self, bytes: &[u8]) -> Vec<TerminalMetadataObservation> {
        let mut observations = Vec::new();

        for &byte in bytes {
            match self.state {
                ParserState::Ground => match byte {
                    ESC => self.state = ParserState::Escape,
                    BEL => observations.push(TerminalMetadataObservation::Bell),
                    _ => {}
                },
                ParserState::Escape => {
                    if byte == b']' {
                        self.osc.clear();
                        self.state = ParserState::Osc;
                    } else {
                        self.state = ParserState::Ground;
                    }
                }
                ParserState::Osc => match byte {
                    BEL => {
                        self.finish_osc(&mut observations);
                        self.state = ParserState::Ground;
                    }
                    ESC => self.state = ParserState::OscEscape,
                    _ => self.push_osc_byte(byte),
                },
                ParserState::OscEscape => {
                    if byte == b'\\' {
                        self.finish_osc(&mut observations);
                        self.state = ParserState::Ground;
                    } else {
                        self.push_osc_byte(ESC);
                        self.push_osc_byte(byte);
                        self.state = ParserState::Osc;
                    }
                }
            }
        }

        observations
    }

    /// Return the retained partial OSC length for bounded-state tests.
    #[must_use]
    pub fn retained_len(&self) -> usize {
        self.osc.len()
    }

    fn push_osc_byte(&mut self, byte: u8) {
        if self.osc.len() >= OSC_MAX_LEN {
            self.osc.clear();
            self.state = ParserState::Ground;
            return;
        }
        self.osc.push(byte);
    }

    fn finish_osc(&mut self, observations: &mut Vec<TerminalMetadataObservation>) {
        if let Ok(text) = std::str::from_utf8(&self.osc) {
            if let Some(observation) = parse_osc(text) {
                observations.push(observation);
            }
        }
        self.osc.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Ground,
    Escape,
    Osc,
    OscEscape,
}

fn parse_osc(text: &str) -> Option<TerminalMetadataObservation> {
    let mut parts = text.split(';');
    let code = parts.next()?;
    match code {
        "0" | "2" => Some(TerminalMetadataObservation::TitleChanged(
            parts.collect::<Vec<_>>().join(";"),
        )),
        "7" => Some(TerminalMetadataObservation::CwdChanged(normalize_osc7_cwd(
            &parts.collect::<Vec<_>>().join(";"),
        ))),
        "9" => notification_from_parts(parts.collect()),
        "133" => parts.next().map(|mark| {
            TerminalMetadataObservation::PromptMark(PromptMarkPayload {
                mark: mark.to_string(),
            })
        }),
        "777" => parse_osc777(parts.collect()),
        _ => None,
    }
}

fn notification_from_parts(parts: Vec<&str>) -> Option<TerminalMetadataObservation> {
    match parts.as_slice() {
        [] => None,
        [body] => Some(TerminalMetadataObservation::Notification(
            NotificationPayload {
                title: "notification".to_string(),
                body: (*body).to_string(),
            },
        )),
        [title, rest @ ..] => Some(TerminalMetadataObservation::Notification(
            NotificationPayload {
                title: (*title).to_string(),
                body: rest.join(";"),
            },
        )),
    }
}

fn parse_osc777(parts: Vec<&str>) -> Option<TerminalMetadataObservation> {
    match parts.as_slice() {
        ["notify", title, rest @ ..] => Some(TerminalMetadataObservation::Notification(
            NotificationPayload {
                title: (*title).to_string(),
                body: rest.join(";"),
            },
        )),
        _ => notification_from_parts(parts),
    }
}

fn normalize_osc7_cwd(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("file://") {
        if let Some(path_index) = rest.find('/') {
            return rest[path_index..].to_string();
        }
    }
    value.to_string()
}
