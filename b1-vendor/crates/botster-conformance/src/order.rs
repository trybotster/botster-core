//! The order constructs (design 5.3), implemented over a [`StepDriver`].
//!
//! | Step | Form |
//! |---|---|
//! | `expect` | `{"expect": <matcher>, "on"?: <scope>}` |
//! | `expect_set` | `{"expect_set": [<matcher>, ..], "ack"?: <step>, "on"?: <scope>}` |
//! | `expect_eventually` | `{"expect_eventually": <matcher>, "ignore"?: [<matcher>, ..], "ack"?: <step>, "on"?: <scope>}` |
//! | `expect_quiet_until` | `{"expect_quiet_until": <matcher>, "barrier": <step>, "until": <matcher>, "on"?: <scope>}` |
//! | `expect_closed` | `{"expect_closed": <reason matcher or null>, "ignore"?: [<matcher>, ..], "on"?: <scope>}` |
//!
//! `ack` (feature hp-events, lead-approved): a step template that the engine hands to the driver for each item as it arrives (the matched item, and for `expect_eventually` every ignored item too), before it waits for the next one.
//! Each `{"$item": "<json pointer>"}` in the template is replaced by that value of the item; a pointer that the item does not have fails the step. It lets a transcript answer
//! items of a set whose order the contract leaves open (the Hub sends the next item only after the answer to the previous one). It asserts no order and binds no variable.
//! An object is an order step only when its keys all belong to the form above. Any other object goes to the driver.
//! Each form also takes `"because": "<contract> <clause id>"` (design 5.6, rule 6), the clause that fixes the expected value. Several clauses are a list of such strings.
//! Each form also takes `"allow_invalid": true`: the received frames of that step are not schema-checked (a step that expects a protocol error from the subject).
//! Received frames are otherwise validated against the resolver's frame schema before they are matched (design 5.4).
//! A run that does not finish within [`Limits`] gives `inconclusive`, never `failed`.

use crate::driver::{Deadline, Poll, StepDriver, StepError};
use crate::matcher::{matches, Bindings, Direction, FrameSchema, MatchFailure, SchemaResolver};
use crate::stimulus::prepare;
use serde_json::{json, Map, Value};
use std::time::Duration;

/// Runner execution limits (design 6.1). They are not contract bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most driver polls that one waiting step may use against a fake.
    pub max_polls: usize,
    /// The wall-clock limit of one step against a real implementation. `None` for a fake.
    pub step_timeout: Option<Duration>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_polls: 10_000,
            step_timeout: None,
        }
    }
}

impl Limits {
    /// The wall-clock limit of one step against a real implementation with a real clock (design 6.1). It is not a contract bound.
    pub const REAL_STEP_TIMEOUT: Duration = Duration::from_secs(10);

    /// The limits against a real implementation: the wall clock ends a waiting step, never the poll count.
    pub fn real() -> Self {
        Limits {
            max_polls: usize::MAX,
            step_timeout: Some(Self::REAL_STEP_TIMEOUT),
        }
    }
}

/// A failed step and its position in the script.
#[derive(Debug, Clone, PartialEq)]
pub struct StepFailure {
    pub step: usize,
    pub error: StepError,
}

/// Runs every step in order. Stops at the first failure.
pub fn run_steps(
    steps: &[Value],
    driver: &mut dyn StepDriver,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
    limits: &Limits,
) -> Result<(), StepFailure> {
    for (step, value) in steps.iter().enumerate() {
        run_step(value, driver, bindings, schemas, limits)
            .map_err(|error| StepFailure { step, error })?;
    }
    Ok(())
}

