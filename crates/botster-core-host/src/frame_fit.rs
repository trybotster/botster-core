//! Steward ruling R-100: a call whose message to the worker can never fit the link frame is refused at the call, with
//! `InvalidInput{field}` (no `OpId`, no route, no event: AM-4). Core never sends a frame that the worker's decoder refuses, and
//! never drops a link for one.
//!
//! The check measures the message that the host sends later, with the worst case of each value that is assigned after the
//! call: the request number `u64::MAX`, the route id `u64::MAX`, and the largest `Size` (DESIGN.md, "R-100"). A `Launch` also
//! has the largest cursor ordinal (`Start` reserves it after the call; ST-7), and the largest color profile and size policy,
//! because a setter on a `Created` session can change them after the check
//! of `Start` (RC-R100-3). So a call that passes the check never fails at the send. The cost is a refusal under the bound:
//! up to about 40 bytes for an `Op` or an `AttachRoute`, and for a `Launch` up to several kilobytes (a full palette is about
//! 6.7 KB of JSON) (RC-R100-5).
//!
//! The bound of a `Launch` is the worker's bound before its launch, [`DEFAULT_MAX_PAYLOAD`]: the worker learns the link
//! frame bound from the `Launch` itself (RC-R100-I1). The bound of each other message is the link frame bound.
//!
//! The fields are the host-supplied values that no `CoreLimits` value bounds. Each other part of a message is a number, a
//! flag, a bounded enum or a bounded list (a palette has 256 entries), and all of them together encode in less than
//! [`FIXED_PART`]. When the fields' upper bounds and [`FIXED_PART`] fit, the message fits and nothing is encoded; else the
//! message is encoded into a counter.

use crate::engine::HostEngine;
use botster_core_contract::prelude::*;
use botster_core_link::frame::DEFAULT_MAX_PAYLOAD;
use botster_core_link::msg::{json_len, HostMsg, MAX_CURSOR_ORDINAL};
use botster_route_codec::prelude::{HexBytes, Key, Modifier};

/// An upper bound of a message's encoded length without its fields (see the module note; the tests measure it).
pub(crate) const FIXED_PART: u64 = 64 * 1024;

/// The largest `Size` that `begin` admits (A2-1): its JSON is the longest.
pub(crate) const WORST_SIZE: Size = Size {
    rows: 65_535,
    cols: 65_535,
    cell_px: Some(CellPx {
        width: u32::MAX,
        height: u32::MAX,
    }),
};

/// An upper bound of one modifier's JSON length with its separator: the longest name, `"caps_lock"`, is 11 bytes.
pub(crate) const MODIFIER_UPPER: u64 = 12;

/// The largest color profile that `begin` admits: a full palette, and each color at its longest JSON.
fn worst_color_profile() -> ColorProfile {
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    ColorProfile {
        palette: Some(vec![white; 256]),
        foreground: white,
        background: white,
        cursor: Some(white),
    }
}

/// `request` with the largest color profile and size policy, which a setter on a `Created` session can still store
/// before the `Launch` (RC-R100-3).
pub(crate) fn worst_launch_request(request: &SpawnRequest) -> SpawnRequest {
    let mut worst = request.clone();
    worst.color_profile = Some(worst_color_profile());
    worst.size_policy = [
        SizePolicy::Latest,
        SizePolicy::Smallest,
        SizePolicy::Largest,
    ]
    .into_iter()
    .max_by_key(json_len);
    worst
}

/// A host-supplied field of a worker message, by its name in the call.
#[derive(Clone, Copy)]
pub(crate) enum Field<'a> {
    Str(&'static str, &'a str),
    Strs(&'static str, &'a [String]),
    Map(&'static str, &'a std::collections::BTreeMap<String, String>),
    Hex(&'static str, &'a HexBytes),
    Mods(&'static str, &'a [Modifier]),
}

