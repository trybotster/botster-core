//! The input shapes of the `0x81 input` frame (Codec A.5). The semantic model is Core 5.1A, with Core's field and value names.

use crate::bytes::{parse_op, Op};
use crate::error::{DecodeDetail, DecodeError};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;

/// The closed 0.1 set of Core's `NamedKey` names (Core 5.1A).
///
/// Clause: Codec A.5, Core IN-9.
pub fn named_key_names() -> Vec<String> {
    let mut names: Vec<String> = [
        "enter",
        "tab",
        "backspace",
        "escape",
        "insert",
        "delete",
        "home",
        "end",
        "page_up",
        "page_down",
        "arrow_up",
        "arrow_down",
        "arrow_left",
        "arrow_right",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    names.extend((1..=35).map(|n| format!("f{n}")));
    names.extend((0..=9).map(|n| format!("kp_{n}")));
    names.extend(
        [
            "kp_decimal",
            "kp_divide",
            "kp_multiply",
            "kp_subtract",
            "kp_add",
            "kp_enter",
            "kp_equal",
            "caps_lock",
            "scroll_lock",
            "num_lock",
            "print_screen",
            "pause",
            "menu",
            "left_shift",
            "left_control",
            "left_alt",
            "left_super",
            "right_shift",
            "right_control",
            "right_alt",
            "right_super",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    names
}

/// A named key. The name is one of [`named_key_names`]. A name outside the set is refused as `unsupported` and never ends the route.
///
/// Clause: Codec A.5, Core 5.1A.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedKey(pub String);

impl NamedKey {
    /// Clause: Codec A.5, Core IN-9.
    pub fn is_known(&self) -> bool {
        named_key_names().contains(&self.0)
    }
}

impl JsonSchema for NamedKey {
    fn schema_name() -> Cow<'static, str> {
        "NamedKey".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "enum": named_key_names() })
    }
}

/// A key: `{"named": <NamedKey>}` or `{"char": "<one scalar>"}`.
///
/// Clause: Codec A.5, Core 5.1A.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Named(NamedKey),
    Char(String),
}

/// A modifier of Core 5.1A.
///
/// Clause: Codec A.5, Core 5.1A.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Shift,
    Alt,
    Ctrl,
    Meta,
    Super,
    Hyper,
    CapsLock,
    NumLock,
}

/// The `event` of a key input.
///
/// Clause: Codec A.5, Core 5.1A.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyEvent {
    Press,
    Repeat,
    Release,
}

/// The `action` of a mouse input.
///
/// Clause: Codec A.5, Core 5.1A.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
    Press,
    Release,
    Move,
    Wheel,
}

/// The `button` of a mouse input. `action` is `wheel` exactly when the button is a `wheel_*` value.
///
/// Clause: Codec A.5, Core 5.1A.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    None,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

/// `color_scheme` reply values.
///
/// Clause: Codec A.5.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColorScheme {
    Dark,
    Light,
}

/// The `reply` of `reply_terminal_query`: the JSON projection of Core's `ReplyTerminalQuery`.
///
/// Clause: Codec A.5, P9.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryReply {
    Clipboard {
        selection: String,
        data_base64: String,
    },
    Pixels {
        width: u32,
        height: u32,
    },
    Chars {
        rows: u32,
        cols: u32,
    },
    WindowState {
        iconified: bool,
    },
    Position {
        x: u32,
        y: u32,
    },
    Text {
        text: String,
    },
    ColorScheme(ColorScheme),
    Encoded {
        bytes_base64: String,
    },
    Decline,
}

/// One input shape of the `0x81 input` frame. `op` is a decimal string, optional except on a paste and a file.
///
/// Clause: Codec A.5, TS-5, TS-5d, TS-5f.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    Key {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        op: Option<Op>,
        key: Key,
        #[serde(default)]
        #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Modifier>(), 0..3)))]
        mods: Vec<Modifier>,
        event: KeyEvent,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shifted_key: Option<Key>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_layout_key: Option<Key>,
    },
    Mouse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        op: Option<Op>,
        action: MouseAction,
        button: MouseButton,
        row: u32,
        col: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<u32>,
        #[serde(default)]
        #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Modifier>(), 0..3)))]
        mods: Vec<Modifier>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notches: Option<u32>,
    },
    Focus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        op: Option<Op>,
        focused: bool,
    },
    Resize {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        op: Option<Op>,
        rows: u32,
        cols: u32,
    },
    ReplyTerminalQuery {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        op: Option<Op>,
        query_id: String,
        reply: QueryReply,
    },
    PasteBegin {
        op: Op,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(test, strategy(proptest::option::of(crate::props::wire_int())))]
        total_bytes: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        require_bracketed: Option<bool>,
    },
    PasteCommit {
        op: Op,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        want_result: Option<bool>,
    },
    PasteAbort {
        op: Op,
    },
    FileBegin {
        op: Op,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(test, strategy(proptest::option::of(crate::props::wire_int())))]
        total_bytes: Option<u64>,
    },
    FileCommit {
        op: Op,
    },
    FileAbort {
        op: Op,
    },
}

