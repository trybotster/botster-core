//! Property tests and fuzz harnesses (docs/tooling.md: proptest with test-strategy, and bolero).
//!
//! The generators come from the types: `Arbitrary` is derived under `cfg(test)`. A hand-written strategy stands only where a type holds a free JSON value
//! or has a value rule (a name, a version, a 2^53 bound). Every generated value is checked against the checked-in schema artifact with `jsonschema`, so the
//! test is not its own oracle. Clause: HP WF-1 to WF-4, R-4, R-6.2, Appendix B, MN-1.

use crate::common::{ConstTrue, Epoch, WireU64, MAX_WIRE_INT};
use crate::frames::*;
use crate::limits::{LaneLimits, PluginLimits, LIMIT_SPECS};
use crate::manifest::*;
use crate::result::ErrorKind;
use crate::wire::*;
use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use serde_json::{json, Map, Value};
use test_strategy::proptest;

// ---- strategies for free JSON values and bounded integers ----

/// An integer that travels as a JSON number (HP WF-3).
pub fn wire_int() -> impl Strategy<Value = u64> {
    0..=MAX_WIRE_INT
}

/// A JSON value: integers within ±2^53, no floats (a float does not round-trip by text), strings of any scalar value.
pub fn value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-(MAX_WIRE_INT as i64)..=(MAX_WIRE_INT as i64)).prop_map(|n| json!(n)),
        "\\PC{0,6}".prop_map(Value::String),
    ];
    leaf.prop_recursive(2, 10, 3, |inner| {
        prop_oneof![
            vec(inner.clone(), 0..3).prop_map(Value::Array),
            btree_map("\\PC{0,6}", inner, 0..3)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

pub fn opt_value() -> impl Strategy<Value = Option<Value>> {
    proptest::option::of(value())
}

pub fn vec_value() -> impl Strategy<Value = Vec<Value>> {
    vec(value(), 0..3)
}

pub fn opt_vec_value() -> impl Strategy<Value = Option<Vec<Value>>> {
    proptest::option::of(vec_value())
}

/// An object with arbitrary keys.
pub fn object() -> impl Strategy<Value = Map<String, Value>> {
    btree_map("\\PC{0,6}", value(), 0..2).prop_map(|m| m.into_iter().collect())
}

pub fn opt_object() -> impl Strategy<Value = Option<Map<String, Value>>> {
    proptest::option::of(object())
}

pub fn opt_vec_object() -> impl Strategy<Value = Option<Vec<Map<String, Value>>>> {
    proptest::option::of(vec(object(), 0..3))
}

/// The open extension fields of a flattened object: their keys never collide with a typed field (`x_` prefix).
pub fn extra() -> impl Strategy<Value = Map<String, Value>> {
    btree_map("x_[a-z]{1,6}", value(), 0..2).prop_map(|m| m.into_iter().collect())
}

// ---- types with a value rule ----

impl Arbitrary for Epoch {
    type Parameters = ();
    type Strategy = BoxedStrategy<Epoch>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        // Both forms: a number up to 2^53 and a decimal string above.
        prop_oneof![wire_int(), any::<u64>()]
            .prop_map(Epoch)
            .boxed()
    }
}

impl Arbitrary for WireU64 {
    type Parameters = ();
    type Strategy = BoxedStrategy<WireU64>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        prop_oneof![wire_int(), any::<u64>()]
            .prop_map(WireU64)
            .boxed()
    }
}

impl Arbitrary for ConstTrue {
    type Parameters = ();
    type Strategy = Just<ConstTrue>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        Just(ConstTrue)
    }
}

impl Arbitrary for ErrorKind {
    type Parameters = ();
    type Strategy = BoxedStrategy<ErrorKind>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        // The schema lists the closed set; the tolerance for an unknown kind (ER-1) has its own unit test.
        proptest::sample::select(ErrorKind::ALL.to_vec()).boxed()
    }
}

impl Arbitrary for PluginName {
    type Parameters = ();
    type Strategy = BoxedStrategy<PluginName>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        "[a-z0-9-]{1,12}".prop_map(PluginName).boxed()
    }
}

impl Arbitrary for SemVer {
    type Parameters = ();
    type Strategy = BoxedStrategy<SemVer>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        (
            0u32..1000,
            0u32..1000,
            0u32..1000,
            proptest::option::of("[a-z][a-z0-9]{0,4}"),
        )
            .prop_map(|(a, b, c, pre)| {
                SemVer(match pre {
                    Some(p) => format!("{a}.{b}.{c}-{p}"),
                    None => format!("{a}.{b}.{c}"),
                })
            })
            .boxed()
    }
}

