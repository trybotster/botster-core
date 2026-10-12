import json, base64, os
OUT = os.path.expanduser('~/botster-sessions/trybotster-botster-contracts-foundation-core-p4a-dp/conformance/core')
ST = b"\x1b\\"
BEL = b"\x07"
b64 = lambda b: base64.b64encode(b).decode()
hx = lambda b: b.hex()

def osc52(sel, data, term=BEL):
    return b"\x1b]52;" + sel + b";" + (b64(data).encode() if data is not None else b"") + term
def osc1337(data):
    return b"\x1b]1337;Copy=:" + b64(data).encode() + ST
def k_write(loc=None, id_=None):
    m = b"type=write"
    if loc: m += b":loc=" + loc
    if id_: m += b":id=" + id_
    return b"\x1b]5522;" + m + ST
def k_data(mime, data, id_=None):
    m = b"type=wdata:mime=" + b64(mime).encode()
    p = b";" + b64(data).encode() if data is not None else b""
    return b"\x1b]5522;" + m + p + ST
def k_commit():
    return b"\x1b]5522;type=wdata" + ST
def k_txn(reps, loc=None, id_=None):
    out = k_write(loc, id_)
    for mime, data in reps: out += k_data(mime, data)
    return out + k_commit()

def save(name, t):
    t = {"id": "conf::" + name, "contract": "core", **t}
    with open(f"{OUT}/conf::{name}.json", "w") as f:
        f.write(json.dumps(t, indent=2, ensure_ascii=False) + "\n")

def prelude(sess="s1"):
    return [
        {"begin": {"op": {"Create": {"session": sess, "request": {"program": [{"hold": {}}]}}}}, "bind": f"$c_{sess}"},
        {"pump_until": {"event": {"Completed": {"op": f"$c_{sess}", "result": {"ok": {"record": {"id": sess, "state": "Created"}}}}}}, "because": "Core LC-3"},
        {"begin": {"op": {"Start": {"id": sess}}}, "bind": f"$s_{sess}"},
        {"pump_until": {"event": {"Completed": {"op": f"$s_{sess}", "result": {"ok": {"record": {"id": sess, "state": "Running"}}}}}}, "because": "Core A2-1"},
    ]
def route_prelude(options=None):
    o = {"history": "none"}
    if options: o.update(options)
    return [
        {"attach_route": {"route": "r1", "session": "s1", "client": "c1", "options": o}, "because": "Core OU-1"},
        {"expect": {"frame": "attached", "terminal_format": {"$type": "string"}}, "on": "r1", "because": "Core OU-9"},
        {"expect": {"frame": "baseline_begin", "rows": {"$type": "integer"}, "cols": {"$type": "integer"}, "modes": {"$type": "object"}}, "on": "r1", "because": "Core OU-9"},
        {"expect": {"frame": "screen", "payload": {"$any": True}}, "on": "r1", "because": "Core OU-9"},
        {"expect_eventually": {"frame": "baseline_end", "history": "not_requested"}, "ignore": [{"frame": "history", "payload": {"$any": True}}], "on": "r1", "because": "Core OU-9"},
        {"expect": {"frame": "live"}, "on": "r1", "because": "Core OU-9"},
    ]
def out(b, sess="s1"):
    return {"control": {"op": "pty_output", "session": sess, "bytes_hex": hx(b)}}
def quiet(): return {"await_quiet": {}}
NONE = {"$one_of": [{"$absent": True}, None]}
def cw(sel, contents, total, because, reason=None, sess="s1"):
    ev = {"id": sess, "selection": sel, "contents": contents, "total_bytes": total}
    if reason: ev["reason"] = reason
    else: ev["reason"] = NONE
    return {"pump_until": {"event": {"ClipboardWrite": ev}}, "because": because}
def rep(mime, data):
    return {"mime": mime, "bytes": {"$bytes_hex": hx(data)}}
ANYMIME = {"$type": "string"}
def fence(label, sess="s1"):
    return {"expect_quiet_until": {"ClipboardWrite": {"id": sess}},
            "barrier": {"begin": {"op": {"UpdateMetadata": {"id": sess, "labels": {"fence": label}}}}, "bind": f"$bar_{label}"},
            "until": {"Completed": {"op": f"$bar_{label}", "result": {"ok": "unit"}}},
            "because": ["Core A13-1", "Core EV-3"]}
