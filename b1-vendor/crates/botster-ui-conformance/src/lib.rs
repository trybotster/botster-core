//! The structural check of the UI transcripts that are not vectors (steward ruling R-1.4, design 7.1).
//!
//! The vectors of the `realizer` subject live in `botster-ui-contract` and run against the reference `realize` (UI-RZ-3b).
//! The other UI ids are proved by a renderer or by the Hub. Their transcripts are in `conformance/ui/`:
//!
//! * subject `client`: the steps of `botster_conformance::client` (`client_load`, `client_act`, `client_expect`);
//! * subject `hub`: the HC step vocabulary (`connect`, `send`, `expect`, ...).
//!
//! Phase 0 has no fake client. A `client` transcript passes when every step parses, its document passes `admit`, and
//! every embedded input parses as its contract type. A real client runs the same file in Phase 1.

use botster_conformance::client::{parse_client_steps, Action, ClientStep};
use botster_conformance::{load_dir, Contract, Subject, Transcript};
use botster_ui_contract::prelude::{
    admit, ActionError, ActionOk, AdmissionContext, Capabilities, KindSnapshot, LocalState,
    Viewport,
};
use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::collections::BTreeMap;

static TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/ui");

/// Every transcript of `conformance/ui/`, sorted by id.
pub fn transcripts() -> Vec<Transcript> {
    load_dir(&TRANSCRIPTS).unwrap_or_else(|e| panic!("conformance/ui: {e}"))
}

/// The structural check of one transcript. `Err` lists every problem.
pub fn check(t: &Transcript) -> Result<(), Vec<String>> {
    let mut problems = vec![];
    if t.contract != Contract::Ui {
        problems.push(format!("{}: contract ui is required", t.id));
    }
    match t.subject {
        Subject::Client => check_client(t, &mut problems),
        // A `hub` transcript is checked structurally in `botster-hub-conformance` (its frames need the HC schemas), and it runs against the
        // real Hub in Stage 2 (HC A5-1).
        Subject::Hub => {}
        other => problems.push(format!(
            "{}: subject {other:?} belongs in the vectors of botster-ui-contract",
            t.id
        )),
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn check_client(t: &Transcript, problems: &mut Vec<String>) {
    let steps = match parse_client_steps(&t.steps) {
        Ok(s) => s,
        Err(e) => {
            problems.push(format!("{}: {e}", t.id));
            return;
        }
    };
    let ctx: AdmissionContext = match t.setup.get("admission") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(c) => c,
            Err(e) => {
                problems.push(format!("{}: setup.admission: {e}", t.id));
                return;
            }
        },
        None => AdmissionContext::default(),
    };
    for (i, step) in steps.iter().enumerate() {
        if let ClientStep::ClientLoad(l) = step {
            // R-1.4: the document passes the real admission.
            if let Err(f) = admit(&l.document, &ctx) {
                problems.push(format!("{} step {i}: admission refused {f:?}", t.id));
            }
            parse_as::<BTreeMap<String, KindSnapshot>>(
                &l.kind_snapshots,
                "kind_snapshots",
                t,
                i,
                problems,
            );
            parse_as::<LocalState>(&l.local_state, "local_state", t, i, problems);
            parse_as::<Viewport>(&l.viewport, "viewport", t, i, problems);
            parse_as::<Capabilities>(&l.capabilities, "capabilities", t, i, problems);
        }
    }
    for (i, step) in steps.iter().enumerate() {
        if let ClientStep::ClientAct(a) = step {
            check_act(&a.action, t, i, problems);
        }
    }
    if !steps
        .iter()
        .any(|s| matches!(s, ClientStep::ClientExpect(_)))
    {
        problems.push(format!("{}: a transcript observes something", t.id));
    }
}

fn parse_as<T: serde::de::DeserializeOwned>(
    v: &Option<Value>,
    what: &str,
    t: &Transcript,
    step: usize,
    problems: &mut Vec<String>,
) {
    if let Some(v) = v {
        if let Err(e) = serde_json::from_value::<T>(v.clone()) {
            problems.push(format!("{} step {step}: {what}: {e}", t.id));
        }
    }
}

/// The Hub inputs of a `client_act` are values of contract types (R-1.4): an error parses as `ActionError`, an `ok` value
/// is an object, and a snapshot row is an object with an `id`.
fn check_act(action: &Action, t: &Transcript, step: usize, problems: &mut Vec<String>) {
    match action {
        Action::HubResult { ok, err, .. } => {
            if ok.is_some() == err.is_some() {
                problems.push(format!(
                    "{} step {step}: hub_result has exactly one of ok and err",
                    t.id
                ));
            }
            if let Some(e) = err {
                if let Err(e) = serde_json::from_value::<ActionError>(e.clone()) {
                    problems.push(format!(
                        "{} step {step}: err is not an ActionError: {e}",
                        t.id
                    ));
                }
            }
            if let Some(o) = ok {
                if let Err(e) = serde_json::from_value::<ActionOk>(o.clone()) {
                    problems.push(format!("{} step {step}: ok is not an ActionOk: {e}", t.id));
                }
            }
        }
        Action::HubRows { rows, .. } => {
            if rows.iter().any(|r| r.get("id").is_none()) {
                problems.push(format!("{} step {step}: every row has an id", t.id));
            }
        }
        _ => {}
    }
}
