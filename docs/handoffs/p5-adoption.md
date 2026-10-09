# Handoff: Stage 1 Core P5 (adoption and restart), with the P1 audit fixes

Implementer: Claude Opus, session `sess-1791169804-0110-5d99de2d47b2ff27cfaac8fba5d16d3e`, worktree
`~/botster-sessions/trybotster-botster-core-stage1-p5-adoption`. Reviewer (sol-high): `sess-1791169805-0111-87b8604455fe78bb06902df4c4e64bc3`
(verdicts in `verdicts/p5-adoption.md` on `stage1/review-p5`). Integration reviewer (cross-package PRs):
`sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9`. Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
Brief: `brief-p5-adoption.md`. The P5 pair also owns the merged P1 code (host, facade, storage part of core-sys).

## State (2026-10-04, PAUSED by the lead: the wind-down chain stopped because #165 needs a second round, C3 and C4)

Nothing unpushed: every `stage1/p5-*` branch equals origin.

| PR | Branch | Pushed head | Content | Review state | Resume step |
|---|---|---|---|---|---|
| #162 | `stage1/p5-a10` | `59cda32` | A10 (#152) test fix; merge `314b51c` of P3's #165 at `c03bcfb`; C1 fix | P5 reviewer round 7 NOT CLEAN (`cf96240`): C1 and P5-F12 closed, P5-F4 open (#165's guard waits). Integration: C1 closed (`e39b6b4`), C2 = P5-F4 open (`9b9cc72`). Lead ruling: P3 fixes F4 in #165; no edit from me. | When the lead sends "#165 CLEAN <final head>": `git merge <that exact head>` (never rebase), push, merge-only delta reviews from both reviewers, ONE gate (Linux preferred, for #165's /proc and pidfd branches; Mac otherwise, then the lead needs one Linux run of the guard slow tests before #163), READY naming the machine. Do NOT merge P3's interim head `71195e7`. |
| #164 | `stage1/p5-audit-contract` | `b8b37a6` | A1 #143, A2 #144, A4 #146, A5 #147, A7 #149, A9 #151; both rulings below | CLEAN from both: P5 reviewer `89893b2` (recorded `b91cd55`), integration `9edbc27`. Mac gate at `b8b37a6`: unit 487, slow 60, all pass (`gates/botster-core-stage1-p5-audit-contract-b8b37a6d-mac-20261004-222024-65207.log`). | After #163 merges (merge order below): `git merge origin/v1`, push, merge-only delta reviews from both, ONE gate, READY. |
| none (paused) | `stage1/p5-audit-host-fixes` | `b3b5fac` | #155, #157, the P1 part of #156 (one squashed commit on `b8b37a6`) | No PR, no review. Mac gate on the same tree (`5308cb1`): unit 471, slow 64 + the A10 race. | Before the PR: check each finding against `audits/v1-pr134-141.md` (git show `02acc4b:audits/v1-pr134-141.md`). Unverified: A15, A17, A19, A20, A21, A25, A27, A46, A48, A50. In the notes below: A13, A14, A18, A22, A23, A26, A45. Then fix the commit message if a claim is wrong, write the PR body, and open a stacked PR on #164. |
| none | `stage1/p5-audit-host` | `5308cb1` | the draft history of the row above (same tree) | none | keep for history only |

### Open audit issues of this pair

