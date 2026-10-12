//! The route step driver (design 5.5, codec suite). It drives a route endpoint through [`RouteSubject`].
//!
//! Steps (each takes `"route": "<name>"`, default `r1`):
//!
//! | Step | Form |
//! |---|---|
//! | `route_attach` | `{"route_attach": {history?, route_features?, writable?, answers_queries?}}`: the runner plays the Hub and hands the route to the worker (Codec TS-1) |
//! | `route_send` | `{"route_send": <decoded frame>, "bind"?: "$size"}`: a valid frame, checked against the schema before it is sent; `bind` binds its size: the type byte plus the body (Codec TS-3), the delimiting of the binding not counted |
//! | `route_send_raw` | `{"route_send_raw": {"bytes_hex": ".."}}`: TS-3 frame bytes (`[type][body]`) with a correct delimiter added |
//! | `route_send_wire` | `{"route_send_wire": {"bytes_hex": ".."}}`: complete wire bytes in one write, unchanged |
//! | `route_read_bytes` | `{"route_read_bytes": {"bytes": n}}`: reads at most n wire bytes, so a frame can be started and left incomplete |
//! | `route_expect` | `{"route_expect": <matcher>}`: the next frame matches |
//! | `route_expect_output` | `{"route_expect_output": {"bytes_hex": "..", "ignore"?: [..], "kind"?: "output"\|"output_deflate"}}`, or `{"parts": [{"fill": "$f"}, {"bytes_hex": ".."}, ..]}` in place of `bytes_hex`: the `output` frames, inflated and joined, equal the bytes (with `parts`: the parts in order, a fill as its pattern, R-47 item 3; a mismatch reports its offset); with `kind`, every one of those frames is of that kind (Core A9-3: compression forced on gives `output_deflate` frames only) |
//! | `route_expect_model_equal` | the client model that loaded the baseline equals the worker's model (Codec TS-4) |
//! | `route_drain` | `{"route_drain": true}`: reads every frame that the subject has ready, within the execution limits |
//! | `route_expect_pty` | `{"route_expect_pty": [{"hex": ".."}, {"var": "$p"}, {"written_by_op": "4", "len_var"?: "$n", "contains"?: [..]}, {"written_without_op": true}]}`, or `{"one_of": [[..], ..]}`: the bytes that reached the PTY, as the control `pty_input` reports them (`bytes`; its `segments` are not read); the last two parts name a run of the log by relation to the transcript's own frames and its observed refusals (R-73 and its follow-up 2); a `written_without_op` run holds no unplaced `bytes`, `text` or paste body, but may cover key, mouse and focus bytes (the stated limit) |
//! | `send_invalid` | `{"send_invalid": <input frame>, "violates": "to_worker"}`: a typed negative input (design 5.4); it goes out as the JSON body of a `0x81` frame |
//! | `route_check_path` | `{"route_check_path": {"path": "$p", "under": "/dir", "max_component_bytes"?: n}}` |
//! | `control` | `{"control": {"op": "..", ..args}, "expect"?: <matcher>}`: a test control of the subject |
//!
//! An expectation carries `"because": "<contract> <clause id>"`, or a list of them.
//! The engine constructs (`expect`, `expect_set`, `expect_eventually`, `expect_quiet_until`, `expect_closed`) take `"on": "<route>"`.
//! A closed route gives `{"eof": true, "ended"?: "<reason>"}`. `ended` is the cause that the worker reports to the Hub for a failed route.

mod driver;
#[cfg(feature = "fake")]
pub mod fake;
mod subject;

pub use driver::{RouteDriver, RouteSchemas};
pub use subject::{ClientModel, Framing, RouteRead, RouteSubject};
