# ReadFacts (ST-7; P5; steward rulings R-101, R-102, R-103, R-105)

- **The log** (`worker/facts.rs`, one per instance, made at the launch): the input records of client and host input in
  one order, the client tail, and the time of the last client input. Only the worker has them at one point of its
  ordering (AM-2), so `ReadFacts` is answered by the worker at once (`model.rs` `read_facts`).
- **A record at receipt.** A host write's record is made when its `WriteInput` comes (`input.rs` `on_write_input`). A
  client input's record is made in the same call that advances `input_rev{client}` (`route.rs` `client_input`). Its
  state is `pending`, and its `pty_bytes` is null.
- **What a client frame is** (`receipt.rs`, one function for each mapping, no catch-all): - one client input: `bytes`,
  `text`, a key, the mouse, focus, a query reply, and a paste's or a file's commit; - not input (no `rev` step, no
  `Activity`, no record, no `age_ms` reset): a paste's or a file's begin, chunks and abort (R-102), a resize (R-105 Q4;
  it is still route activity for DP-6 `Latest`), and a frame that does not parse, which closes the route `BadFrame`
  (R-103 (1)); - the record keeps what was sent (R-103 (1)): a key's, the mouse's or focus's 5.1A fields, read from the
  frame's body with the contract's types; a query reply as `kind: bytes` with the decoded bytes of an `encoded` reply,
  else the JSON of its `reply` as received (R-105 Q2); a frame of an unknown kind as `kind: bytes` with its body as
  received. Text that is not UTF-8 is `kind: text` with no payload; - a refused input's record ends at once with its
  IN-2 reason, not the wire reason (R-101 Q2): `not_writable`, `not_ready` and `refused` are `Refused`; `too_large` is
  `PayloadTooLarge`; `unsupported{what}` is `Unsupported` with the variant of its cause when one exists (the codec's
  `key`, an unknown named key, is `named_key`), else `Other` (R-103 (2), R-105 Q3); a refusal at the admission point
  keeps `SessionEnded` or `Stopping` apart.
- **Completion is an appended status update** with a new cursor (`input.rs` `end_record`); the record is never edited.
  The state comes from the IN-2 outcome (`record_end`): `Partial` and `Failed` are `partial` with the exact PTY bytes, 0
  included (R-101 Q1); `Unknown` is `unknown` with null PTY bytes. A kind that the worker cannot write is
  `NotWritten(Unsupported{what})` in the result and the record, never `Internal` (R-101 Q3). The end of a client input
  reports `Observation::ClientInputEnded{route}`, and the host raises the keyed `Activity{Client(route)}` edge again
  (EV-6).
- **Inputs that never start:** a host write that the adoption fence drops ends `not_written{refused}` with 0 PTY bytes:
  the fence refuses the old epoch's commands (DP-8), and no cancel was issued (R-103 (3)). A client input that waits in
  a closing route's `received` queue is discarded and ends `not_written` with 0 PTY bytes, never `cancelled` (R-105 Q1):
  its IN-2 reason (`SessionEnded` or `Stopping`) when the session's end closed the route, else `refused` (`receipt.rs`
  `discarded`). The session's end is `SessionEnded`, `SessionRemoved` and `SessionLost`: the worker never closes a route
  `SessionLost`, but a lost session is an ended session for IN-2 (lead, 2026-10-12). An input at the admission point
  ends at its turn, as before.
- **The cursor** is `ordinal << 29 | seq` (R-101 follow-up): the 24-bit ordinal of `LaunchSpec.cursor_ordinal` (the
  host's durable counter of the data directory) and the sequence number of the entry. An adoption keeps both, so a
  cursor read before it reads incrementally after it. A cursor of another ordinal is a `gap`. Each record that has not
  ended keeps one sequence number for its status update, so an end always has one. When no record can be made, a host
  write is `NotWritten(Refused)` with exact zero, and a client frame is refused `refused` with no `rev` step;
  `ReadFacts` still answers.
- **Retention:** at most `input_events_kept` entries, and at most `input_events_bytes` bytes of the list as JSON, the
  form in which the link carries it. The oldest entry goes first. The newest record and its status update are never
  dropped: a record that would not leave room for its status update is cut to the longest payload that does, then to its
  minimal form, with `payload_truncated`. `payload_truncated` means only that something was cut: a record with no
  truthful payload has none and is not `payload_truncated` (R-103). The minimal form always keeps the cursor, the full
  source tags, `at`, kind and state. If the protected pair in its minimal form alone exceeds `input_events_bytes` (route
  tags at `max_route_tag_bytes` that JSON escapes), the list keeps the pair and drops every older record, and every read
  reports `gap` while it is over the bound: this is the only case where the list, and `input_records_cost`, exceed the
  bound (steward ruling R-116; the 9B minimum stays).
- **Time:** `at` and `age_ms` are milliseconds on the worker's own clock, from the worker's start (the first `now` that
  it is given). Before any client input, `age_ms` counts from that start (R-101 Q4, Q5). They are not comparable with
  the unix `at` of events (TM-1).
- **The client tail** keeps the last `input_tail_bytes` PTY bytes that client input wrote, exactly; `tail_complete`
  turns false at the first byte that it drops.
- **Prior art:** old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no `ReadFacts`, no input records and no client
  tail (`git grep` for `input_records`, `InputRecord`, `ReadFacts` and `client_tail` finds nothing). No prior art
  applies.