- #152 (A10): in #162.
- #143, #144, #146, #147, #149, #151: in #164.
- #155, #157, the P1 part of #156: on `stage1/p5-audit-host-fixes`, no PR.
- Not this pair: A29, A31 (payload, P3); A51 and the guard parts of A22 and A50 (P3's #165).

### A15 host duty (Core Amendment 15; after #161, the contracts-v0.1.17 pin)

`CoreLimits.max_key_text_bytes` (default 256, range 1 to 4,096; out of range: `open` fails `InvalidConfig`, 9B). The host refuses a
key whose `text` is over the bound with sync `InvalidInput{field: "text"}` and no `OpId`, checked before any IN-9 computation.
The host admission is P5's; the worker check is P3's. The a15_1 ids stay pending until both harnesses pass. (Different from
audit finding A15, the macOS `start_time` mutation item in #156.)

### Rulings in force

- **Registry layout (lead, audit A1):** `rows/<kind>/<components>.row`; the id in lowercase base32 with no padding, cut into
  components of at most 200 characters; all access by `openat`/`mkdirat`/`renameat`/`unlinkat` on directory descriptors;
  atomic writes; foreign names counted in `diagnostics()["edges"]["foreign_registry_files"]` and left alone. No migration.
- **data_dir boundary (lead, integration K4):** Core creates only `data_dir` (non-recursive `mkdir`); a missing parent fails
  `RegistryFailed`. Every open syncs `data_dir` and its parent, and no other ancestor; a parent that cannot be opened fails
  `RegistryFailed`. Stated in the rustdoc of `Core::open` and `DataDir::open`.
- **P5-F4 (lead):** not waived; P3 fixes it in #165.

### Merge order (lead)

1. #162 carrying #165. 2. #163 (P3 audit fixes). 3. #164. 4. #142 (P7). 5. #161 (pin move to contracts-v0.1.17).

### Exact next steps on resume

1. #162: wait for the lead's "#165 CLEAN <final head>", then the #162 resume step above.
2. #164: after #163 merges, the #164 resume step above.
3. Then: the audit-host PR (row 3); A15 after #161; P5 proper (design sketch below, with the prior-art note); the A52 decision.

### Prior art for adoption (lead asked; to study before P5 proper, not yet read in this session)

Three tools reattach a client to sessions that outlive it: tmux, zellij and shpool. In each, a long-lived server owns the
PTYs, and a new client reconnects over a Unix socket. Before the P5 design is final, read how each one handles these points:
- how it finds the socket and who may connect to it;
- how it checks that the process at the other end is the one it expects;
- what state it replays to the new client;
- what happens when the old client is still attached.
Compare the result with AD-6 (an endpoint only the host's uid can use; proof of the token, the instance and the epoch) and
with the design sketch below, where the worker listens and the new host connects. Record what is reused, and what is not and why
(BUILD.md rule 0).

## #155 decisions (for the reviewer)

- `review.rs` is gone: unique tests moved to `remove.rs` (LC-7), `flow_edges.rs`, `admission.rs` (AM-4, stronger variant), `queue_pressure.rs` (EV-5c merged), `ready.rs` (OR-1); duplicates deleted.
- `World::pump_last_first` is an adversarial order (OR-3, A5-2) that replaces every read of `ready()` in the ordering tests.
- `World::over(&earlier)` opens a handle over the earlier rows and OS process table; `World::configured`/`offering` replace engine-field writes (A18).
- A26: `HostEngine::request_stop` is the one stop entry; two mutant exclusions (dead guards) removed.
- A14: both over-wide exclusions removed (a `LINK_FRAME_CAP` const; a test of a read admitted behind a create and a start).
- A13: TM-5 is one driver test (`due_deadlines_are_processed_in_order_of_due_time`).
- A45 hex helpers now live in `botster_core_link::proof` (`token_hex`, `token_from_hex`); host `EPOCH_KEY` deleted (unused; P6's A34 may add a shared one).
- The driver's scheduler clamps `.min(len - 1)` are removed: the `Scheduler` trait promises an index of the candidates (A23).

## Design of the A1/A2/A4/A5/A9 PR (cross-package: two small testkit edits)

- A5: `HostDriver::open(cfg, edges)` takes no instant; `HostEngine::new(cfg, registry_ids)`; edge results enter through
  `HostEngine::input` with no time; only `pump` calls `Machine::handle(now, Clock)`. The facade `clippy.toml` now disallows
  `Instant::now` and `SystemTime::now` (TM-1), with an allow at the top of the two facade slow-test files (the test is the host).
- A2: `HostDriver::open` reads the row ids through `edges.read_rows(ROW_PREFIX)` (one code path for real and testkit);
  `Create` refuses an id in `unadopted` with `IdInUse`; open fails `RegistryFailed` when the read fails.
- A1: `AdoptAll` recovers every row. `Row::decode` rejects non-JSON, another version, or a row whose id is not its key's: the
  session is `Lost(RegistryCorrupt)` with a minted instance and `unknown_request()` (size 0x0). A row of a session this
  handle holds is skipped. Decodable non-`Created` rows stay `Lost(Other)` (P5 placeholder; no test asserts it).
  Header-damaged row files (key unreadable) are still dropped by `FileStorage::list_rows`: no id is known. Open item.
- A4: the `Stop` waiters of a failed `Starting` row write complete with `registry_failed(e)`.
- A9: new `Action::ProbeIdentity` / `Input::IdentityState` and `HostEdges::identity_state`. Remove of a worker that cannot be
  asked probes; Matches -> Kill; Absent/Reused -> gone. The remove grace probes again instead of re-killing blindly.
  Testkit `Spawner::identity_state` (required method): `WorkerSpawner` answers for its own processes only. Cross-host
  process modelling in the testkit is a P5-proper item.
- Tests: `src/tests/registry.rs` (A1, A2, A9), `flow_edges.rs` A4 test replaces the test that pinned the skip;
  `World::over(&earlier)` opens a new handle over the earlier rows and OS process table.

## Decisions pending (lead asked to record here)

- A52 / `PayloadId.start_time` 0 sentinel vs `Option<u64>`: decide during P5 adoption (AD-6, AD-7, A10). Not decided yet.

## P5 proper: design sketch (draft, not reviewed)

Prior art: the old daemon adopted "through a reconnectable control socket" that the worker listens on, and the host
connects (`runtime/worker_process.rs::adopt_reserved_inner` at `72b2e33`). AD-6 says "worker or guardian endpoints are
readable and connectable only by the host's uid". The hello proof binds the host epoch, so a worker cannot open a link to a
host whose epoch it has not seen: the new host connects to the worker.

1. **Worker endpoint.** The worker binds a listener at `<data_dir>/w/<short id of the instance>` (directory `0700`), given
   in its launch arguments. It keeps listening for its whole life. `open` checks that this path fits a socket address, as
   it does for the control socket (A47).
2. **AdoptAll, per row:** `Created` stays `Created`; a rejected row is `Lost(RegistryCorrupt)` (done); `Starting` with no
   identity is `Lost(StartInterrupted)`; otherwise probe the identity (A9's `ProbeIdentity`): `Absent`/`Reused` give
   `Lost(WorkerGone)`; `Matches` gives a connect to the endpoint and the adopt handshake.
3. **Adopt handshake (AD-6, DP-8, AD-4).** The host sends `Hello{T, instance, proof(token, instance, new epoch), epoch}`.
   The worker checks the proof with its token, the instance, and that the epoch is above every epoch it has seen; it then
   fences the old host (closes the old link) and answers with its own hello (its protocol P). The host checks the answer:
   P outside {T, T-1} gives `Lost(WorkerVersion)`. A connect that fails, or no answer within `startup` (the transcript
   `a6_1_withheld...` advances 60 s), gives `Lost(WorkerUnreachable)` (decision: `startup` is the reachability deadline;
   record in DESIGN.md).
4. **Adoption report.** After the handshake the worker sends its state: payload running or exited (code, signal), the
   terminal state, features, formats, the payload identity, its routes (`RouteAdopted`, P4a) and focus (DP-12). The host
   posts one `SessionState` per AD-1: `Running` (or `Exited`), `Stopping` with the stop re-issued and `stop_grace` from
   the adoption, `Exited` final-model workers stay `Exited`.
5. **A10/A11.** The impostor control answers the handshake with a wrong token or instance; Core never signals it and
   accepts no later frame from it (a link that failed AD-6 is closed and its frames are never decoded).
6. **Testkit.** The `Sim` keeps workers across a host drop (it does). It needs: worker endpoints by path (in memory), a
   `connect_worker` host edge, and a process table shared across the hosts of one harness (identity probes see the old
   host's workers). Cross-package with P6 (testkit) and P3 (worker machine and binary): integration reviewer.
7. **Waits on others:** route ids (P4a), service ids (P7): stay pending with that reason.
8. **A52 (lead):** `PayloadId.start_time` 0 sentinel vs `Option<u64>`: decide in step 4 (the report carries the payload
   identity); leaning to `Option<u64>` with "unknown never matches", because a sentinel is a value that looks valid.

## Guard race (2026-10-04, root cause of the Mac gate failures of #162 and P7's #142)

On macOS a group kill (`killpg`) is not atomic against `fork`: a child whose fork completes after the kill listed the
members escapes it (Linux closes the window). The guard fixture printed readiness before forking `echo | tee fifo`; an
escaped `tee` held the parent's stderr pipe. Every `GroupGuard` user must kill only a quiet group: start the last process,
then report readiness with a builtin. When the audit-host branch is merged with v1, its moved guard tests and its
`waiting_child` helpers must follow this rule (they background `cat` before readiness already; recheck `blocked_parent`).
Both Mac gates (P5 #162, P7) started at 21:28:01: the lead asked whether they overlapped despite `botsterq --exclusive`.

## Queued (lead, 2026-10-04): Core Amendment 15 (final, manifest final35, contracts main 1725abf)

Small PR after the audit fixes, after #161 (contracts-v0.1.17 pin): `CoreLimits.max_key_text_bytes` (default 256, range 1 to
4,096; out of range: `open` fails `InvalidConfig`, 9B), and the host refusal: a key whose `text` is over the bound is sync
`InvalidInput{field: "text"}` with no `OpId`, checked before any IN-9 computation. Coordinate the split with P3 (host
admission is P5's, the worker check is P3's). The a15_1 ids stay pending until both harnesses pass.