A = ["Core A13-1", "Core EV-3"]
HELLO = b"hello"

# --- A13-1 ---------------------------------------------------------------------------------------------
w52 = osc52(b"c", HELLO); w1337 = osc1337(HELLO); w5522 = k_txn([(b"text/plain", HELLO)])
save("a13_1_every_recognized_clipboard_write_is_surfaced", {
    "clause": "A13-1", "subject": "core",
    "requires": {"controls": ["pty_output"]},
    "steps": prelude() + route_prelude() + [
        out(w52), cw("c", [rep(ANYMIME, HELLO)], 5, A),
        out(w1337), cw("c", [rep(ANYMIME, HELLO)], 5, A),
        out(w5522), cw("c", [rep(b"text/plain".decode(), HELLO)], 5, A),
        quiet(),
        {"route_expect_output": {"bytes_hex": hx(w52 + w1337 + w5522)}, "route": "r1", "because": ["Core A13-1", "Core OU-12"]},
    ],
    "fake": "structural", "ruling": "R-12"})

sel_steps = []
for sel, expect in [(b"c", "c"), (b"p", "p"), (b"cp", "cp"), (b"s0", "s0"), (b"", "c")]:
    sel_steps += [out(osc52(sel, b"hi")), cw(expect, [rep(ANYMIME, b"hi")], 2, A)]
sel_steps += [out(k_txn([(b"text/plain", b"hi")], loc=b"primary")), cw("p", [rep("text/plain", b"hi")], 2, A),
              out(k_txn([(b"text/plain", b"hi")])), cw("c", [rep("text/plain", b"hi")], 2, A),
              out(osc1337(b"hi")), cw("c", [rep(ANYMIME, b"hi")], 2, A)]
save("a13_1_selection_from_osc_52_string_or_location", {
    "clause": "A13-1", "subject": "core", "requires": {"controls": ["pty_output"]},
    "steps": prelude() + sel_steps, "fake": "structural", "ruling": "R-12"})

HTML = b"<b>hello</b>"
head = k_write() + k_data(b"text/plain", HELLO) + k_data(b"text/html", HTML)
save("a13_1_all_representations_one_event_atomic", {
    "clause": "A13-1", "subject": "core", "requires": {"controls": ["pty_output"]},
    "steps": prelude() + [
        out(head), quiet(), fence("before_commit"),
        out(k_commit()),
        {"pump_until": {"event": {"ClipboardWrite": {"id": "s1", "selection": "c",
            "contents": {"$set": [rep("text/plain", HELLO), rep("text/html", HTML)]}, "total_bytes": len(HELLO) + len(HTML), "reason": NONE}}},
         "because": A + ["Core A13-1"]},
        quiet(), fence("after_commit"),
    ], "fake": "structural", "ruling": "R-12"})

save("a13_1_empty_contents_is_clear_distinct_from_empty_value", {
    "clause": "A13-1", "subject": "core", "requires": {"controls": ["pty_output"]},
    "steps": prelude() + [
        out(osc52(b"c", b"")), cw("c", [], 0, A),
        out(k_txn([(b"text/plain", None)])), cw("c", [rep("text/plain", b"")], 0, A),
        out(k_write() + k_commit()), cw("c", [], 0, A),
    ], "fake": "structural", "ruling": "R-12"})

save("a13_1_too_large_counts_all_representations", {
    "clause": "A13-1", "subject": "core", "requires": {"controls": ["pty_output"]},
    "setup": {"limits": {"clipboard_bytes": 10}},
    "steps": prelude() + [
        out(k_txn([(b"text/plain", b"hello"), (b"text/html", b"world")])),
        {"pump_until": {"event": {"ClipboardWrite": {"id": "s1", "selection": "c",
            "contents": {"$set": [rep("text/plain", b"hello"), rep("text/html", b"world")]}, "total_bytes": 10, "reason": NONE}}},
         "because": A},
        out(k_txn([(b"text/plain", b"abcdef"), (b"text/html", b"ghijkl")])),
        {"pump_until": {"event": {"ClipboardWrite": {"id": "s1", "selection": "c", "contents": NONE, "total_bytes": 12, "reason": "TooLarge"}}},
         "because": A},
    ], "fake": "structural", "ruling": "R-12"})