impl Field<'_> {
    fn name(&self) -> &'static str {
        match *self {
            Field::Str(n, _)
            | Field::Strs(n, _)
            | Field::Map(n, _)
            | Field::Hex(n, _)
            | Field::Mods(n, _) => n,
        }
    }

    /// An upper bound of the field's JSON length, without encoding it: a string byte takes at most six bytes (`\u00XX`), a
    /// hex byte two (in its `$bytes_hex` object), and each string its two quotes and a separator.
    pub(crate) fn upper(&self) -> u64 {
        let text = |s: &str| 6 * s.len() as u64 + 3;
        match *self {
            Field::Str(_, s) => text(s),
            Field::Strs(_, list) => 2 + list.iter().map(|s| text(s)).sum::<u64>(),
            Field::Map(_, map) => 2 + map.iter().map(|(k, v)| text(k) + text(v)).sum::<u64>(),
            // `{"$bytes_hex":"<hex>"}` (Codec A.4): two bytes a byte and 17 more.
            Field::Hex(_, bytes) => 2 * bytes.0.len() as u64 + 20,
            Field::Mods(_, mods) => 2 + MODIFIER_UPPER * mods.len() as u64,
        }
    }

    /// The field's JSON length.
    pub(crate) fn exact(&self) -> u64 {
        match *self {
            Field::Str(_, s) => json_len(s),
            Field::Strs(_, list) => json_len(list),
            Field::Map(_, map) => json_len(map),
            Field::Hex(_, bytes) => json_len(bytes),
            Field::Mods(_, mods) => json_len(mods),
        }
    }
}

/// The fields of an op that travels to the worker as `HostMsg::Op` (`Next::Forward` in `commit`); `None` for an op that
/// does not travel so (the host's own ops, and those with a message of their own: `Stop`, `Remove`, `Detach`, `Launch`).
pub(crate) fn op_fields(op: &Op) -> Option<Vec<Field<'_>>> {
    let mut fields = Vec::new();
    match op {
        Op::WriteInput {
            session, payload, ..
        } => {
            fields.push(Field::Str("session", &session.0));
            match payload {
                InputPayload::Bytes { bytes } => fields.push(Field::Hex("payload.bytes", bytes)),
                InputPayload::Paste { bytes, .. } => {
                    fields.push(Field::Hex("payload.bytes", bytes))
                }
                InputPayload::Text { text } => fields.push(Field::Str("payload.text", text)),
                InputPayload::Key(key) => {
                    fields.push(Field::Mods("payload.mods", &key.mods));
                    for (name, k) in [
                        ("payload.key", Some(&key.key)),
                        ("payload.shifted_key", key.shifted_key.as_ref()),
                        ("payload.base_layout_key", key.base_layout_key.as_ref()),
                    ] {
                        match k {
                            Some(Key::Named(n)) => fields.push(Field::Str(name, &n.0)),
                            Some(Key::Char(c)) => fields.push(Field::Str(name, c)),
                            _ => {}
                        }
                    }
                    if let Some(text) = &key.text {
                        fields.push(Field::Str("payload.text", text));
                    }
                }
                InputPayload::Mouse(mouse) => fields.push(Field::Mods("payload.mods", &mouse.mods)),
                _ => {}
            }
        }
        Op::CaptureSnapshot { session, owner } => {
            fields.push(Field::Str("session", &session.0));
            fields.push(Field::Str("owner", &owner.0));
        }
        Op::SetNotificationPolicy { session, policy } => {
            fields.push(Field::Str("session", &session.0));
            if let NotificationPolicy::Matching { patterns } = policy {
                fields.push(Field::Strs("policy.patterns", patterns));
            }
        }
        Op::SetSizePolicy { session, .. }
        | Op::SetColorProfile { session, .. }
        | Op::SetOutputTap { session, .. }
        | Op::ReadScreen { session, .. }
        | Op::ReadCursor { session }
        | Op::ReadModeFlags { session }
        | Op::ReadFacts { session, .. }
        | Op::Resize { session, .. } => fields.push(Field::Str("session", &session.0)),
        Op::Signal { id, .. } => fields.push(Field::Str("id", &id.0)),
        _ => return None,
    }
    Some(fields)
}

/// The fields of a launch request; `policy` is the name of the notification policy's patterns in the call.
pub(crate) fn launch_fields<'a>(request: &'a SpawnRequest, policy: &'static str) -> Vec<Field<'a>> {
    let mut fields = vec![
        Field::Strs("request.argv", &request.argv),
        Field::Map("request.env", &request.env),
        Field::Str("request.cwd", &request.cwd),
    ];
    if let Some(NotificationPolicy::Matching { patterns }) = &request.notification_policy {
        fields.push(Field::Strs(policy, patterns));
    }
    fields
}

