# Hub subject controls (feature `hc-runner`)

The HC runner drives a `HubSubject` (`crates/botster-hub-conformance/src/hc/subject.rs`). Stage 2 implements it for the real Hub, in-process, over the Hub's in-memory transport (HC A5-1). This file is the list of what the harness owes the suite, as `docs/core-testkit-controls.md` is for Core.

The rules (design 6.3, BUILD.md "Testing" rule 8):
- The harness builds the Hub from injected edges: the clock, storage, the Core handle (Core's testkit), the program backend, the plugin loader, the transport, and the seeded scheduler. It never adds a test branch to production code, and it never reads an internal field of the Hub to answer a control.
- A control either **causes** an event through an edge, or **observes** at an edge. No control answers from a stand-in. A control that the harness cannot build gives `unsupported_control`, never a pass.
- A control is never a contract obligation for production.
- The in-memory transport is test plumbing. It is never a binding: it never appears in `welcome.features` as `binding:*`, and a run over it never satisfies a binding clause (HC A5-1, TB-0). The binding runs of TB-0 are named slow-tier runs of the real Hub.

## Harness obligations that are not controls

| Obligation | Clauses | What the harness does |
|---|---|---|
| Source rows | R-11, DS-5 | The rows of `rows` (and of the fixtures) are the state that the harness's own path produces, never defaults that the Hub fills in. The harness builds each session through the Core testkit with a probe program (`s1` is `running`), ends one (`s2`, `ended`, `exit {code: 0}`) and loses one (`s3`, `lost`), and each row that the Hub then serves must equal the fixture row. `tests/hc_structural.rs` validates every fixture and setup row against the generated kind schemas, so a row is complete and typed (the `session` row of R-11 included). |
| `configure(setup.hub)` | BD-1, BD-3, DS-5, HS-0, HS-2, HS-8, UI-CF-1, UI-EM-7, UI-RC-4 | Builds the Hub from injected parts. The members of `setup.hub` are: `rows` and the named fixture (`conformance/hc/fixtures`, `setup.fixture`): the contents of the Hub's sources; `plugins`: the fixture plugins that the plugin-loader edge offers; `packages`: installed packages with their views, `ui_actions` and `assets` (UI-CF-1, UI-EM-7, UI-RC-4); `assets_origin`: the origin of the asset capability URLs (UI-EM-7); `limits`: the Hub's configuration, which `welcome.limits` reports (BD-3); `version`: the Hub's current protocol version, `hub_current` (HS-2: a constructor argument, since the contract leaves the number to the build); `max_connections`: the connection limit of HS-8; `denied`: the permissions that the caller lacks (DS-5); `socket_path`: the listener path (HS-0); `adoption_held`: the Core testkit's process edge holds adoption until `adoption_done` (HS-0); `hub_id`: the Hub's id (the fixtures use `hub-1`); `worker_terminal_formats`: the route formats that the session workers behind the Hub offer, which the harness sets at the Core testkit's worker edge, or reports `unsupported_control` when it cannot. Every limit that a transcript sets is at or above the contract's floor (BD-1: `max_watches` of at least 32; `bd_1_watch_cap` sets 32 and reads it back from `welcome`). |
| Offered formats and features | HS-3, TS-1 | The transcripts that attach or select a format name it in `setup.hub.worker_terminal_formats` and in `hello.terminal_formats` (`ghostsnp/2+raw`, the example that the HC layering note on the route gives). HC does not require that name: it is the value that these transcripts configure, and a harness whose workers offer another format configures the same member and changes the name in one place. The features of `welcome` are the Hub's own: `hs_3_features_intersected` relies on the contract-named `ui_embed`. |
| `start()` | HS-0 | Starts the Hub. `setup.start: false` leaves it down (the HS-0 transcripts start it with `hub_up`). |
| `connect(binding?, uid?, credential?)` | TB-0, TB-L3, HS-7 | Opens a connection on the in-memory transport, or on the named binding. `Ok(None)` means that nothing listens (HS-0). `uid` and `credential` are what the binding authenticates. |
| `write`, `read`, `close` | TR-1, EN-5 | Move bytes, one write per call. `read` never waits for the Hub. |
| `advance_clock(ms)` | HS-8, EN-8 | Moves the injected clock. A Hub that cannot inject its clock gives `Err`; the step is `unsupported_control`. |
| `pump()` | A5-4 | Runs the Hub's injected scheduler one step. The seed of the run picks the producer variations inside the Hub (response order, resync cuts, the moment of `hub_shutdown`). The driver pumps only when a read is empty, so a transcript never sleeps. |
| A permute-only delivery decorator | A5-4 | Optional on the transport edge: it changes batching, chunking and the interleaving of independent streams, and keeps every stream's order, baselines and frame bounds. It never invents, alters or drops a frame. A drop is a scripted fault (`drop_response`). |

## Controls

| Control | Clauses | Arguments and result | Harness obligation (the edge) |
|---|---|---|---|
| `hub_up` | HS-0 | `{}` gives `{state, code?, closed, exit_nonzero?}`. | Starts the Hub that `start: false` left down, with a sink as the ready descriptor (`botster-hub up --ready-fd`), and reports what the Hub wrote to it (HS-0). The sink is the injected edge. |
| `ready_line` | HS-0 | `{}` gives `{state: none or ready or failed, path?, code?, closed, exit_nonzero?}`. | Reads the ready sink without waiting. `none` means that nothing has been written (the Hub is not yet listening and adopted). |
| `adoption_done` | HS-0 | `{}`. | Lets the injected Core testkit finish adoption (its process edge). HS-0 writes the ready line only after listen and adoption, so the harness holds adoption until this control. |
| `crash_before_ready` | HS-0 | `{}`. | The process edge ends the Hub before it writes a ready line, as a kill would, and closes the sink. A client then sees end-of-file without a line (HS-0: a failure). |
| `crash` | EN-7, HS-5, HS-9 | `{}`. | Drops the Hub value with no shutdown path (as `kill -9`; the named slow-tier run of `en_7_committed_survives_crash` and `en_7b_caller_operation_id_resolves_after_kill9` sends a real SIGKILL to a real Hub process instead), keeps the storage and Core edges and the surviving workers, and builds a new Hub over them. Every connection of the old Hub is closed. Only committed state survives. |
| `hold_recovery` | EN-7 | `{}`. | The storage edge holds the replay of durable-operation records after the next restart, until `recovery_done`. While it is held, `get(operation)` is `busy` (EN-7b). |
| `recovery_done` | EN-7 | `{}`. | Releases the replay at the storage edge. Durable-operation recovery then completes (EN-7b). |
| `shutdown_now` | EN-2, EN-3, HS-9 | `{}`. | Delivers the host's shutdown request to the Hub through its signal edge. The Hub answers each outstanding request (EN-3) and sends `close { hub_shutdown }` (HS-9). |
| `hold_completion` | EN-1, EN-3, EN-3b, EN-4, EN-8 | `{count}`. | The Hub's injected scheduler withholds the next `count` admitted requests: they stay admitted and outstanding, and nothing is dispatched for them until `release_completion`. The control forces no production behavior (EN-3b leaves `cancel` best effort); it fixes only where the request waits, so a transcript sees a request that is still outstanding (EN-1, EN-4, EN-8). |
| `release_completion` | EN-3b, EN-4, EN-8 | `{}`. | Releases every held completion at the edge, in the order the Hub's scheduler decides (A5-4). |
| `drop_response` | EN-8 | `{count}`. | The transport edge drops the next `count` response frames after the Hub committed the effect, and cuts that connection (a lost response). It is a scripted fault (R-1.4, A5-2), the only kind of drop. The transcript does not assert the cut: the client simply retries on a new connection (EN-8). |
| `worker` | HS-5, HS-9 | `{session, route_id, inner, args}`. | A control of the route endpoint behind the Hub, over the Core testkit's program backend: `inner: pty_output` makes the scripted program print `args.bytes_hex`. The bytes are scripted. Nothing emulates a terminal (R-5, R-7). |
| `caller_of` | HS-7 | `{conn}` gives `{kind, credential_id?}`. | Reads, at the plugin-loader edge, the caller that the Hub passed with a dispatch for a request on that connection (A.3). The harness sends one probe request that a fixture plugin records. It does not read a Hub field. |
| `watch_count` | HS-9 | `{conn}` gives `{count}`. | Counts, at the recording source edge, the subscriptions that the Hub holds for that connection's watches. The edge records each subscribe and each release. It does not read a Hub field. |
| `reload_plugin` | DS-6 | `{key}`. | The plugin-loader edge reloads plugin `key` (R-1.4: a reload between `describe` and dispatch). |
| `dispatch_log` | DS-6, UI-RC-4 | `{}` gives `{dispatched: [{kind, action, ...}]}`. | The plugin-loader edge's own record of the dispatches that reached it, in order. A negative (`dispatched: []`) is meaningful because the same edge records the dispatch of the positive case in `ds_6_stale_descriptor_refused_not_dispatched`. |

## Client-subject player (HC A5-2)

The runner plays the Hub for `subject: client` transcripts (`player.rs`). It holds no Hub logic. The client sits behind `ClientSubject`, with its own transport adapter. A client harness owes: `deliver`, `close`, `act` (`client_act`: `connect` (the client starts and says hello), `reconnect` (a new connection with its limits forgotten), `describe` (the client sends `describe`, DS-1: the obligation is to make it ask, whatever its internal order), `demand {requests, watches, payload_bytes}` (work above any bound: the client sends, queues or refuses it), `request {verb, kind, target}`), `pump`, `take_written`, `observe`, and `configure(setup.client)`. `setup.client.hand_built` names the kinds that it has hand-built screens for (DS-4).

| Observation | Clauses | Holds when the client answers |
|---|---|---|
| `generic_screen {kind}` | DS-4 | `true` |
| `notice_logged {kind}` | DS-4 | `true` |
| `response_delivered {id}`, `welcome_accepted {}` | EN-6 | `true` (an unknown optional member is ignored) |

The step `client_requests {at_most, verb?, max_frame_bytes?}` (BD-3) runs the client to quiet and checks that everything it wrote is a request (of that verb), at most `at_most`, none over the byte bound. BD-3 fixes maxima, so the step fixes no minimum: a client may serialize, queue or refuse work. The client's frames before the step must all be matched by the script.
| 
A script ends with `client_idle`: the client has run to quiet and wrote no frame that the script did not match (HC A5-2). Each wait is bounded: a client that never comes to quiet, or a deadline that has passed, ends the step as `inconclusive`. The R-1.4 faults that the player executes (the rest move to `ts-player`, see `docs/hc-runner-notes.md`): `disconnect` (drop the transport, also mid-route and mid-frame after a `send_wire` of half a frame), `send_raw` (a malformed or cut frame), and `send_wire` (a chunk boundary at any byte).
