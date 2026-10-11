//! The real tier's session lookup for the oracle controls ([`crate::oracle_controls`]). The oracle reads are shared with the
//! testkit; this tier supplies the size of the stored row and the session's known output history.
//!
//! `oracle_screen` reads O_raw of `oracle_resume` (R-62): a fresh oracle terminal that applies every `pty_output` that the
//! session's probe acknowledged, with the size and the profile of the stored request. Conditions 1 and 3 of `oracle_resume`
//! hold for it: a session whose program writes output that the transcript does not state gives `unsupported_control`
//! `unknown_output_history`, and a session that a `Resize` or a `SetColorProfile` reached gives `unsupported_control`
//! `size_or_profile_changed`. The transcript fences the read (condition 2): a read before the model applied the output gives
//! only a false failure.

use super::resume::known_output_history;
use super::RealCoreHarness;
use crate::capture_layer::lock;
use crate::controls::parse;
use crate::oracle_compare::raw_oracle;
use crate::oracle_controls::{screen, QueryReply, Screen};
use botster_core_conformance::ControlError;
use serde_json::Value;

impl RealCoreHarness {
    /// `oracle_query_reply` (Core EV-8): the size of the session's stored row.
    pub(super) fn oracle_query_reply(
        &self,
        handle: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        let args: QueryReply = parse(args)?;
        let (_, row) = self.stored(handle, &args.session)?;
        args.reply(&row.request.size)
    }

    /// `oracle_screen` (Core ST-2, A2-1): see the module documentation.
    pub(super) fn oracle_screen(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let Screen { session, history } = parse(args)?;
        let (_, row) = self.stored(handle, &session)?;
        if !known_output_history(&row.request) {
            return Err(ControlError::UnsupportedFor(
                "unknown_output_history".into(),
            ));
        }
        let shared = self
            .handles
            .get(handle)
            .and_then(|open| open.resume.upgrade())
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
        lock(&shared).check(&session)?;
        let worker = row.worker.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no worker process", session.0))
        })?;
        let all: Vec<u8> = self
            .programs
            .output(worker.identity())
            .into_iter()
            .flat_map(|(_, bytes)| bytes)
            .collect();
        let raw = raw_oracle(&row.request.size, row.request.color_profile.as_ref(), &all)
            .map_err(|e| ControlError::Bad(format!("oracle_screen: no oracle terminal: {e:?}")))?;
        screen(&raw, history)
    }
}
