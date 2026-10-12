//! The transcript model (design 5.1): the common envelope. Suite step kinds stay opaque JSON values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The ledger prefix that owns a test id (design section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Contract {
    Core,
    Codec,
    Hc,
    Hp,
    Lb,
    Ui,
    Cli,
}

/// The side under test (design 5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    Core,
    Worker,
    Hub,
    Client,
    PluginHub,
    PluginHost,
    LuaHost,
    Cli,
    Realizer,
}

/// What a subject must offer for a transcript to apply (design 5.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// Feature names that the subject must advertise. A subject without one gives `not_applicable`.
    #[serde(default)]
    pub features: Vec<String>,
    /// Test controls that the transcript uses. A subject without one gives `unsupported_control`.
    #[serde(default)]
    pub controls: Vec<String>,
}

/// A transcript that has no steps by design (design section 2, withdrawn ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptKind {
    Withdrawn,
}

/// How a subject's fake treats a transcript (design 5.1, `fake`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FakeMode {
    /// The subject's fake does not model this behavior by steward ruling (`ruling`). Against the fake the runner checks the stimuli
    /// only (`structural`). Against a real implementation the transcript runs in full.
    Structural,
}

/// One transcript file: one test id (design 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Transcript {
    /// The test id. It equals the file name without `.json`, and it is in the ledger.
    pub id: String,
    /// The ledger prefix that owns the id.
    pub contract: Contract,
    /// The clause row id, for example `EN-8`.
    pub clause: String,
    /// The side under test.
    pub subject: Subject,
    #[serde(default)]
    pub requires: Requires,
    /// Limits and a named fixture. Its shape belongs to the suite.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub setup: Value,
    /// The ordered script. Its step kinds belong to the suite, except the order constructs of the engine.
    #[serde(default)]
    pub steps: Vec<Value>,
    /// Pinned seeds. Absent means every seed of the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeds: Option<Vec<u64>>,
    /// `withdrawn` marks a transcript with no steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TranscriptKind>,
    /// `structural` marks a transcript that runs in full only against a real implementation. It needs a `ruling`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fake: Option<FakeMode>,
    /// The steward ruling that makes the transcript structural against a fake, for example `R-5`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruling: Option<String>,
}

/// The generated JSON Schema of [`Transcript`], as the text of `schemas/transcript.schema.json`.
pub fn transcript_schema_json() -> String {
    let schema = schemars::schema_for!(Transcript);
    let mut text = serde_json::to_string_pretty(&schema).expect("schema serializes");
    text.push('\n');
    text
}