/// Runs one step: an order construct, or any other step through the driver.
pub fn run_step(
    step: &Value,
    driver: &mut dyn StepDriver,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
    limits: &Limits,
) -> Result<(), StepError> {
    let deadline = Deadline::after(limits.step_timeout);
    let Some(kind) = order_kind(step) else {
        let prepared = prepare(step, bindings, schemas)?;
        let result = driver.exec(&prepared, bindings, &deadline);
        return if deadline.expired() {
            Err(timeout())
        } else {
            result
        };
    };
    let obj = step.as_object().expect("order_kind checked an object");
    let scope = obj.get("on").cloned().unwrap_or(Value::Null);
    let mut waiter = Waiter {
        driver,
        scope: &scope,
        budget: limits.max_polls,
        deadline,
        schemas,
        validate: obj.get("allow_invalid") != Some(&Value::Bool(true)),
    };
    match kind {
        "expect" => expect(&mut waiter, &obj["expect"], bindings, schemas),
        "expect_set" => expect_set(&mut waiter, obj, bindings, schemas),
        "expect_eventually" => expect_eventually(&mut waiter, obj, bindings, schemas),
        "expect_quiet_until" => expect_quiet_until(&mut waiter, obj, bindings, schemas, limits),
        "expect_closed" => expect_closed(&mut waiter, obj, bindings, schemas),
        _ => unreachable!("order_kind returns a known kind"),
    }
}

/// The order construct that the step is (`expect`, `expect_set`, ...), or `None` for a step that goes to the driver. A driver can read
/// the nested steps that the runner executes with it: the `ack` of `expect_set` and `expect_eventually`, and the `barrier` of
/// `expect_quiet_until`.
pub fn order_kind(step: &Value) -> Option<&'static str> {
    let obj = step.as_object()?;
    let forms: [(&str, &[&str]); 5] = [
        ("expect", &["on", "allow_invalid", "because"]),
        ("expect_set", &["on", "ack", "allow_invalid", "because"]),
        (
            "expect_eventually",
            &["on", "ignore", "ack", "allow_invalid", "because"],
        ),
        (
            "expect_quiet_until",
            &["on", "barrier", "until", "allow_invalid", "because"],
        ),
        (
            "expect_closed",
            &["on", "ignore", "allow_invalid", "because"],
        ),
    ];
    let mut found = None;
    for (name, extras) in forms {
        if obj.contains_key(name) {
            if found.is_some() {
                return None;
            }
            if obj
                .keys()
                .all(|k| k == name || extras.contains(&k.as_str()))
            {
                found = Some(name);
            } else {
                return None;
            }
        }
    }
    found
}

enum Next {
    Item(Value),
    Closed(Value),
    Idle,
}

fn timeout() -> StepError {
    StepError::Inconclusive {
        limit: "step_timeout".to_string(),
    }
}

struct Waiter<'a> {
    driver: &'a mut dyn StepDriver,
    scope: &'a Value,
    budget: usize,
    deadline: Deadline,
    schemas: &'a dyn SchemaResolver,
    validate: bool,
}

impl Waiter<'_> {
    /// A received frame must validate against its schema before it is matched (design 5.4).
    fn check_received(&self, item: &Value) -> Result<(), StepError> {
        if !self.validate {
            return Ok(());
        }
        if let FrameSchema::Ref(r) = self.schemas.frame_schema(Direction::Received, item) {
            self.schemas
                .validate(&r, item)
                .map_err(|e| StepError::Failed {
                    expected: json!(format!("a received frame that validates against '{r}'")),
                    received: json!({ "frame": item, "error": e }),
                })?;
        }
        Ok(())
    }

    fn next(&mut self) -> Result<Next, StepError> {
        while self.budget > 0 {
            self.budget -= 1;
            if self.deadline.expired() {
                return Err(timeout());
            }
            let polled = self.driver.next(self.scope, &self.deadline);
            if matches!(polled, Poll::TimedOut) || self.deadline.expired() {
                return Err(timeout());
            }
            match polled {
                Poll::Item(v) => {
                    self.check_received(&v)?;
                    return Ok(Next::Item(v));
                }
                Poll::Closed(v) => return Ok(Next::Closed(v)),
                Poll::Idle => return Ok(Next::Idle),
                Poll::Pending => {}
                Poll::TimedOut => unreachable!("handled above"),
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".to_string(),
        })
    }
}