/// Refuses `InvalidInput{field}` when `msg` cannot fit `bound` (R-100); `field` is the largest field by its JSON length.
/// `msg` is built only when the fields' upper bounds and [`FIXED_PART`] do not fit.
pub(crate) fn fit_within(
    bound: u64,
    fields: &[Field<'_>],
    msg: impl FnOnce() -> HostMsg,
) -> Result<(), CoreError> {
    let upper: u64 = fields.iter().map(Field::upper).sum();
    if upper.saturating_add(FIXED_PART) <= bound {
        return Ok(());
    }
    let len = msg().encoded_len();
    if len <= bound {
        return Ok(());
    }
    let field = fields
        .iter()
        .max_by_key(|f| f.exact())
        .map_or("", Field::name);
    Err(CoreError::new(
        ErrorCode::InvalidInput {
            field: Some(field.into()),
        },
        format!("the message to the worker is {len} bytes, over its frame bound {bound}; the largest field is {field}"),
    ))
}

impl HostEngine {
    /// [`fit_within`] the link frame bound of this handle.
    fn fit(&self, fields: &[Field<'_>], msg: impl FnOnce() -> HostMsg) -> Result<(), CoreError> {
        fit_within(u64::from(self.link_frame_bound()), fields, msg)
    }

    /// The `Launch` that `request` gives, at the worst size, color profile and size policy, within the worker's bound
    /// before its launch (R-100; RC-R100-3, RC-R100-I1).
    fn fit_launch(&self, request: &SpawnRequest, policy: &'static str) -> Result<(), CoreError> {
        fit_within(
            u64::from(DEFAULT_MAX_PAYLOAD),
            &launch_fields(request, policy),
            || {
                let worst = worst_launch_request(request);
                HostMsg::Launch(Box::new(self.launch_spec_for(
                    &worst,
                    WORST_SIZE,
                    MAX_CURSOR_ORDINAL,
                )))
            },
        )
    }

    /// The frame check of `begin` (R-100), after the arguments and the payload size: the op's own `HostMsg::Op`, and the
    /// future `Launch` of a call that sets what it carries.
    pub(crate) fn check_frame_fit(&self, op: &Op) -> Result<(), CoreError> {
        if let Some(fields) = op_fields(op) {
            self.fit(&fields, || HostMsg::Op {
                req: u64::MAX,
                op: op.clone(),
            })?;
        }
        match op {
            Op::Create { request, .. } => {
                self.fit_launch(request, "request.notification_policy.patterns")
            }
            Op::Start { id } => match self.sessions.get(id) {
                Some(s) => self.fit_launch(&s.request, "request.notification_policy.patterns"),
                None => Ok(()),
            },
            // Before the start, the policy goes into the row and travels in the `Launch` (DP-6, A3-1).
            Op::SetNotificationPolicy { session, policy } => match self.sessions.get(session) {
                Some(s) if s.admit == crate::session::Admit::Created => {
                    let mut request = s.request.clone();
                    request.notification_policy = Some(policy.clone());
                    self.fit_launch(&request, "policy.patterns")
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// The frame check of `attach` (R-100): the route's `AttachRoute`, with the caller's `client` (DP-8).
    pub(crate) fn check_attach_fit(
        &self,
        options: &AttachOptions,
        limits: &AppliedRouteLimits,
        client: &ClientId,
    ) -> Result<(), CoreError> {
        let mut fields = vec![
            Field::Str("client", &client.0),
            Field::Str("file_directory", &options.file_directory),
            Field::Strs("route_features", &options.route_features),
            Field::Strs("terminal_formats", &options.terminal_formats),
        ];
        for (name, tag) in [("route_tag", &options.route_tag), ("owner", &options.owner)] {
            if let Some(tag) = tag {
                fields.push(Field::Str(name, tag));
            }
        }
        self.fit(&fields, || HostMsg::AttachRoute {
            route: RouteId(u64::MAX),
            options: options.clone(),
            limits: *limits,
            client: client.clone(),
        })
    }
}
