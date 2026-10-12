//! Property tests and a fuzz harness (docs/tooling.md: proptest with test-strategy, and bolero).
//!
//! The generators come from the types: `Arbitrary` is derived under `cfg(test)`. A hand-written strategy stands only where a type has a value rule
//! (a name from a closed set, a 2^53 bound, base64 text, one scalar) or holds bytes. Every generated frame is checked against the checked-in schema
//! artifact with `jsonschema`, so the test is not its own oracle.
//! Clause: Codec A.1, A.4, A.5, TS-3, TB-L2, TR-3, BP-3.

use crate::bytes::{base64_encode, HexBytes, Op, Payload};
use crate::error::{DecodeError, ProtocolErrorCode};
use crate::frames::*;
use crate::input::*;
use crate::types::*;
use crate::wire::*;
use bytes::Bytes;
use proptest::collection::vec;
use proptest::prelude::*;
use serde_json::Value;
use test_strategy::proptest;

// ---- strategies for the types that have a value rule ----

/// An integer that travels as a JSON number (A.1): at most 2^53 − 1.
pub fn wire_int() -> impl Strategy<Value = u64> {
    0..=crate::strict::MAX_SAFE_INTEGER as u64
}

/// Standard base64 text of some bytes (A.4, A.5).
pub fn base64_text() -> impl Strategy<Value = String> {
    vec(any::<u8>(), 0..16).prop_map(|b| base64_encode(&b))
}

impl Arbitrary for Op {
    type Parameters = ();
    type Strategy = BoxedStrategy<Op>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        any::<u64>().prop_map(Op).boxed()
    }
}

impl Arbitrary for Payload {
    type Parameters = ();
    type Strategy = BoxedStrategy<Payload>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        vec(any::<u8>(), 0..64).prop_map(Payload::from).boxed()
    }
}

impl Arbitrary for NamedKey {
    type Parameters = ();
    type Strategy = BoxedStrategy<NamedKey>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        // An unknown name is refused as `unsupported` (A.5); that class has its own unit test.
        proptest::sample::select(named_key_names())
            .prop_map(NamedKey)
            .boxed()
    }
}

impl Arbitrary for Key {
    type Parameters = ();
    type Strategy = BoxedStrategy<Key>;
    fn arbitrary_with(_: ()) -> Self::Strategy {
        prop_oneof![
            any::<NamedKey>().prop_map(Key::Named),
            any::<char>().prop_map(|c| Key::Char(c.to_string())),
        ]
        .boxed()
    }
}

fn one_char(key: Key) -> Key {
    match key {
        Key::Named(_) => Key::Char("a".into()),
        c => c,
    }
}