impl Input {
    /// The `op` of the input, when it has one.
    ///
    /// Clause: Codec A.5, TS-5.
    pub fn op(&self) -> Option<Op> {
        match self {
            Input::Key { op, .. }
            | Input::Mouse { op, .. }
            | Input::Focus { op, .. }
            | Input::Resize { op, .. }
            | Input::ReplyTerminalQuery { op, .. } => *op,
            Input::PasteBegin { op, .. }
            | Input::PasteCommit { op, .. }
            | Input::PasteAbort { op }
            | Input::FileBegin { op, .. }
            | Input::FileCommit { op }
            | Input::FileAbort { op } => Some(*op),
        }
    }
}

impl Input {
    /// The field constraints of Core 5.1A that a type cannot state. `Err` names the broken constraint.
    ///
    /// Clause: Codec A.5, Core IN-9.
    pub fn check(&self) -> Result<(), &'static str> {
        fn unique(mods: &[Modifier]) -> bool {
            mods.iter().enumerate().all(|(i, m)| !mods[..i].contains(m))
        }
        fn one_scalar(key: &Key) -> bool {
            match key {
                Key::Char(c) => c.chars().count() == 1,
                Key::Named(_) => true,
            }
        }
        match self {
            Input::Key {
                key,
                mods,
                shifted_key,
                base_layout_key,
                ..
            } => {
                if !one_scalar(key) {
                    return Err("char must be one Unicode scalar");
                }
                if !unique(mods) {
                    return Err("mods is a set");
                }
                for alt in [shifted_key, base_layout_key].into_iter().flatten() {
                    if !matches!(alt, Key::Char(_)) || !one_scalar(alt) {
                        return Err("alternate keys are one char");
                    }
                }
                if shifted_key.is_some() && !mods.contains(&Modifier::Shift) {
                    return Err("shifted_key needs shift in mods");
                }
                Ok(())
            }
            Input::Mouse {
                action,
                button,
                mods,
                notches,
                ..
            } => {
                let wheel_button = matches!(
                    button,
                    MouseButton::WheelUp
                        | MouseButton::WheelDown
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                );
                if wheel_button != (*action == MouseAction::Wheel) {
                    return Err("action is wheel exactly when the button is a wheel button");
                }
                if mods
                    .iter()
                    .any(|m| !matches!(m, Modifier::Shift | Modifier::Alt | Modifier::Ctrl))
                    || !unique(mods)
                {
                    return Err("mouse mods are a subset of shift, alt, ctrl");
                }
                if notches.is_some_and(|n| n == 0 || *action != MouseAction::Wheel) {
                    return Err("notches is at least 1 and only with wheel");
                }
                Ok(())
            }
            Input::Resize { rows, cols, .. } if *rows == 0 || *cols == 0 => {
                Err("a size is positive")
            }
            Input::ReplyTerminalQuery {
                reply: QueryReply::Position { x, y },
                ..
            } if *x > 65535 || *y > 65535 => Err("position is u16"),
            _ => Ok(()),
        }
    }
}

const KINDS: [&str; 11] = [
    "key",
    "mouse",
    "focus",
    "resize",
    "reply_terminal_query",
    "paste_begin",
    "paste_commit",
    "paste_abort",
    "file_begin",
    "file_commit",
    "file_abort",
];

/// The result of reading a `0x81 input` body.
///
/// Clause: Codec A.1 (`invalid_input`), A.4 (extension rules), A.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedInput {
    Input(Input),
    /// A parseable input with an unknown kind or enum value: the worker refuses it with `unsupported { what }`, and the route stays open.
    Unsupported {
        op: Option<Op>,
        what: String,
    },
    /// A parseable input that breaks a field constraint of Core IN-9: the worker refuses it, and the route stays open.
    Invalid {
        op: Option<Op>,
        what: &'static str,
    },
}