# --- A13-1b --------------------------------------------------------------------------------------------
B = ["Core A13-1b", "Core AM-2"]
def ack(status):
    return {"control": {"op": "oracle_clipboard_ack", "session": "s1", "status": status}, "bind_fields": {"bytes": "$ack"},
            "because": ["Core A13-1b", "Core R-7"]}
def log_is_ack(because=B):
    return {"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": "$ack"}, "because": because}
S = k_txn([(b"text/plain", HELLO)], id_=b"a1")
CTRL = ["oracle_clipboard_ack", "pty_input", "pty_output"]

save("a13_1b_one_acknowledgement_with_no_client", {
    "clause": "A13-1b", "subject": "core", "requires": {"controls": CTRL},
    "steps": prelude() + [out(S), quiet(), ack("success"), log_is_ack(),
        cw("c", [rep("text/plain", HELLO)], 5, A)],
    "fake": "structural", "ruling": "R-12"})

for name, ans in [("answering", True), ("refusing", False)]:
    save(f"a13_1b_one_acknowledgement_with_{'an' if ans else 'a'}_{name}_client", {
        "clause": "A13-1b", "subject": "core", "requires": {"controls": CTRL},
        "steps": prelude() + route_prelude({"answers_queries": ans}) + [
            out(S), quiet(), ack("success"), log_is_ack(),
            {"route_expect_output": {"bytes_hex": hx(S)}, "route": "r1", "because": ["Core A13-1", "Core OU-12"]},
            quiet(), log_is_ack(),
            cw("c", [rep("text/plain", HELLO)], 5, A)],
        "fake": "structural", "ruling": "R-12"})

PASTE = b"b" * 40
save("a13_1b_acknowledgement_is_contiguous_under_input_pressure", {
    "clause": "A13-1b", "subject": "core", "requires": {"controls": CTRL + ["edges_quiet", "pty_accept"]},
    "steps": prelude() + route_prelude() + [
        {"control": {"op": "pty_accept", "session": "s1", "bytes": 10}},
        {"route_send": {"frame": "input", "kind": "paste_begin", "op": "5"}},
        {"route_send": {"frame": "paste_chunk", "op": "5", "bytes": {"$bytes_hex": hx(PASTE)}}},
        {"route_send": {"frame": "input", "kind": "paste_commit", "op": "5"}},
        out(S), quiet(), ack("success"),
        {"control": {"op": "pty_accept", "session": "s1", "bytes": 1000}},
        {"let": {"$ackn": {"hex_len": ["$ack", 0]}}},
        {"let": {"$total": {"add": ["$ackn", len(PASTE)]}}},
        {"await_pty": {"session": "s1", "min_bytes": "$total"}},
        quiet(),
        {"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": "$log"}, "because": B},
        {"check": {"hex_len_eq": ["$log", "$total"]}, "because": B},
        {"check": {"hex_prefix": ["$log", hx(PASTE)]}, "because": B},
        {"check": {"hex_ends_with": ["$log", "$ack"]}, "because": B},
    ], "fake": "structural", "ruling": "R-12"})

BELLS = []
for _ in range(20): BELLS += [out(b"\x07"), quiet()]
save("a13_1b_success_with_no_host_and_a_full_event_queue", {
    "clause": "A13-1b", "subject": "core", "requires": {"controls": CTRL + ["edges_quiet"]},
    "setup": {"limits": {"event_queue": 16}},
    "steps": prelude() + [{"drain": {}}] + BELLS + [out(S), quiet(), ack("success"), log_is_ack(),
        {"expect_eventually": {"EventsLost": {"id": "s1"}}, "because": ["Core A13-1b", "Core EV-2", "Core EV-6"], "ignore": [{"Activity": {"id": "s1"}}]}],
    "fake": "structural", "ruling": "R-12"})

BIG = b"hello world"
save("a13_1b_over_bound_is_io_error", {
    "clause": "A13-1b", "subject": "core", "requires": {"controls": CTRL},
    "setup": {"limits": {"clipboard_bytes": 4}},
    "steps": prelude() + [out(k_txn([(b"text/plain", BIG)], id_=b"a1")), quiet(), ack("io_error"), log_is_ack(),
        {"pump_until": {"event": {"ClipboardWrite": {"id": "s1", "selection": "c", "contents": NONE, "total_bytes": len(BIG), "reason": "TooLarge"}}}, "because": A}],
    "fake": "structural", "ruling": "R-12"})