/// Makes a generated input satisfy the field constraints of Core 5.1A that a type cannot state (`Input::check`), and a nonzero `op` where the frame needs one.
fn normalize(input: Input) -> Input {
    fn dedup(mods: Vec<Modifier>) -> Vec<Modifier> {
        let mut seen = Vec::new();
        for m in mods {
            if !seen.contains(&m) {
                seen.push(m);
            }
        }
        seen
    }
    let nonzero = |op: Op| if op.0 == 0 { Op(1) } else { op };
    match input {
        Input::Key {
            op,
            key,
            mods,
            event,
            text,
            shifted_key,
            base_layout_key,
        } => {
            let mut mods = dedup(mods);
            let shifted_key = shifted_key.map(one_char);
            if shifted_key.is_some() && !mods.contains(&Modifier::Shift) {
                mods.push(Modifier::Shift);
            }
            Input::Key {
                op,
                key,
                mods,
                event,
                text,
                shifted_key,
                base_layout_key: base_layout_key.map(one_char),
            }
        }
        Input::Mouse {
            op,
            action,
            button,
            row,
            col,
            x,
            y,
            mods,
            notches,
        } => {
            let wheel_button = matches!(
                button,
                MouseButton::WheelUp
                    | MouseButton::WheelDown
                    | MouseButton::WheelLeft
                    | MouseButton::WheelRight
            );
            let action = match (wheel_button, action) {
                (true, _) => MouseAction::Wheel,
                (false, MouseAction::Wheel) => MouseAction::Press,
                (false, a) => a,
            };
            let mods = dedup(mods)
                .into_iter()
                .filter(|m| matches!(m, Modifier::Shift | Modifier::Alt | Modifier::Ctrl))
                .collect();
            let notches = notches
                .filter(|_| action == MouseAction::Wheel)
                .map(|n| n.max(1));
            Input::Mouse {
                op,
                action,
                button,
                row,
                col,
                x,
                y,
                mods,
                notches,
            }
        }
        Input::Resize { op, rows, cols } => Input::Resize {
            op,
            rows: rows.max(1),
            cols: cols.max(1),
        },
        Input::ReplyTerminalQuery {
            op,
            query_id,
            reply: QueryReply::Position { x, y },
        } => Input::ReplyTerminalQuery {
            op,
            query_id,
            reply: QueryReply::Position {
                x: x % 65536,
                y: y % 65536,
            },
        },
        Input::PasteBegin {
            op,
            total_bytes,
            require_bracketed,
        } => Input::PasteBegin {
            op: nonzero(op),
            total_bytes,
            require_bracketed,
        },
        Input::PasteCommit { op, want_result } => Input::PasteCommit {
            op: nonzero(op),
            want_result,
        },
        Input::PasteAbort { op } => Input::PasteAbort { op: nonzero(op) },
        Input::FileBegin {
            op,
            name,
            mime,
            total_bytes,
        } => Input::FileBegin {
            op: nonzero(op),
            name,
            mime,
            total_bytes,
        },
        Input::FileCommit { op } => Input::FileCommit { op: nonzero(op) },
        Input::FileAbort { op } => Input::FileAbort { op: nonzero(op) },
        other => other,
    }
}

pub fn valid_input() -> impl Strategy<Value = Input> {
    any::<Input>().prop_map(normalize)
}

/// An `output_dropped` that follows TS-4c: at least one notification, and an empty title or body is omitted.
fn valid_dropped(mut o: OutputDropped) -> OutputDropped {
    o.last.title = o.last.title.filter(|t| !t.is_empty());
    o.last.body = o.last.body.filter(|t| !t.is_empty());
    o
}

// ---- the checked-in schemas are the oracle ----

fn artifact(file: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/hub-client/route")
        .join(file);
    serde_json::from_str(&std::fs::read_to_string(path).expect("artifact"))
        .expect("artifact parses")
}