fn failed(expected: &Value, received: Value) -> StepError {
    StepError::Failed {
        expected: expected.clone(),
        received,
    }
}

fn bad_form(reason: &str) -> StepError {
    StepError::Failed {
        expected: json!("a well-formed order step"),
        received: json!(reason),
    }
}

fn match_failed(matcher: &Value, item: &Value, failure: &MatchFailure) -> StepError {
    match failure {
        MatchFailure::Mismatch { .. } => failed(matcher, item.clone()),
        MatchFailure::BadMatcher { .. } => bad_form(&failure.describe()),
    }
}

/// A matcher that does not match gives `Ok(false)`. A malformed matcher is an error.
fn try_match(
    matcher: &Value,
    item: &Value,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<bool, StepError> {
    match matches(matcher, Some(item), bindings, schemas) {
        Ok(()) => Ok(true),
        Err(MatchFailure::Mismatch { .. }) => Ok(false),
        Err(bad @ MatchFailure::BadMatcher { .. }) => Err(bad_form(&bad.describe())),
    }
}

fn matcher_list<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], StepError> {
    match obj.get(key) {
        None => Ok(&[]),
        Some(Value::Array(list)) => Ok(list),
        Some(_) => Err(bad_form(&format!("'{key}' takes an array of matchers"))),
    }
}

fn describe_end(got: &Next, what: &str) -> Value {
    match got {
        Next::Item(v) => v.clone(),
        Next::Closed(reason) => json!({ "closed": reason }),
        Next::Idle => json!(format!("no item arrived: {what}")),
    }
}