impl Arbitrary for ProvidedInterface {
    type Parameters = ();
    type Strategy = BoxedStrategy<ProvidedInterface>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        ("[a-z0-9-]{1,10}", 0u32..100, 0u32..100, 0u32..100)
            .prop_map(|(n, a, b, c)| ProvidedInterface(format!("{n}@{a}.{b}.{c}")))
            .boxed()
    }
}

impl Arbitrary for Capability {
    type Parameters = ();
    type Strategy = BoxedStrategy<Capability>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        (
            prop_oneof![Just("net".to_string()), "[a-z_]{1,10}"],
            proptest::option::of(any::<String>()),
            proptest::option::of(any::<bool>()),
            proptest::option::of(vec(any::<String>(), 0..3)),
        )
            .prop_map(|(surface, scope, optional, allow)| {
                // `allow` is only valid on surface `net` (HP MN-1).
                let allow = if surface == "net" { allow } else { None };
                Capability {
                    surface,
                    scope,
                    optional,
                    allow,
                }
            })
            .boxed()
    }
}

impl Arbitrary for PluginLimits {
    type Parameters = ();
    type Strategy = BoxedStrategy<PluginLimits>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        let numeric = LIMIT_SPECS.len();
        (
            vec(wire_int(), numeric),
            any::<LaneLimits>(),
            any::<String>(),
            any::<[u16; 2]>(),
            (
                any::<String>(),
                any::<String>(),
                any::<String>(),
                any::<String>(),
            ),
            vec(proptest::option::of(wire_int()), 6),
        )
            .prop_map(|(nums, lanes, probe, ports, (a, b, c, d), opts)| {
                let mut m = Map::new();
                for (spec, n) in LIMIT_SPECS.iter().zip(nums) {
                    m.insert(spec.name.to_string(), json!(n));
                }
                m.insert("lanes".into(), serde_json::to_value(lanes).unwrap());
                m.insert("probe_text".into(), json!(probe));
                m.insert("session_port_range".into(), json!(ports));
                m.insert("target_plugin_dir".into(), json!(a));
                m.insert("process_path".into(), json!(b));
                m.insert("net_listen".into(), json!(c));
                m.insert("assets_listen".into(), json!(d));
                for (name, v) in [
                    "cpu_seconds",
                    "open_files",
                    "processes",
                    "address_space_bytes",
                    "stderr_bytes",
                    "instruction_budget",
                ]
                .into_iter()
                .zip(opts)
                {
                    match v {
                        Some(n) => m.insert(name.into(), json!(n)),
                        None => m.remove(name),
                    };
                }
                serde_json::from_value(Value::Object(m)).expect("the generated limits parse")
            })
            .boxed()
    }
}

// ---- the checked-in artifacts are the oracle ----

fn artifact(file: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/plugin")
        .join(file);
    serde_json::from_str(&std::fs::read_to_string(path).expect("artifact"))
        .expect("artifact parses")
}

/// A validator compiles once per definition: compiling a schema in every case would dominate the run time.
fn frame_validator(name: &str) -> &'static jsonschema::Validator {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<BTreeMap<String, &'static jsonschema::Validator>>> =
        OnceLock::new();
    static DEFS: OnceLock<Value> = OnceLock::new();
    let defs = DEFS.get_or_init(|| artifact("plugin-frames.schema.json")["$defs"].clone());
    let mut cache = CACHE.get_or_init(Default::default).lock().expect("lock");
    cache.entry(name.to_string()).or_insert_with(|| {
        let schema = json!({"$ref": format!("#/$defs/{name}"), "$defs": defs});
        Box::leak(Box::new(
            jsonschema::validator_for(&schema).expect("frame schema compiles"),
        ))
    })
}

fn manifest_validator() -> &'static jsonschema::Validator {
    static V: std::sync::OnceLock<jsonschema::Validator> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        jsonschema::validator_for(&artifact("plugin-manifest.schema.json"))
            .expect("manifest schema compiles")
    })
}

fn frame_def_name(f: &Frame) -> &'static str {
    match f {
        Frame::Hello(_) => "Hello",
        Frame::Ready(_) => "Ready",
        Frame::Activated(_) => "Activated",
        Frame::Shutdown(_) => "Shutdown",
        Frame::Fault(_) => "Fault",
        Frame::Cancel(_) => "Cancel",
        Frame::Invoke(_) => "Invoke",
        Frame::Result(_) => "ResultFrame",
        Frame::HostCall(_) => "HostCall",
        Frame::HostResult(_) => "HostResult",
    }
}