fn validator(file: &'static str) -> &'static jsonschema::Validator {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<BTreeMap<&'static str, &'static jsonschema::Validator>>> =
        OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().expect("lock");
    cache.entry(file).or_insert_with(|| {
        Box::leak(Box::new(
            jsonschema::validator_for(&artifact(file)).expect("route schema compiles"),
        ))
    })
}

fn big() -> FrameBounds {
    FrameBounds {
        max_frame: 1 << 20,
        max_screen: 1 << 20,
        max_history: 1 << 20,
    }
}

fn in_schema(file: &'static str, value: &Value) -> Result<(), TestCaseError> {
    match validator(file).validate(value) {
        Ok(()) => Ok(()),
        Err(e) => Err(TestCaseError::fail(format!(
            "violates the checked-in {file}: {e}\n{value}"
        ))),
    }
}

/// A frame from the worker is valid under the checked-in schema and survives every delimiter (A.4, TS-3, TB-L2, TR-3).
fn check_client(frame: ToClient, type_byte: u8) -> Result<(), TestCaseError> {
    in_schema(
        "to-client.schema.json",
        &serde_json::to_value(&frame).unwrap(),
    )?;
    prop_assert_eq!(frame.type_byte(), type_byte);
    let wire = frame.encode_shared();
    prop_assert_eq!(wire[0], type_byte);
    prop_assert_eq!(&wire[..], &frame.encode()[..]);
    prop_assert_eq!(
        ToClient::decode(&wire, &big()).map_err(fail)?,
        Decoded::Frame(frame.clone())
    );
    prop_assert_eq!(
        ToClient::decode_shared(&wire, &big()).map_err(fail)?,
        Decoded::Frame(frame.clone())
    );
    check_delimiters(&wire)
}

fn check_worker(frame: ToWorker, type_byte: u8) -> Result<(), TestCaseError> {
    in_schema(
        "to-worker.schema.json",
        &serde_json::to_value(&frame).unwrap(),
    )?;
    prop_assert_eq!(frame.type_byte(), type_byte);
    let wire = Bytes::from(frame.encode());
    prop_assert_eq!(wire[0], type_byte);
    prop_assert_eq!(
        ToWorker::decode(&wire, &big()).map_err(fail)?,
        Decoded::Frame(frame.clone())
    );
    prop_assert_eq!(
        ToWorker::decode_shared(&wire, &big()).map_err(fail)?,
        Decoded::Frame(frame)
    );
    check_delimiters(&wire)
}

fn fail(e: DecodeError) -> TestCaseError {
    TestCaseError::fail(format!("{e}"))
}

/// Both delimiters rebuild the frame (TB-L2, TR-3).
fn check_delimiters(wire: &Bytes) -> Result<(), TestCaseError> {
    let mut stream = StreamReader::new();
    stream.push(&stream_wrap(wire));
    prop_assert_eq!(stream.next_frame(&big()).map_err(fail)?, Some(wire.clone()));
    prop_assert_eq!(stream.pending(), 0);
    let mut chunks = ChunkReader::new();
    let messages = chunk_wrap(wire, &[7]);
    let (last, rest) = messages.split_last().unwrap();
    for m in rest {
        prop_assert_eq!(chunks.push_message(m, &big()).map_err(fail)?, None);
    }
    prop_assert_eq!(
        chunks.push_message(last, &big()).map_err(fail)?,
        Some(wire.clone())
    );
    Ok(())
}

macro_rules! property {
    ($(#[$m:meta])* fn $name:ident($($args:tt)*) $body:block) => {
        #[proptest(cases = 64, failure_persistence = Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct("proptest-regressions/props.txt"))))]
        $(#[$m])*
        fn $name($($args)*) $body
    };
}

macro_rules! client_frame {
    ($name:ident, $ty:ty, $variant:path, $t:expr) => {
        property! { fn $name(body: $ty) { check_client($variant(body), $t)?; } }
    };
}

// Codec A.4, TS-3: every frame from the worker round-trips with the type byte of A.4.
client_frame!(
    attached_round_trips,
    Attached,
    ToClient::Attached,
    TYPE_ATTACHED
);
client_frame!(
    baseline_begin_round_trips,
    BaselineBegin,
    ToClient::BaselineBegin,
    TYPE_BASELINE_BEGIN
);
client_frame!(
    baseline_end_round_trips,
    BaselineEnd,
    ToClient::BaselineEnd,
    TYPE_BASELINE_END
);
client_frame!(modes_round_trips, Modes, ToClient::Modes, TYPE_MODES);
client_frame!(resync_round_trips, Resync, ToClient::Resync, TYPE_RESYNC);
client_frame!(
    input_refused_round_trips,
    InputRefused,
    ToClient::InputRefused,
    TYPE_INPUT_REFUSED
);
client_frame!(
    input_done_round_trips,
    InputDone,
    ToClient::InputDone,
    TYPE_INPUT_DONE
);
client_frame!(
    terminal_query_round_trips,
    TerminalQuery,
    ToClient::TerminalQuery,
    TYPE_TERMINAL_QUERY
);

property! {
    fn output_dropped_round_trips(body: OutputDropped) {
        check_client(ToClient::OutputDropped(valid_dropped(body)), TYPE_OUTPUT_DROPPED)?;
    }
}

property! {
    fn route_closed_round_trips(mut body: RouteClosed) {
        // `exit` is present only when the reason is `session_ended` (TS-8).
        if body.reason != CloseReason::SessionEnded {
            body.exit = None;
        }
        check_client(ToClient::RouteClosed(body), TYPE_ROUTE_CLOSED)?;
    }
}

property! {
    fn data_frames_round_trip_raw(payload: Payload) {
        // BP-3: the body is the bytes, with no JSON around them.
        for (frame, t) in [
            (ToClient::Screen { payload: payload.clone() }, TYPE_SCREEN),
            (ToClient::History { payload: payload.clone() }, TYPE_HISTORY),
            (ToClient::Output { payload: payload.clone() }, TYPE_OUTPUT),
        ] {
            prop_assert_eq!(&frame.encode()[1..], &payload[..]);
            check_client(frame, t)?;
        }
    }
}

property! {
    fn live_round_trips(_unit: ()) {
        check_client(ToClient::Live, TYPE_LIVE)?;
    }
}

property! {
    fn output_deflate_round_trips_and_returns_the_inflated_payload(raw: Payload) {
        // The payload of the frame is a DEFLATE stream (A.4 0x0E); the decoder returns what it inflated.
        let frame = ToClient::output_deflate(Payload::from(deflate_output(&raw)));
        check_client(frame.clone(), TYPE_OUTPUT_DEFLATE)?;
        for decoded in [
            ToClient::decode_shared(&frame.encode_shared(), &big()).map_err(fail)?,
            ToClient::decode(&frame.encode(), &big()).map_err(fail)?,
        ] {
            let Decoded::Frame(got) = decoded else { return Err(TestCaseError::fail("a frame")) };
            prop_assert_eq!(got.inflated().map(|b| &b[..]), Some(&raw[..]));
            prop_assert_eq!(&got, &frame);
        }
    }
}

// Codec A.4, A.5, TS-3: every frame from the client round-trips with the type byte of A.4.
property! {
    fn input_round_trips(#[strategy(valid_input())] input: Input) {
        prop_assert_eq!(input.check(), Ok(()));
        check_worker(ToWorker::Input(input), TYPE_INPUT)?;
    }
}

property! {
    fn paste_chunk_round_trips(op: Op, bytes: Payload) {
        let op = if op.0 == 0 { Op(1) } else { op };
        check_worker(ToWorker::PasteChunk { op, bytes }, TYPE_PASTE_CHUNK)?;
    }
}

property! {
    fn file_chunk_round_trips(op: Op, bytes: Payload) {
        let op = if op.0 == 0 { Op(1) } else { op };
        check_worker(ToWorker::FileChunk { op, bytes }, TYPE_FILE_CHUNK)?;
    }
}

property! {
    fn bytes_round_trips(op: Op, bytes: Payload) {
        check_worker(ToWorker::Bytes { op, bytes }, TYPE_BYTES)?;
    }
}

property! {
    fn text_round_trips(op: Op, text: String) {
        check_worker(ToWorker::Text { op, text }, TYPE_TEXT)?;
    }
}

property! {
    /// A payload in JSON is `{"$bytes_hex": <lowercase hex>}`, for both byte types (A.4, BP-3). The hex is built here without the codec.
    fn bytes_hex_json_round_trips(raw: Vec<u8>) {
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let json = serde_json::json!({ "$bytes_hex": hex });
        prop_assert_eq!(serde_json::to_value(Payload::from(raw.clone())).unwrap(), json.clone());
        prop_assert_eq!(serde_json::to_value(HexBytes(raw.clone())).unwrap(), json.clone());
        prop_assert_eq!(serde_json::from_value::<Payload>(json.clone()).unwrap(), Payload::from(raw.clone()));
        prop_assert_eq!(serde_json::from_value::<HexBytes>(json).unwrap(), HexBytes(raw.clone()));
        prop_assert_eq!(Payload::from(raw.clone()).to_hex(), hex.clone());
        prop_assert_eq!(HexBytes(raw).to_hex(), hex);
    }
}

// ---- delimiters (TB-L2, TR-3) ----

property! {
    /// Any split of a byte stream gives the same frames (TB-L2).
    fn a_stream_gives_the_same_frames_for_any_split(
        #[strategy(vec(vec(any::<u8>(), 1..40), 0..5))] frames: Vec<Vec<u8>>,
        #[strategy(vec(1usize..17, 1..8))] splits: Vec<usize>,
    ) {
        let stream: Vec<u8> = frames.iter().flat_map(|f| stream_wrap(f)).collect();
        let mut reader = StreamReader::new();
        let mut got: Vec<Vec<u8>> = Vec::new();
        let mut rest = &stream[..];
        let mut i = 0;
        while !rest.is_empty() {
            let n = splits[i % splits.len()].min(rest.len());
            reader.push(&rest[..n]);
            rest = &rest[n..];
            i += 1;
            // Bounded: a reader that never returns `None` fails the property and does not hang it.
            for _ in 0..=frames.len() {
                let Some(f) = reader.next_frame(&big()).map_err(fail)? else { break };
                got.push(f.to_vec());
            }
        }
        prop_assert_eq!(got, frames);
        prop_assert_eq!(reader.pending(), 0);
    }
}

property! {
    /// Any chunk sizes give the same frame (TR-3).
    fn chunks_of_any_size_give_the_same_frame(
        #[strategy(vec(any::<u8>(), 1..80))] frame: Vec<u8>,
        #[strategy(vec(1usize..20, 1..6))] sizes: Vec<usize>,
    ) {
        let mut reader = ChunkReader::new();
        let messages = chunk_wrap(&frame, &sizes);
        let (last, rest) = messages.split_last().unwrap();
        for m in rest {
            prop_assert_eq!(reader.push_message(m, &big()).map_err(fail)?, None);
        }
        prop_assert_eq!(reader.push_message(last, &big()).map_err(fail)?.map(|b| b.to_vec()), Some(frame));
    }
}

// ---- decoder robustness on arbitrary bytes (A.1, A.4) ----

const TYPES: [u8; 20] = [
    TYPE_ATTACHED,
    TYPE_BASELINE_BEGIN,
    TYPE_SCREEN,
    TYPE_HISTORY,
    TYPE_BASELINE_END,
    TYPE_LIVE,
    TYPE_OUTPUT,
    TYPE_MODES,
    TYPE_RESYNC,
    TYPE_INPUT_REFUSED,
    TYPE_ROUTE_CLOSED,
    TYPE_TERMINAL_QUERY,
    TYPE_INPUT_DONE,
    TYPE_OUTPUT_DEFLATE,
    TYPE_OUTPUT_DROPPED,
    TYPE_INPUT,
    TYPE_PASTE_CHUNK,
    TYPE_BYTES,
    TYPE_TEXT,
    TYPE_FILE_CHUNK,
];

/// Arbitrary bytes, bytes with a known type byte, and a valid frame with one mutation (the bytes that a fuzzer finds first).
fn wire_bytes() -> impl Strategy<Value = Vec<u8>> {
    let known = (
        proptest::sample::select(TYPES.to_vec()),
        vec(any::<u8>(), 0..60),
    )
        .prop_map(|(t, mut b)| {
            b.insert(0, t);
            b
        });
    let mutated = (
        prop_oneof![
            any::<crate::frames::Resync>().prop_map(|b| ToClient::Resync(b).encode()),
            any::<Attached>().prop_map(|b| ToClient::Attached(b).encode()),
            any::<InputRefused>().prop_map(|b| ToClient::InputRefused(b).encode()),
            valid_input().prop_map(|i| ToWorker::Input(i).encode()),
        ],
        any::<proptest::sample::Index>(),
        any::<u8>(),
        0u8..3,
    )
        .prop_map(|(mut frame, at, byte, how)| {
            let i = at.index(frame.len());
            match how {
                0 => frame[i] = byte,
                1 => frame.truncate(i),
                _ => frame.insert(i, byte),
            }
            frame
        });
    prop_oneof![vec(any::<u8>(), 0..120), known, mutated]
}

/// A refusal is typed: its code is the one of its detail, and one of the codes that a decoder gives (A.1).
fn check_refusal(e: &DecodeError) -> Result<(), TestCaseError> {
    prop_assert_eq!(e.code, e.detail.code());
    prop_assert!(matches!(
        e.code,
        ProtocolErrorCode::FrameTooLarge
            | ProtocolErrorCode::MalformedFrame
            | ProtocolErrorCode::UnknownFrame
            | ProtocolErrorCode::InvalidInput
    ));
    Ok(())
}

fn check_decoded_client(bytes: &[u8], bounds: &FrameBounds) -> Result<(), TestCaseError> {
    match ToClient::decode(bytes, bounds) {
        Ok(Decoded::Frame(frame)) => {
            // What decodes also encodes, and decodes again to the same frame.
            let again = ToClient::decode(&frame.encode(), &big()).map_err(fail)?;
            prop_assert_eq!(again, Decoded::Frame(frame));
        }
        Ok(Decoded::Ignored { type_byte }) => prop_assert!(type_byte & IGNORABLE_BIT != 0),
        Ok(other) => {
            return Err(TestCaseError::fail(format!(
                "a worker frame gave {other:?}"
            )))
        }
        Err(e) => check_refusal(&e)?,
    }
    Ok(())
}

fn check_decoded_worker(bytes: &[u8], bounds: &FrameBounds) -> Result<(), TestCaseError> {
    match ToWorker::decode(bytes, bounds) {
        Ok(Decoded::Frame(frame)) => {
            let again = ToWorker::decode(&frame.encode(), &big()).map_err(fail)?;
            prop_assert_eq!(again, Decoded::Frame(frame));
        }
        Ok(Decoded::Ignored { type_byte }) => prop_assert!(type_byte & IGNORABLE_BIT != 0),
        Ok(Decoded::Unsupported { .. } | Decoded::Invalid { .. }) => {}
        Err(e) => check_refusal(&e)?,
    }
    Ok(())
}

/// A stream reader fed `stream`, and a chunk reader fed `messages`: neither panics, a refusal is typed, and every frame that a
/// reader returns is decoded under the same checks (the readers and the decoders in one harness).
fn drain_readers(
    stream: &[u8],
    messages: &[Vec<u8>],
    bounds: &FrameBounds,
) -> Result<(), TestCaseError> {
    let mut reader = StreamReader::new();
    reader.push(stream);
    // A frame takes at least 5 bytes of the stream, so more reads than that is a reader that never ends.
    for _ in 0..=stream.len() / 5 + 1 {
        match reader.next_frame(bounds) {
            Ok(Some(frame)) => {
                prop_assert!(!frame.is_empty());
                prop_assert!(frame.len() as u64 <= bound_of(frame[0], bounds));
                check_decoded_client(&frame, bounds)?;
                check_decoded_worker(&frame, bounds)?;
            }
            Ok(None) => break,
            Err(e) => {
                check_refusal(&e)?;
                break;
            }
        }
    }
    let mut chunks = ChunkReader::with_max_chunk(64);
    for message in messages {
        match chunks.push_message(message, bounds) {
            Ok(Some(frame)) => {
                check_decoded_client(&frame, bounds)?;
                check_decoded_worker(&frame, bounds)?;
            }
            Ok(None) => {}
            Err(e) => {
                check_refusal(&e)?;
                break;
            }
        }
    }
    Ok(())
}

/// The bytes as received (garbage to a reader), and the bytes as one frame under each delimiter (a frame to a reader).
fn check_readers(bytes: &[u8], bounds: &FrameBounds) -> Result<(), TestCaseError> {
    let raw: Vec<Vec<u8>> = bytes.chunks(9).map(<[u8]>::to_vec).collect();
    drain_readers(bytes, &raw, bounds)?;
    if !bytes.is_empty() {
        drain_readers(&stream_wrap(bytes), &chunk_wrap(bytes, &[5]), bounds)?;
    }
    Ok(())
}

property! {
    fn the_decoders_never_panic_and_every_refusal_is_typed(
        #[strategy(wire_bytes())] bytes: Vec<u8>,
        #[strategy(1u64..200)] max_frame: u64,
    ) {
        let bounds = FrameBounds { max_frame, max_screen: max_frame * 2, max_history: max_frame + 10 };
        check_decoded_client(&bytes, &bounds)?;
        check_decoded_worker(&bytes, &bounds)?;
        check_readers(&bytes, &bounds)?;
    }
}

// ---- the bolero harness (a property test under `cargo test`; the fuzz job names this target) ----

/// A violated invariant fails the fuzz run: the harness panics, and bolero reports the input.
fn must(r: Result<(), TestCaseError>) {
    if let Err(e) = r {
        panic!("route decoder invariant violated: {e}");
    }
}

#[test]
#[should_panic(expected = "route decoder invariant violated")]
fn a_failed_check_fails_the_fuzz_harness() {
    must(Err(TestCaseError::fail("probe")));
}

/// Fuzz target `route_frame_decoder`: the stream reader, both frame decoders and both delimiters on arbitrary bytes.
#[test]
fn bolero_route_frame_decoder() {
    bolero::check!()
        .with_iterations(256)
        .for_each(|bytes: &[u8]| {
            for bounds in [
                FrameBounds {
                    max_frame: 64,
                    max_screen: 128,
                    max_history: 96,
                },
                big(),
            ] {
                must(check_decoded_client(bytes, &bounds));
                must(check_decoded_worker(bytes, &bounds));
                must(check_readers(bytes, &bounds));
            }
        });
}

/// The encoding before the lookup table: one `format!` per byte. The tests below hold the table to it.
fn hex_by_format(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The decoding before the digit table: `char::to_digit(16)` per digit.
fn hex_decode_by_to_digit(text: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return Err("odd number of hex digits".to_string());
    }
    digits
        .chunks(2)
        .map(|pair| {
            let hi = (pair[0] as char).to_digit(16);
            let lo = (pair[1] as char).to_digit(16);
            match (hi, lo) {
                (Some(h), Some(l)) => Ok((h * 16 + l) as u8),
                _ => Err("not a hex digit".to_string()),
            }
        })
        .collect()
}

/// BP-3: the table's hex is byte-identical to the `format!` hex for each of the 256 byte values, alone and all together.
#[test]
fn hex_encode_is_the_format_encoding_for_every_byte() {
    let all: Vec<u8> = (0..=255).collect();
    for b in &all {
        assert_eq!(crate::bytes::hex_encode(&[*b]), format!("{b:02x}"));
    }
    assert_eq!(crate::bytes::hex_encode(&all), hex_by_format(&all));
    assert_eq!(crate::bytes::hex_encode(&[]), "");
}

/// BP-3: the digit table decodes as `to_digit(16)` did: every byte in either case, every pair of the 256 characters, and the
/// errors (an odd count, a non-hex character), with spaces ignored.
#[test]
fn hex_decode_is_the_to_digit_decoding_for_every_pair() {
    for hi in 0..=255u8 {
        for lo in 0..=255u8 {
            let (hi, lo) = (char::from(hi), char::from(lo));
            let text: String = [hi, lo].iter().collect();
            assert_eq!(
                crate::bytes::hex_decode(&text),
                hex_decode_by_to_digit(&text),
                "{text:?}"
            );
        }
    }
    let all: Vec<u8> = (0..=255).collect();
    let lower = hex_by_format(&all);
    assert_eq!(crate::bytes::hex_decode(&lower).unwrap(), all);
    assert_eq!(
        crate::bytes::hex_decode(&lower.to_uppercase()).unwrap(),
        all
    );
    for text in ["", "a", "0 1", "f\tF\n", "zz", "0g"] {
        assert_eq!(
            crate::bytes::hex_decode(text),
            hex_decode_by_to_digit(text),
            "{text:?}"
        );
    }
}

property! {
    /// BP-3: the table's hex equals the `format!` hex and decodes back to the same bytes.
    fn hex_table_round_trips(raw: Vec<u8>) {
        let hex = crate::bytes::hex_encode(&raw);
        prop_assert_eq!(&hex, &hex_by_format(&raw));
        prop_assert_eq!(crate::bytes::hex_decode(&hex).unwrap(), raw);
    }
}