/// Reads the JSON body of a `0x81 input` frame.
///
/// A body that is not valid JSON, not an object, or has no valid shape is `malformed_frame`.
/// A paste or file shape with a missing `op`, or with `op` 0, is `malformed_frame` (TS-5d, TS-5f).
///
/// Clause: Codec A.1, A.5.
pub fn parse_input(body: &[u8]) -> Result<ParsedInput, DecodeError> {
    let value = crate::strict::parse(body)?;
    let Some(object) = value.as_object() else {
        return Err(DecodeDetail::InputNotObject.into());
    };
    let Some(kind) = object.get("kind").and_then(Value::as_str) else {
        return Err(DecodeDetail::InputNoKind.into());
    };
    let op = object.get("op").and_then(Value::as_str).and_then(parse_op);
    let op = op.map(Op);
    if !KINDS.contains(&kind) {
        return Ok(ParsedInput::Unsupported {
            op,
            what: "kind".to_string(),
        });
    }
    match serde_json::from_value::<Input>(value.clone()) {
        Ok(input) => {
            if matches!(
                input,
                Input::PasteBegin { op: Op(0), .. }
                    | Input::PasteCommit { op: Op(0), .. }
                    | Input::PasteAbort { op: Op(0) }
                    | Input::FileBegin { op: Op(0), .. }
                    | Input::FileCommit { op: Op(0) }
                    | Input::FileAbort { op: Op(0) }
            ) {
                return Err(DecodeDetail::InputNeedsOp.into());
            }
            if let Input::Key {
                key,
                shifted_key,
                base_layout_key,
                ..
            } = &input
            {
                let unknown = [Some(key), shifted_key.as_ref(), base_layout_key.as_ref()]
                    .into_iter()
                    .flatten()
                    .any(|k| matches!(k, Key::Named(n) if !n.is_known()));
                if unknown {
                    return Ok(ParsedInput::Unsupported {
                        op,
                        what: "key".to_string(),
                    });
                }
            }
            if let Err(what) = input.check() {
                return Ok(ParsedInput::Invalid { op, what });
            }
            Ok(ParsedInput::Input(input))
        }
        Err(error) => match unknown_value(object) {
            Some(what) => Ok(ParsedInput::Unsupported {
                op,
                what: what.to_string(),
            }),
            None => Err(DecodeDetail::InputShape(error.to_string()).into()),
        },
    }
}

/// Finds the first field whose value is an unknown enum value, so that the input is refused instead of ending the route.
fn unknown_value(object: &serde_json::Map<String, Value>) -> Option<&'static str> {
    fn not_in(value: Option<&Value>, known: &[&str]) -> bool {
        matches!(value, Some(Value::String(s)) if !known.contains(&s.as_str()))
    }
    if not_in(object.get("event"), &["press", "repeat", "release"]) {
        return Some("event");
    }
    if not_in(object.get("action"), &["press", "release", "move", "wheel"]) {
        return Some("action");
    }
    if not_in(
        object.get("button"),
        &[
            "left",
            "middle",
            "right",
            "back",
            "forward",
            "none",
            "wheel_up",
            "wheel_down",
            "wheel_left",
            "wheel_right",
        ],
    ) {
        return Some("button");
    }
    if let Some(Value::Array(mods)) = object.get("mods") {
        let known = [
            "shift",
            "alt",
            "ctrl",
            "meta",
            "super",
            "hyper",
            "caps_lock",
            "num_lock",
        ];
        if mods.iter().any(|m| not_in(Some(m), &known)) {
            return Some("mods");
        }
    }
    if let Some(reply) = object.get("reply") {
        let known = [
            "clipboard",
            "pixels",
            "chars",
            "window_state",
            "position",
            "text",
            "color_scheme",
            "encoded",
            "decline",
        ];
        match reply {
            Value::String(s) if !known.contains(&s.as_str()) => return Some("reply"),
            Value::Object(map) if map.len() == 1 => {
                let (name, data) = map.iter().next().expect("one entry");
                if !known.contains(&name.as_str()) {
                    return Some("reply");
                }
                if name == "color_scheme" && not_in(Some(data), &["dark", "light"]) {
                    return Some("color_scheme");
                }
            }
            _ => {}
        }
    }
    None
}
