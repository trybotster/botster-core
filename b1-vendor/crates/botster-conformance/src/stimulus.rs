//! Checks before a stimulus is sent (design 5.4).
//!
//! | Step | Check before the driver sees it |
//! |---|---|
//! | `send`, `send_frame`, `route_send` | Variables are substituted. The frame must validate against its schema. |
//! | `control` with `op: datachannel_send` | The nested `frame` is checked as a `route_send` frame is (Core DP-3: the peer sends a codec frame, never hand-written bytes). |
//! | `send_invalid` | `{"send_invalid": <value>, "violates": "<schema ref>[@<rule>]"}`. Variables are substituted. The value must FAIL the schema, and with `@<rule>` one failure must be at exactly that schema path (for example `@/required`, which is not `/properties/outer/required`). |
//! | `send_raw`, `route_send_raw`, `send_frame_raw`, `send_wire`, `route_send_wire` | None. The step reaches the driver unchanged, byte for byte. Each step is one write. |
//!
//! A failed check is a transcript error. The subject never sees the stimulus, so a typo never reaches it.

use crate::driver::StepError;
use crate::matcher::{substitute, Bindings, Direction, FrameSchema, SchemaResolver};
use serde_json::{json, Value};

const VALID_FRAME_STEPS: [&str; 3] = ["send", "send_frame", "route_send"];
const RAW_STEPS: [&str; 5] = [
    "send_raw",
    "route_send_raw",
    "send_frame_raw",
    "send_wire",
    "route_send_wire",
];

fn transcript_error(reason: String) -> StepError {
    StepError::Failed {
        expected: json!("a stimulus that passes its pre-send check"),
        received: json!(reason),
    }
}

/// A frame that a stimulus sends must validate against its schema.
fn check_frame(kind: &str, frame: &Value, schemas: &dyn SchemaResolver) -> Result<(), StepError> {
    match schemas.frame_schema(Direction::Sent, frame) {
        FrameSchema::Ref(r) => schemas.validate(&r, frame).map_err(|e| {
            transcript_error(format!(
                "'{kind}' frame does not validate against '{r}': {e}"
            ))
        }),
        FrameSchema::NotFramed => Err(transcript_error(format!(
            "'{kind}' value has no frame schema"
        ))),
    }
}

/// Returns the step to dispatch: substituted and checked, or unchanged for a raw stimulus and for any other step.
pub fn prepare(
    step: &Value,
    bindings: &Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<Value, StepError> {
    let Some(obj) = step.as_object() else {
        return Ok(step.clone());
    };
    if RAW_STEPS.iter().any(|k| obj.contains_key(*k)) {
        return Ok(step.clone());
    }
    if let Some(kind) = VALID_FRAME_STEPS.iter().find(|k| obj.contains_key(**k)) {
        let prepared = substitute(step, bindings)
            .map_err(|v| transcript_error(format!("unbound variable {v}")))?;
        check_frame(kind, &prepared[*kind], schemas)?;
        return Ok(prepared);
    }
    // A control that carries a codec frame (Core DP-3, `datachannel_send`): the nested frame must validate before it is sent.
    if obj
        .get("control")
        .and_then(|c| c.get("op"))
        .is_some_and(|op| op == "datachannel_send")
    {
        // The frame is checked on its own, before the other arguments (a route that is bound only at run time) are substituted:
        // a structural run has no bindings, and a bad frame must still be found.
        match step["control"].get("frame") {
            Some(frame) => {
                let frame = substitute(frame, bindings)
                    .map_err(|v| transcript_error(format!("unbound variable {v}")))?;
                check_frame("datachannel_send", &frame, schemas)?;
            }
            None => {
                return Err(transcript_error(
                    "'datachannel_send' needs a 'frame'".to_string(),
                ))
            }
        }
        return substitute(step, bindings)
            .map_err(|v| transcript_error(format!("unbound variable {v}")));
    }
    if obj.contains_key("send_invalid") {
        let prepared = substitute(step, bindings)
            .map_err(|v| transcript_error(format!("unbound variable {v}")))?;
        let violates = prepared
            .get("violates")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                transcript_error("'send_invalid' needs a string 'violates'".to_string())
            })?;
        let (schema_ref, rule) = match violates.split_once('@') {
            Some((r, rule)) => (r, Some(rule)),
            None => (violates, None),
        };
        let paths = schemas
            .violations(schema_ref, &prepared["send_invalid"])
            .map_err(transcript_error)?;
        if paths.is_empty() {
            return Err(transcript_error(format!(
                "'send_invalid' value passes '{schema_ref}'"
            )));
        }
        if let Some(rule) = rule {
            if !paths.iter().any(|p| p == rule) {
                return Err(transcript_error(format!(
                    "'send_invalid' value fails '{schema_ref}' at {paths:?}, not at '{rule}'"
                )));
            }
        }
        return Ok(prepared);
    }
    Ok(step.clone())
}