fn expect(
    w: &mut Waiter<'_>,
    matcher: &Value,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<(), StepError> {
    match w.next()? {
        Next::Item(item) => matches(matcher, Some(&item), bindings, schemas)
            .map_err(|f| match_failed(matcher, &item, &f)),
        other => Err(failed(
            matcher,
            describe_end(&other, "expected the next item"),
        )),
    }
}

/// `ack`: the step template with every `{"$item": "<pointer>"}` replaced from the item.
fn item_step(template: &Value, item: &Value) -> Result<Value, StepError> {
    match template {
        Value::Object(o) if o.len() == 1 && o.contains_key("$item") => {
            let pointer = o["$item"]
                .as_str()
                .ok_or_else(|| bad_form("'$item' takes a JSON pointer"))?;
            item.pointer(pointer).cloned().ok_or_else(|| {
                failed(
                    &json!(format!("the item has a value at {pointer}")),
                    item.clone(),
                )
            })
        }
        Value::Object(o) => o
            .iter()
            .map(|(k, v)| Ok((k.clone(), item_step(v, item)?)))
            .collect::<Result<Map<String, Value>, StepError>>()
            .map(Value::Object),
        Value::Array(a) => a
            .iter()
            .map(|v| item_step(v, item))
            .collect::<Result<Vec<Value>, StepError>>()
            .map(Value::Array),
        other => Ok(other.clone()),
    }
}

/// Hands the `ack` step of an order step to the driver for one received item (it does nothing when the step has none).
fn ack_item(
    w: &mut Waiter<'_>,
    obj: &Map<String, Value>,
    item: &Value,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<(), StepError> {
    let Some(template) = obj.get("ack") else {
        return Ok(());
    };
    let step = item_step(template, item)?;
    let prepared = prepare(&step, bindings, schemas)?;
    let result = w.driver.exec(&prepared, bindings, &w.deadline);
    if w.deadline.expired() {
        Err(timeout())
    } else {
        result
    }
}

fn expect_set(
    w: &mut Waiter<'_>,
    obj: &Map<String, Value>,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<(), StepError> {
    let Value::Array(matchers) = &obj["expect_set"] else {
        return Err(bad_form("'expect_set' takes an array of matchers"));
    };
    let mut items = Vec::with_capacity(matchers.len());
    for _ in matchers {
        match w.next()? {
            Next::Item(item) => {
                ack_item(w, obj, &item, bindings, schemas)?;
                items.push(item)
            }
            other => {
                return Err(failed(
                    &Value::Array(matchers.clone()),
                    json!({ "items": items, "then": describe_end(&other, "the set is incomplete") }),
                ))
            }
        }
    }
    let mut used = vec![false; items.len()];
    let mut attempt = bindings.clone();
    if assign(matchers, 0, &items, &mut used, &mut attempt, schemas)? {
        *bindings = attempt;
        Ok(())
    } else {
        Err(failed(&Value::Array(matchers.clone()), Value::Array(items)))
    }
}

/// Finds a one-to-one assignment of matchers to items, with consistent bindings.
fn assign(
    matchers: &[Value],
    at: usize,
    items: &[Value],
    used: &mut [bool],
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<bool, StepError> {
    if at == matchers.len() {
        return Ok(true);
    }
    for i in 0..items.len() {
        if used[i] {
            continue;
        }
        let mut attempt = bindings.clone();
        if try_match(&matchers[at], &items[i], &mut attempt, schemas)? {
            used[i] = true;
            if assign(matchers, at + 1, items, used, &mut attempt, schemas)? {
                *bindings = attempt;
                return Ok(true);
            }
            used[i] = false;
        }
    }
    Ok(false)
}

fn expect_eventually(
    w: &mut Waiter<'_>,
    obj: &Map<String, Value>,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<(), StepError> {
    let target = &obj["expect_eventually"];
    let ignore = matcher_list(obj, "ignore")?;
    loop {
        match w.next()? {
            Next::Item(item) => {
                if try_match(target, &item, bindings, schemas)? {
                    return ack_item(w, obj, &item, bindings, schemas);
                }
                let mut ignored = false;
                for matcher in ignore {
                    if try_match(matcher, &item, &mut bindings.clone(), schemas)? {
                        ignored = true;
                        break;
                    }
                }
                if !ignored {
                    return Err(failed(target, item));
                }
                ack_item(w, obj, &item, bindings, schemas)?;
            }
            other => return Err(failed(target, describe_end(&other, "no matching item"))),
        }
    }
}

fn expect_quiet_until(
    w: &mut Waiter<'_>,
    obj: &Map<String, Value>,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
    limits: &Limits,
) -> Result<(), StepError> {
    let forbidden = &obj["expect_quiet_until"];
    let (Some(barrier), Some(until)) = (obj.get("barrier"), obj.get("until")) else {
        return Err(bad_form("'expect_quiet_until' needs 'barrier' and 'until'"));
    };
    run_step(barrier, w.driver, bindings, schemas, limits)?;
    loop {
        match w.next()? {
            Next::Item(item) => {
                if try_match(forbidden, &item, &mut bindings.clone(), schemas)? {
                    return Err(failed(
                        &json!({ "quiet_until": until, "not": forbidden }),
                        item,
                    ));
                }
                if try_match(until, &item, bindings, schemas)? {
                    return Ok(());
                }
            }
            other => {
                return Err(failed(
                    until,
                    describe_end(&other, "the barrier answer never arrived"),
                ))
            }
        }
    }
}

fn expect_closed(
    w: &mut Waiter<'_>,
    obj: &Map<String, Value>,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<(), StepError> {
    let reason = &obj["expect_closed"];
    let ignore = matcher_list(obj, "ignore")?;
    let expected = json!({ "closed": reason });
    loop {
        match w.next()? {
            Next::Closed(got) => {
                if reason.is_null() {
                    return Ok(());
                }
                return matches(reason, Some(&got), bindings, schemas)
                    .map_err(|f| match_failed(&expected, &json!({ "closed": got }), &f));
            }
            Next::Item(item) => {
                let mut ignored = false;
                for matcher in ignore {
                    if try_match(matcher, &item, &mut bindings.clone(), schemas)? {
                        ignored = true;
                        break;
                    }
                }
                if !ignored {
                    return Err(failed(&expected, item));
                }
            }
            Next::Idle => return Err(failed(&expected, describe_end(&Next::Idle, "no close"))),
        }
    }
}