const FB: u32 = u32::MAX;

/// A generated frame is valid under the checked-in schema, and it round-trips on every lane it may use (HP section 2, WF-1, WF-2, Appendix B).
fn check_frame(frame: Frame, lanes: &[Lane]) -> Result<(), TestCaseError> {
    let payload = frame.payload_value();
    let v = frame_validator(frame_def_name(&frame));
    if let Err(e) = v.validate(&payload) {
        return Err(TestCaseError::fail(format!(
            "{} violates the checked-in schema: {e}\n{payload}",
            frame_def_name(&frame)
        )));
    }
    for lane in lanes {
        let bytes = encode(*lane, &frame, FB).map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
        match decode(&bytes, FB, Some(frame.epoch())) {
            Ok(Decoded::Frame {
                frame: got,
                lane: l,
                used,
            }) => {
                prop_assert_eq!(&got, &frame);
                prop_assert_eq!(l, *lane);
                prop_assert_eq!(used, bytes.len());
            }
            other => return Err(TestCaseError::fail(format!("decode gave {other:?}"))),
        }
        // A frame of another epoch is dropped (R-6.2 step 3), whatever it holds.
        let other = Epoch(frame.epoch().0 ^ 1);
        match decode(&bytes, FB, Some(other)) {
            Ok(Decoded::Stale { epoch, .. }) => prop_assert_eq!(epoch, frame.epoch()),
            other => return Err(TestCaseError::fail(format!("a stale frame gave {other:?}"))),
        }
    }
    Ok(())
}

macro_rules! frame_property {
    ($name:ident, $ty:ty, $wrap:expr, $lanes:expr) => {
        #[proptest(cases = 64, failure_persistence = Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("proptest-regressions/props.txt"))))]
        fn $name(value: $ty) {
            check_frame($wrap(value), $lanes)?;
        }
    };
}

frame_property!(hello_round_trips, Hello, Frame::Hello, &[Lane::Control]);
frame_property!(ready_round_trips, Ready, Frame::Ready, &[Lane::Control]);
frame_property!(
    activated_round_trips,
    Activated,
    Frame::Activated,
    &[Lane::Control]
);
frame_property!(
    shutdown_round_trips,
    Shutdown,
    Frame::Shutdown,
    &[Lane::Control]
);
frame_property!(fault_round_trips, Fault, Frame::Fault, &[Lane::Control]);
frame_property!(cancel_round_trips, Cancel, Frame::Cancel, &[Lane::Control]);
frame_property!(invoke_round_trips, Invoke, Frame::Invoke, &[Lane::Invoke]);
frame_property!(
    result_round_trips,
    ResultFrame,
    Frame::Result,
    &[Lane::Invoke]
);
frame_property!(
    host_result_round_trips,
    HostResult,
    Frame::HostResult,
    &[Lane::Call, Lane::Bulk]
);

#[proptest(cases = 64, failure_persistence = Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("proptest-regressions/props.txt"))))]
fn host_call_round_trips_on_the_lane_of_its_call_class(mut call: HostCall, bulk: bool) {
    // R-4 P1b: log.write is bulk-class and every other call uses lane 2.
    if bulk {
        call.call = BULK_CALL.to_string();
    } else if call.call == BULK_CALL {
        call.call.push('x');
    }
    let lane = if bulk { Lane::Bulk } else { Lane::Call };
    check_frame(Frame::HostCall(call), &[lane])?;
}

#[proptest(cases = 64, failure_persistence = Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("proptest-regressions/props.txt"))))]
fn manifest_round_trips_and_validates(manifest: Manifest) {
    let value = serde_json::to_value(&manifest).unwrap();
    if let Err(e) = manifest_validator().validate(&value) {
        return Err(TestCaseError::fail(format!(
            "the manifest violates the checked-in schema: {e}\n{value}"
        )));
    }
    let text = serde_json::to_vec(&value).unwrap();
    let back: Manifest =
        serde_json::from_slice(&text).map_err(|e| TestCaseError::fail(format!("{e}\n{value}")))?;
    prop_assert_eq!(back, manifest);
}

// ---- decoder robustness on arbitrary bytes (R-6.2) ----

