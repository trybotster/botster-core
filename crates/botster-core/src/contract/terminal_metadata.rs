//! Stateful terminal metadata producer for PTY byte streams.
//!
//! This parser observes a narrow set of OSC/control sequences while leaving
//! raw PTY bytes untouched for the authoritative terminal stream.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

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

impl TerminalMetadataObservation {
    /// Payload-free metadata category for diagnostics.
    #[must_use]
    pub const fn kind(&self) -> TerminalMetadataKind {
        match self {
            Self::TitleChanged(_) => TerminalMetadataKind::Title,
            Self::CwdChanged(_) => TerminalMetadataKind::Cwd,
            Self::PromptMark(_) => TerminalMetadataKind::PromptMark,
            Self::Bell => TerminalMetadataKind::Bell,
            Self::Notification(_) => TerminalMetadataKind::Notification,
        }
    }
}

/// Payload-free terminal metadata lane category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalMetadataKind {
    /// OSC 0/2 terminal title.
    Title,
    /// OSC 7 current working directory.
    Cwd,
    /// OSC 133 semantic prompt mark.
    PromptMark,
    /// Terminal bell.
    Bell,
    /// OSC notification.
    Notification,
}

/// Typed terminal metadata shaping decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalMetadataShapingOutcome {
    /// Metadata was accepted for delivery.
    Accepted,
    /// A pending high-churn metadata value was replaced by the latest value.
    LatestWin,
    /// Metadata repeated an already retained value and was suppressed.
    Deduplicated,
    /// Metadata exceeded the explicit per-drain admission limit.
    RateLimited,
    /// Metadata could not be retained in the bounded lane.
    Dropped,
}

/// Payload-free terminal metadata shaping observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalMetadataShapingObservation {
    /// Metadata category affected by the outcome, when category-scoped.
    pub kind: Option<TerminalMetadataKind>,
    /// Typed shaping decision.
    pub outcome: TerminalMetadataShapingOutcome,
    /// Number of metadata observations covered by this report.
    pub count: usize,
}

impl TerminalMetadataShapingObservation {
    /// Build one category-scoped shaping observation.
    #[must_use]
    pub const fn one(kind: TerminalMetadataKind, outcome: TerminalMetadataShapingOutcome) -> Self {
        Self {
            kind: Some(kind),
            outcome,
            count: 1,
        }
    }
}

/// Bounded shaper for lossy terminal metadata lanes.
#[derive(Debug, Clone)]
pub struct TerminalMetadataLaneShaper {
    pending: VecDeque<TerminalMetadataObservation>,
    capacity: usize,
    per_drain_limit: usize,
    accepted_this_drain: usize,
}

impl TerminalMetadataLaneShaper {
    /// Build a bounded shaper. Capacity and limit are clamped to at least one.
    #[must_use]
    pub fn new(capacity: usize, per_drain_limit: usize) -> Self {
        Self {
            pending: VecDeque::new(),
            capacity: capacity.max(1),
            per_drain_limit: per_drain_limit.max(1),
            accepted_this_drain: 0,
        }
    }

    /// Admit one observation, returning payload-free shaping observations.
    pub fn push(
        &mut self,
        observation: TerminalMetadataObservation,
    ) -> Vec<TerminalMetadataShapingObservation> {
        let kind = observation.kind();

        if self.accepted_this_drain >= self.per_drain_limit {
            return vec![TerminalMetadataShapingObservation::one(
                kind,
                TerminalMetadataShapingOutcome::RateLimited,
            )];
        }

        if self.pending.iter().any(|pending| pending == &observation) {
            return vec![TerminalMetadataShapingObservation::one(
                kind,
                TerminalMetadataShapingOutcome::Deduplicated,
            )];
        }

        if matches!(
            kind,
            TerminalMetadataKind::Title | TerminalMetadataKind::Cwd
        ) {
            if let Some(pending) = self
                .pending
                .iter_mut()
                .find(|pending| pending.kind() == kind)
            {
                *pending = observation;
                return vec![TerminalMetadataShapingObservation::one(
                    kind,
                    TerminalMetadataShapingOutcome::LatestWin,
                )];
            }
        }

        if self.pending.len() >= self.capacity {
            return vec![TerminalMetadataShapingObservation::one(
                kind,
                TerminalMetadataShapingOutcome::Dropped,
            )];
        }

        self.pending.push_back(observation);
        self.accepted_this_drain += 1;
        vec![TerminalMetadataShapingObservation::one(
            kind,
            TerminalMetadataShapingOutcome::Accepted,
        )]
    }

    /// Drain retained metadata in source order after shaping.
    pub fn drain(&mut self) -> Vec<TerminalMetadataObservation> {
        self.accepted_this_drain = 0;
        self.pending.drain(..).collect()
    }

    /// Retained metadata count for boundedness checks.
    #[must_use]
    pub fn retained_len(&self) -> usize {
        self.pending.len()
    }

    /// Configured retained metadata capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
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
        "7" => normalize_osc7_cwd(&parts.collect::<Vec<_>>().join(";"))
            .map(TerminalMetadataObservation::CwdChanged),
        "9" => parse_osc9(parts.collect()),
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

fn parse_osc9(parts: Vec<&str>) -> Option<TerminalMetadataObservation> {
    match parts.as_slice() {
        // OSC 9;4 is a progress-reporting form, not a notification.
        ["4", ..] => None,
        _ => notification_from_parts(parts),
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

fn normalize_osc7_cwd(value: &str) -> Option<String> {
    if let Some(rest) = value.strip_prefix("file://") {
        if let Some(path_index) = rest.find('/') {
            return Some(rest[path_index..].to_string());
        }
        return None;
    }
    Some(value.to_string())
}