/// Every outcome of `decode` is one typed step of R-6.2, and the bytes it names are consistent with the input.
fn check_outcome(
    buf: &[u8],
    frame_bytes: u32,
    current: Option<Epoch>,
) -> Result<(), TestCaseError> {
    let res = decode(buf, frame_bytes, current);
    let len = (buf.len() >= 4).then(|| u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]));
    match res {
        Ok(Decoded::Incomplete { needed }) => {
            prop_assert!(needed >= 1);
            prop_assert!(
                len.is_none_or(|l| l <= frame_bytes && l >= 2),
                "an over-limit or short len is refused, not awaited"
            );
        }
        Ok(
            Decoded::Frame { used, .. }
            | Decoded::Stale { used, .. }
            | Decoded::IgnoredAdvisory { used, .. }
            | Decoded::UnknownStateChanging { used, .. },
        ) => {
            let l = len.expect("a whole frame has a header");
            prop_assert_eq!(used, 4 + l as usize);
            prop_assert!(used <= buf.len());
        }
        Err(WireError::LenOverLimit { len: l, limit }) => {
            prop_assert_eq!(Some(l), len);
            prop_assert!(l > limit);
        }
        Err(WireError::LenTooShort { len: l }) => prop_assert!(l < 2),
        Err(WireError::UnknownLane { lane }) => prop_assert!(lane > 3),
        Err(WireError::WrongLane { lane, .. }) => prop_assert!(lane <= 3),
        Err(_) => {}
    }
    Ok(())
}

#[proptest(cases = 64, failure_persistence = Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("proptest-regressions/props.txt"))))]
fn the_decoder_never_panics_on_arbitrary_bytes(
    #[strategy(vec(any::<u8>(), 0..300))] bytes: Vec<u8>,
    frame_bytes: u32,
    current: Option<Epoch>,
) {
    check_outcome(&bytes, frame_bytes, current)?;
}

#[proptest(cases = 64, failure_persistence = Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("proptest-regressions/props.txt"))))]
fn the_decoder_follows_the_r6_order_on_a_valid_header(
    lane: u8,
    ty: u8,
    #[strategy(vec(any::<u8>(), 0..120))] payload: Vec<u8>,
    current: Option<Epoch>,
) {
    // A well-formed header with an arbitrary body: a malformed body is refused whatever the type (R-6.2 step 2).
    let bytes = encode_raw(lane, ty, &payload, FB).unwrap();
    check_outcome(&bytes, FB, current)?;
    let is_object_with_epoch = std::str::from_utf8(&payload)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .is_some_and(|v| v.get("epoch").is_some());
    if !is_object_with_epoch {
        prop_assert!(
            decode(&bytes, FB, current).is_err(),
            "a body that is not one JSON object with an epoch is refused for every type"
        );
    }
}

// ---- bolero harnesses (run as property tests in the default tier; the fuzz job names these targets) ----

/// A violated invariant fails the fuzz run: the harness panics, and bolero reports the input.
fn must(r: Result<(), TestCaseError>) {
    if let Err(e) = r {
        panic!("decoder invariant violated: {e}");
    }
}

#[test]
#[should_panic(expected = "decoder invariant violated")]
fn a_failed_check_fails_the_fuzz_harness() {
    must(Err(TestCaseError::fail("probe")));
}

/// Fuzz target `plugin_frame_decoder`.
#[test]
fn bolero_plugin_frame_decoder() {
    bolero::check!()
        .with_iterations(256)
        .for_each(|bytes: &[u8]| {
            for current in [None, Some(Epoch(0)), Some(Epoch(7))] {
                must(check_outcome(bytes, 4096, current));
                must(check_outcome(bytes, FB, current));
            }
        });
}

/// Fuzz target `plugin_manifest_parser`: the parser never panics, and a manifest that parses is stable under a second round and valid under the checked-in schema.
#[test]
fn bolero_plugin_manifest_parser() {
    let schema = artifact("plugin-manifest.schema.json");
    let v = std::panic::AssertUnwindSafe(
        jsonschema::validator_for(&schema).expect("manifest schema compiles"),
    );
    bolero::check!()
        .with_iterations(256)
        .for_each(|bytes: &[u8]| {
            if let Ok(m) = serde_json::from_slice::<Manifest>(bytes) {
                let value = serde_json::to_value(&m).unwrap();
                assert!(
                    v.is_valid(&value),
                    "a parsed manifest violates the checked-in schema: {value}"
                );
                let again: Manifest =
                    serde_json::from_value(value).expect("a parsed manifest parses again");
                assert_eq!(again, m);
            }
        });
}
