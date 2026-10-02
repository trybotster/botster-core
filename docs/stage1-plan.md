# Stage 1 plan: Botster Core v1

Owner: the Stage 1 Core lead (Claude, session `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`). Branch `stage1/plan`, cut from `origin/v1` (`d91495eb8b6e4c730177c1d440d2d06ce762caa5`). Date: 2026-10-01. Status: plan for review (Sol). No product code is written by this plan.

`docs/BUILD.md` of botster-contracts is binding over this plan. Where this plan and BUILD.md differ, BUILD.md wins, and the difference is a defect in this plan.

## 0. Pins

Every input of this plan, at a fixed revision. An implementer reads these revisions and no others.

| Input | Revision |
|---|---|
| botster-contracts (BUILD.md, steward rulings, runners, transcripts) | tag **`contracts-v0.1.13`** = commit `a8db5c9a0f43fc440564989fd38a55121fdda38b` (manifest final30). Revisions 1 to 6 read `9666bf5`, 7 to 9 `2f2996e`, 10 and 11 `contracts-v0.1.1`, 12 `contracts-v0.1.3`, 13 and 14 `contracts-v0.1.6`, 15 `contracts-v0.1.7`, 16 and 17 `contracts-v0.1.8`, 18 and 19 `contracts-v0.1.9`. Doc changes between `contracts-v0.1.9` and `contracts-v0.1.13`: steward rulings R-21 to R-28, of which only R-28 is Core's (kitty flags, release events and zero output follow the terminal model: an unknown flag set the model ignores posts nothing; a release with flag 2 off and a legacy release are `NotWritten(NotReported)`); `docs/tooling.md` adds full_moon (Lua, not Core); BUILD.md unchanged. Manifests final23 to final29 change no Core file. The tag also fixes the conformance driver's `await_*` idle check (await-recheck). |
| Joint manifest | `frozen/current/MANIFEST.md`, tag `manifest-final30` = commit `037200cbf0d261f10a3a73c427e0f664d422b9e5` (final30 = final29 + Core Amendment 13; final23 to final29 are Hub, plugin and Lua amendments with no Core file). Before that: tag `manifest-final22` = commit `0c91752c68eb87e155c77838ad196e86468370ca` (final22 = final21 + Core erratum 4); tag `manifest-final21` = commit `4a1d0b773eeefd3f0c501c85ac00b65ae68386b0` (final20 = final19 + Core A11, `891aab2285fc30203ecf39549943b1bada76294d`; final21 = final20 + Core A12). Before that: tag `manifest-final19` = commit `13525f6bc07c638f4f544a63e20d8cfa8846ea53` (final17 = final16 + Core A9, `b6a4cfba409d069fc952740d97896900b41ada79`; final18 = final17 + Core A10, `ae0cd4f55848bfd5c90448e28694b5a990ad96a4`; final19 = final18 + Core erratum 3). final16 was `6b9b280ed28deef48092bb419a83fdbeb247e640`, final15 `ff5e7745b0928b7b64b54c29d0f0e00ae7e6d677`, final14 `063d6f05a9bd2e02fe8d252e82aff72c34a25d01`. |
| Core Amendment 9 (A9-1: `max_frame_bytes` must carry the route's own `attached` frame, else sync `InvalidInput`; A9-2: every attached route can carry the minimum `output_dropped` frame, withdrawing two A3-1 ids; A9-3: the testkit's scheduler edge makes DP-3's per-frame compression choice, and an ineligible payload is never compressed) | `frozen/current/core-contract-v1.17-amendment-9-candidate1.md`, sha256 `860604ba6a54d42d9dc5dd9e00fed9e4ba2bde1c2d6be44b420e4b0fdce8b042` |
| Core Amendment 10 (A10-1: the process edge can present an impostor at the adopt handshake, and Core never signals it; A10-2: the storage edge can corrupt one stored row, which becomes `Lost(RegistryCorrupt)`) | `frozen/current/core-contract-v1.17-amendment-10-candidate1.md`, sha256 `9fc505025fdf7292dd30b919684ddd3bb78257ebbae3b26eb4a528e6d656b768` |
| Core Amendment 11 (A11-1: after Core's real AD-6 check rejects an impostor's handshake, worker-shaped frames from that impostor change nothing) | `frozen/current/core-contract-v1.17-amendment-11-candidate1.md`, sha256 `beac85d513a1bc6e464f384b15e2b4f2a8d69b42af79fdbc8ee58db38106274e` |
| Core Amendment 12 (A12-1a: the process edge can hold the worker's next report at the control-link boundary, then release it or lose it; A12-1b: a scheduler point inside the worker's `Remove` cleanup, after it started and before its result; a partial cleanup claims no paths) | `frozen/current/core-contract-v1.17-amendment-12-candidate2.md`, sha256 `bc0f20ae2577ef54ea91ce05828ac7123f10d0b7dc98eab970bce64ea2898a9d` |
| Core Amendment 13 (A13-1: `ClipboardWrite{id, selection, contents, total_bytes, reason?}` for every clipboard write the terminal model recognizes (OSC 52, OSC 1337 Copy, OSC 5522); selection from the OSC 52 string, else the model's location (standard `c`, primary `p`, selection `s`); all representations atomic; `[]` means clear; TooLarge carries no contents. A13-1b: the worker alone acknowledges OSC 5522, one contiguous AM-2 transaction, no `input_rev` change, SUCCESS within `clipboard_bytes` else IO_ERROR. A13-2: replaces EV-3 in full; reads unchanged) | `frozen/current/core-contract-v1.17-amendment-13-candidate5.md`, sha256 `ec5debacd1d0affd4de08eaef3f1747646b80da05504e304cf890ac2c35edb20` |
| Core erratum 4 (E4-1: the reserved admission cost of a query reply: a client reply reserves the size of its `0x81 input` frame as it arrived; the shadow fallback reserves the length of the reply bytes the worker writes; a reply one byte over the free bound is parked) | `frozen/current/core-contract-v1.17-erratum-4-candidate1.md`, sha256 `d67542855e4d38ad222e5882d1ec735956de3ce9fa71063b7aaaf5125bcfe5c4` |
| Core erratum 3 (E3-1: due effects and the `pump_events` budget: an effect with no event runs in its pump; a step atomic with its event runs only with budget, else is carried whole with `more = true`; carried steps first; a carried step meeting a full mandatory queue parks; due order among runnable steps only; `Silent` in the first pump with budget) | `frozen/current/core-contract-v1.17-erratum-3-candidate3.md`, sha256 `77291006b7ccc89e101917315e2248d8c8a31a5210a3e0120f69ebc63b83ae3b` |
| Core Amendment 7 (A7-1: an `attach` or `AttachWebRtc` option out of its OU-1 range is refused synchronously with `InvalidInput{field}`, nothing reserved; the chunk cap above the SCTP maximum is clamped, not refused) | `frozen/current/core-contract-v1.17-amendment-7-candidate4.md`, sha256 `1d1a40c80cff6253faa0878602d6079c4669ed75f1fd12280cc77b3b9a4b8bcf` |
| Core Amendment 8 (A8-1: a capture reserves `max_snapshot_bytes` at `begin` until the host polls its `Completed`, then counts `total_bytes` or nothing; A8-2: a snapshot that cannot be formed within its bounds, including pending parser state the model can no longer carry for any reason with no ground state available, is `SnapshotTooLarge`) | `frozen/current/core-contract-v1.17-amendment-8-candidate5.md`, sha256 `c6918a9bf19d739422abc775441a9aa42811bbe044f1a18cd39f20215e2a8006` |
| Core erratum 2 (accepted `19668c9`): E2-1 no `TitleChanged` for OSC 1; E2-2 `other_modes` are the tracked modes with no normative field; E2-3 `ModesChanged` compares the final flags after each `vt_write` with the last posted value | `frozen/current/core-contract-v1.17-erratum-2-candidate2.md`, sha256 `d0eb896737ecc54e11baf606afd784b2ee3f6555ff34c2840228dd10537a09bb` |
| Steward ruling R-13 | SGR-pixels reports zero-based pixels; the 5.1A `+1` is for cells only |
| Core contract v1.17 | `core-contract-v1.17.md`, sha256 `8ee7ca05300156312363a00bf3bafae99674d0f09fc349866f71bb2f3a6d3486` |
| Core Amendment 2 | sha256 `5a596f4407795616395dbb41b38a7e64381ac54dfe8d0a2a9166c36ae1a5b180` |
| Core Amendment 3 | sha256 `10e0424eda74d588a21bba3051e9f31078aa0f66ae56c98001e8b02d24518a80` |
| Core Amendment 4 | sha256 `e78576d3455b1fa88f6a63581038441a5135f408280cc97cbe2cf47590089ddb` |
| Core Amendment 5 | sha256 `72c05f4b5ef8e3808536d23f0a7f56640e2d4b6fa1b66c0c7433ba157b1e89e8` |
| Core erratum 1 | sha256 `0a7578bb6eabe7c1b2eca6f673965d13eddb7cc59e1332e3187b1f9b62a37709` |
| Route codec revision 36 (Core adopts it verbatim, A3-1) | sha256 `1c6b16948450a327fa969a445a4249d82b3370cc80c8983bbb59ecada3d307a0` |
| Core Amendment 6 (accepted as candidate 3; ACCEPT `5d2dd45`): A6-1 the testkit service-lane edge and the withheld control link; A6-2 first worker protocol 1 and the enumerated deferred set; A6-3 uploads at Remove | `frozen/current/core-contract-v1.17-amendment-6-candidate3.md`, sha256 `572101467891dfc3c60ce7496ba458ff614e8fd9e0a4b3abac9caae30d4bbf5d`, in manifest final13 |
| Steward rulings | R-1 to R-12 in `docs/steward-rulings.md` at the botster-contracts commit above |
| Old botster-core (prior art, read-only) | `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` (= `origin/main`). The local `main` of `~/Projects/botster-core` is a stale ancestor (`053148f`). Never read it. |
| Ghostty fork (libghostty) | **Moved by revision 18** (orchestrator's Q1 approval, in its order: upstream first, then a minimal fork patch series): `https://github.com/trybotster/ghostty` branch `botster/vt-core-stage1-b` at **`85a8d8eb197c5752887c017c9a3faa6f1dc1969b`**, on upstream `ghostty-org/ghostty` `main` at `83edd491e3024ae5e50393d62877b8897da1cccd`. The patch series (17 commits): the `startHyperlink` lifetime root-cause fix; the notification source (A2-4); the paste marker frame without payload rewrite (IN-8, DP-5); the query effect with exact request bytes and `vt_write_until_query` (EV-8, R-17); key events with hyper, meta, shifted and base-layout keys, F26 to F35 (5.1A); the `xterm-ghostty` terminfo name and source export (TI-1); mouse cells as given and the active tracking and format getters (IN-9, ST-4, R-13); the typed query reply encoders (EV-8); the OSC 52 selection and terminator (EV-3, EV-8). Reviewed CLEAN by the P2 reviewer (`stage1/review-p2` `1fb4c3e`) with the audit at `stage1/p2-libghostty` `52b3d86`. Upstream supplied OSC 133 prompt marks and clipboard read. The nine old fork-only SEGV commits are not carried: the audit maps each to an upstream fix or states why it is moot. Nothing was sent upstream. The earlier pin was `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`. |
| Zig | `0.16.0`, unchanged by the Ghostty pin move. The lib-vt build needs seven Zig packages, prefetched into an immutable package store; `build.rs` fetches nothing (the Linux gate has no network: the fork ref and the seven package URLs and hashes go to `ci/remote/fetch.sh` through the infra engineer). The vault note "Ghostty needs 0.15.2" is stale for this pin. |
| Rust toolchain | `1.97.0` (the Hub's CI pin), in `rust-toolchain.toml` |
| Earlier rebuild plan (a guide only) | `~/botster-sessions/shared/core-rebuild-plan-v1.md`, sha256 `d1f05eec3b953819e83b147856e8509640247f0fc69d1287c75f230ce876b4d5`. It was written against Core v1.5, so section 4 overrides it. |
| Ghostty query inventory | `~/botster-sessions/shared/ghostty-query-inventory-20260930.md`, sha256 `f5d59a256e45221b75f8d89efd3ff604b7deb7eb131a09a46eb3137e79e69e43` |
| Clause ownership | `docs/stage1-clauses/*.txt`, generated by `docs/stage1-clauses/owners.py` from the ledger at the botster-contracts commit above |

**Dependency pin for code.** Stage 1 crates depend on botster-contracts by git **tag** (foundation design section 11). Stage 0 cuts the first tag. P0 pinned `contracts-v0.1.1`; each package moves to the tag the lead names, one commit per move. The current tag is `contracts-v0.1.13`. Each later move of the pin is its own commit, and the commit names the ids that changed.

## 1. What Stage 1 delivers, and when it is done

Stage 1 builds the real Botster Core on the `v1` branch of botster-core. It has these parts:
- the Core library (`Core`, which implements `CoreApi` from `botster-core-contract`);
- the session worker as a library type, and one thin binary;
- the service guardian;
- the libghostty binding;
- `botster-core-testkit` (Core A5-1);
- the example consumer (Core section 15).

**Done when** (BUILD.md Stage 1):
1. The Core conformance suite (`botster-core-conformance`, all 652 active Core ids of the ledger at the pinned tag (654 minus the 2 withdrawn by A9-2)) passes on **real Core in-process through the testkit**, under the CI seed set.
2. The same suite passes on **real Core with real worker and guardian processes** (the slow tier).
3. Each id that only a real OS condition can prove (A5-3: a real crash, fsync, descriptor handoff, `payload_dies_with_guardian`, and the others that the replacement map names) passes as a **named real-process test** that cites its clause. On the testkit, the suite lists that id as real-only (A5-3).
4. The section 15 acceptance script passes as a test.
5. **Deferred ids (A6-2, after its acceptance only).** Zero ids may be failed or pending. The only ids that may be neither passed nor pending are those in `conformance/core-deferred.toml` that the gate validates (section 5): A6-2's complete deferred set, which is exactly `conf::ad_4_previous_worker_version_adopts` (until the first release whose worker protocol T is 2) and `conf::ad_4_missing_worker_capability_is_unsupported` (until the first release whose protocol adds a worker feature that the pinned T − 1 worker lacks). Everything else passes now, including:
   - `conf::dp_12_unreachable_or_n_minus_1_worker_leaves_focused_unknown`: its unreachable-worker case is active and must pass, with the worker alive and its control link withheld past the deadline (`Lost(WorkerUnreachable)`, A6-1); its previous-worker case is not applicable (`focus_report` is in protocol 1), as A6-2 records;
   - every `CursorReadUnsupported` case, through its own causes (no worker model for the session), not through an older worker;
   - `conf::a2_6_per_worker_features_in_worker_features` and `conf::dp_12_focus_report_capability_is_advertised_per_worker`, with the current worker.
   Stage 1 builds the A6-2 behavior that is testable now: worker protocol 1, `adoptable_worker_protocols() = {1}`, and an out-of-set hello gives `Lost(WorkerVersion)`. A6 is accepted (manifest final13). The two AD-4 ids move from pending to deferred in the commit that pins a contracts tag containing final13 (section 5 rule 2).
6. Apart from item 5, only `passed` counts. `unsupported_control`, `inconclusive` and `not_applicable` are not passes (foundation design 6.1), except `not_applicable` for a feature that `features()` honestly reports absent.

The C ABI (section 13) is **not** built in Stage 1. The facade rules that section 13 places on the Rust API are kept (no callbacks into host code, no borrowed data across calls, explicit discriminants).

## 2. Architecture

### 2.1 One idea: every component is a sans-IO state machine

Every component that contains contract logic is a **sans-IO state machine**. It takes inputs, it produces actions, and it reports its next deadline. It performs no I/O, reads no clock, spawns no thread and draws no random number. A **driver** connects the machine to the world.

The machine interface is the same for every component (the shape that str0m uses, section 7):

```rust
// Shape only; P0 writes the real signatures.
trait Machine {
    type Input;   // a byte read, a socket readiness, a child exit, a timer, a host call
    type Action;  // a byte write, a spawn, a signal, a storage write, an event for the host
    fn handle(&mut self, now: Instant, input: Self::Input);
    fn poll_action(&mut self) -> Option<Self::Action>;
    fn next_deadline(&self) -> Option<Instant>;
}
```

There are two drivers for each machine:
- the **real driver**, which wires real sockets, the real PTY, real processes, real files and the real clock;
- the **testkit driver**, which wires in-memory edges and a seeded scheduler (section 4).

**The machine is the same code in both cases.** This is how BUILD.md's in-process worker rule and Core A5-1's "one code path" are met: the worker logic exists once. A difference between the two runs is a bug in an edge or a driver, never a reason to branch the machine.

Why this shape, and not threads or async:
- **Determinism.** A machine that reads no clock and spawns no thread is deterministic for a given input order. The testkit controls that order, so A5-1 ("the same script, seed and inputs give the same events and results") holds by construction.
- **Core already is this shape.** Core's host API is `begin`, `pump(now)`, `poll_events`, `next_deadline` (section 2, TM-1, TM-3). The worker and the guardian get the same shape.
- **Perturbation at the source.** A5-2 requires every seeded variation to come from the scheduler, the edges or Core's own consumption boundary. With machines, the scheduler chooses the order of ready inputs, and the edges choose chunk sizes. Nothing reorders results after the fact.

### 2.2 The components

| Component | Machine | Real driver | Testkit driver |
|---|---|---|---|
| Core host (the library in the host process) | `HostEngine`: registry model, admission table (AM-1), op table (A2-1), event queue classes M, K, D, L (EV-2, EV-5, EV-6), captures (ST-6), deadlines (TM-3), adoption logic (AD-1), service bookkeeping (SV-3, SV-6) | `Core` in `botster-core`: `pump(now)` reads the control links, feeds the engine, and executes its actions | `TestkitCore` in the testkit: the same engine, with in-memory links and the seeded scheduler |
| Session worker | `Worker`: PTY input admission (AM-2), input encoding (IN-8, IN-9), guards (IN-10), the terminal model through `botster-terminal-ghostty`, route state machines (OU-2), egress queues and backpressure (OU-3), resync (OU-9), queries (EV-8), files (DP-5b), focus (DP-12), input records (ST-7), the tap (TP-1) | `botster-worker` binary, role `session`: a `mio` loop over the PTY master, the control socket, the route sockets, the UDP sockets of WebRTC routes and the child-exit watch | the testkit runs the same `Worker` on the test thread, stepped by the scheduler, with the scripted program edge and in-memory route streams |
| Service guardian | `Guardian`: the service's identity, orphan grace (SV-8), exit status (SV-5), log ring (SV-9), tree kill (SV-9) | `botster-worker` binary, role `guardian` | the same `Guardian`, stepped by the scheduler, with the process edge |
| Service lanes | part of `HostEngine` (Core listens, SV-2) | real Unix listeners per lane | in-memory lane endpoints to a scripted service (A6-1) |

**One binary, two roles.** `OpenConfig` carries only `worker_path` (LC-1), and Core fixes no file name (erratum 1). So the guardian is the same binary in its `guardian` role. The topology and the binary name of the guardian are implementation-defined (section 10), so this choice needs no contract change. One binary also keeps the install layout (HC RT-3: `botster-worker`) at one file.

### 2.3 Edges

An **edge** is a trait that a machine's driver calls for one kind of I/O. Each edge has a real implementation and a testkit implementation. BUILD.md's HARD Phase 1 requirement lists the sources of nondeterminism. This plan maps each to one edge.

| Edge | Used by | Real implementation | Testkit implementation | Faults it can inject (A5-3) |
|---|---|---|---|---|
| `Clock` | worker, guardian drivers | `Instant::now()` in the driver only | the virtual clock: the latest `now` that the host passed to `pump` (TM-1), or `Sim::advance_to` | none |
| `Storage` (the registry) | host engine | one file per row under `data_dir`, written atomically with fsync of the file and the directory; `flock` on a lock file for LC-2 | in-memory rows | a failed write; a write whose effect is unknown (`RegistryFailed{uncertain: true}`, AD-7) |
| `Process` | host engine (spawning workers and guardians), guardian (spawning the service) | `posix_spawn` or `fork`+`exec` with `pre_exec` descriptor control, process groups, rlimits, `killpg`, identity by pid and start time, exit watch by kqueue `EVFILT_PROC` (macOS) or pidfd (Linux), descendant enumeration | the testkit creates an in-process `Worker` or `Guardian` and wires its links | spawn refused with an `errno` (`StartFailed{ExecFailed{errno}}`); a worker or guardian ended at a script point (`Lost(GuardianLost)`, `WorkerGone`); a worker kept **alive with its control link withheld** past the deadline, which gives `Lost(WorkerUnreachable)`, not `WorkerGone` (A6-1, AD-2) |
| `Program` (the payload on the PTY) | worker | the PTY master and the payload's process group | the scripted program: the probe script interpreted in-process (writes output, reads input, exits with a code or a signal at script points) | write-size and read-size variation (A5-2); `pty_blocked` |
| `Link` (the control link to a worker or a guardian) | host engine, worker, guardian | a Unix stream socket, with `SCM_RIGHTS` for the route descriptor | an in-memory duplex with descriptor objects passed by value | a broken link (`break_control`, `lose_worker`) |
| `RouteTransport` | worker | a connected stream descriptor (DP-2), or a UDP socket for a WebRTC route | an in-memory connected stream; an in-memory datagram pair for WebRTC | read and write size variation; peer close; write failure |
| `ServiceLane` | host engine | a Unix listener per lane in a 0700 directory (SV-2) | in-memory connected streams to a scripted service that sends the real SV-2 preamble (A6-1) | peer close, truncation, oversize (SV-2). Peer OS identity, orphan grace, `payload_dies_with_guardian`, descendant enumeration and the SIGTERM-then-group-kill stop stay real-process only (A6-1). |
| `Wake` | host engine | a pollable kqueue or epoll descriptor over all control links, plus a self-pipe for TM-6 runnable work | an in-memory level flag; under shuttle, built on shuttle primitives | a spurious wake (A5-1) |
| `FileSink` (DP-5b) | worker | a request and completion interface; the operations run on one file-I/O thread per worker, outside the readiness loop (2.3b) | the same interface; the scheduler chooses when each completion is delivered; the effect is a real file in the test's temporary root (files are not sockets or processes, so the default tier allows them, BUILD.md testing rule 1) | a failed create or write (`write_failed`) |
| `Scheduler` | every driver | the production policy: control first, then round-robin over sessions with `pump_bytes` and `pump_events` (9B) | a seeded ChaCha8 policy that chooses which ready input is handled next, how much work one `pump` does, and the batch size of `poll_events` (A5-2) | none |
| `Entropy` (random values, 2.3a) | host engine, worker, guardian, the WebRTC stack | the OS CSPRNG (`getrandom`) | a ChaCha8 stream seeded from the test seed, separate from the scheduler's stream | none |

### 2.3a Random values

Every random value comes from the `Entropy` edge, and from nowhere else. Consumers:
- the host engine: the per-worker token (AD-6), the service secret `CORE_SERVICE_SECRET` (SV-1), and the `InstanceId` of a session (ID-1) if P1 makes it random;
- the WebRTC stack of the worker (P4c): DTLS certificates and keys, ICE credentials, SCTP and DTLS nonces. The P4c spike (R6) audits every random source of the chosen stack and wires it to `Entropy`. If a source of the stack cannot be injected, P4c reports BLOCKED with the list; the lead takes it to the orchestrator before the stack is adopted.

Values that do not need to be random are **counters**, not random: `OpId`, `RouteId`, `CaptureId`, the worker's `query_id` (EV-8: "opaque, minted by the worker"), and the uniqueness suffix of an uploaded file name (DP-5b, A2-9).

**Production keeps secrets unpredictable:** the real edge is the OS CSPRNG only. The seeded stream exists only in the testkit crate. No production crate can construct it, and the facade cannot select it.

**Hidden randomness is banned in the machine crates.** `std::collections::HashMap` and `HashSet` with the default `RandomState` give a per-process iteration order. P0 adds them to `disallowed-types`. Machines use `BTreeMap`, `BTreeSet`, or an explicit fixed hasher.

### 2.3b File input never blocks the worker loop (DP-5b)

DP-5b: "file I/O never blocks the PTY reader or another route". A file system call can block. So the `Worker` machine never calls one. It emits a **file request** action and receives a **file completion** input.

- **Requests:** `Create{file, dir, name}` (exclusive create, A2-9 name rules), `Write{file, bytes}`, `Close{file}`, `Delete{file}`.
- **Completions:** `Created{file, path}`, `Written{file, n}`, `Closed{file}`, `Deleted{file}`, and `Failed{file, errno}`. Each one is an input to the same `Worker` machine.
- **Real driver:** one file-I/O thread per worker process runs the requests in FIFO order. It wakes the `mio` loop through a `mio::Waker` when a completion is ready. The thread lives in the driver (`botster-core-sys`), not in the machine.
- **Bound and backpressure:** the request queue is bounded per route by `max_chunk_bytes` worth of pending writes. When a route's file requests are at the bound, the worker stops reading **that route** (transport backpressure, as DP-5's input queue). Other routes and the PTY continue.
- **Order:** the requests of one file run in order. Completions of different routes have no promised order. The path paste of a `file_commit` is admitted only after its file's `Closed` completion, at the commit's place in that route's receive order (DP-5b, AM-2). The route's later input waits behind it; other routes do not.
- **Abort and route end before the commit:** the worker drops the queued writes of that file and requests `Delete` of the partial file. It sends no frame (DP-5b).
- **Cleanup at `Remove`** follows LC-7 step 3 as Core Amendment 6, A6-3, states it. The worker deletes the uploads, because it knows their names. The cleanup result is decided only on **completions**:
  1. When `Remove` reaches the worker, every open upload ends: queued writes are dropped, and the worker waits for each request already running on its file thread (`Created`, `Written` or `Failed`).
  2. The worker requests `Delete` for every file that its routes wrote, and for every partial file, and waits for every `Deleted` or `Failed` completion.
  3. The worker sends one **complete** result on its AD-6-authenticated link, and then ends: `Deleted` (every file is gone, or there were none) or `NotDeleted{reason: delete_failed, paths}`, where `paths` are **exactly** the files whose `Delete` failed.
  4. The host engine reports `NotDeleted{reason: outcome_unknown}`, and claims no path, when it has no complete trusted result: the worker is unreachable (a `Lost` session with no stray worker found), the worker is lost during the cleanup, the result is lost, or the result came on a link that failed AD-6. `Deleted` is reported only from a complete result of the authenticated worker (A6-3).
  5. **The teardown always continues** (A6-3): after the result or the `outcome_unknown` decision, the host engine advances LC-7 to step 4 (the durable row) and step 5 (the id is free). `Completed{Remove}` carries `RemoveReport{uploads}` (A6-4 item 1); `SessionState{Released}` and `Completed{Remove}` follow step 5 as LC-7 says. No `Deleted` is reported while an uploaded file may remain.
- **Owners:** P4b, the worker side (steps 1 to 3); P1, the host side (step 5, `RemoveReport`); P5, the `Lost` and restart cases (step 4, and the stray-worker path).
- **Testkit:** the same request and completion interface. The scheduler chooses when each completion is delivered (a choice point of 2.4). The worker logic is the same in both runs.

### 2.3c No test branches

**Edges are dependency injection, never test branches** (BUILD.md testing rule 8). No crate that contains a machine has a cargo feature or an environment variable that changes behavior for tests. P0 adds a `clippy.toml` `disallowed-methods` list to the machine crates: `std::time::Instant::now`, `std::time::SystemTime::now`, `std::thread::spawn`, `rand::*`, `getrandom::*`, `std::env::var`, and every `std::fs` function (file I/O goes through `FileSink`, 2.3b); and `disallowed-types` bans the default-hashed `HashMap` and `HashSet` (2.3a). A machine that uses one fails CI. The drivers, the `sys` crate and the testkit are exempt.

### 2.4 The scheduler

- **Production policy.** The real driver handles control-link input before route and PTY I/O (the old worker's "control first" turn, `botster-session-worker.rs` at the old SHA). The host engine visits sessions round-robin, with the `pump_bytes` and `pump_events` bounds of 9B.
- **Seeded policy.** `with_seed(n)` (A5-2) seeds one ChaCha8 stream (the generator that botster-contracts already uses for its seeds). Each choice point draws from it. The choice points are the A5-2 list, plus one edge timing that A5-2 does not name and that varies only an order the contract leaves open: when a file completion is delivered (2.3b; DP-5b gives completions of different routes no order). The A5-2 list:
  - which ready work runs first in a `pump` (OR-3);
  - whether an operation's progress is deferred to a later `pump` (never zero pumps, OR-1);
  - how many pumps pass between `Start` and `Running`, and between `Stop` and `Exited` (the program and process edges);
  - the bound on the work one `pump` does;
  - the size of the non-empty part of the queue that `poll_events` returns;
  - how many updates of a class K key happen before the host polls;
  - the write sizes of the program edge, and the read sizes of the route-transport edge;
  - spurious wakes;
  - the place of a due deadline's event among the other events of its `pump`.
- **The terminal model is fed in steps, and input is admitted only between them** (Core erratum 2, E2-3; EV-8 (c), (g); R-17). A **model step** is one completed call into libghostty that consumes a PREFIX of the PTY bytes the worker has read and not yet fed: `vt_write` for bytes with no query, or `vt_write_until_query`, which stops right after a recognized query and reports how many bytes it consumed. Rules:
  1. The unconsumed suffix is retained, in order, and fed by later steps; no byte is dropped, reordered or fed twice (OU-12 still carries every byte to the routes in order).
  2. After a step that ends at a query, the worker handles the query (EV-8: offer to the eligible route with the host-supplied deadline, hold the shadow reply). When `pending_queries` is full, the worker feeds NO further step until a slot frees, and output after that query is held in order (EV-8 (g), source backpressure).
  3. After EACH step, the worker compares the model's final `ModeFlags` with the last posted value and posts one `ModesChanged` only if they differ (E2-3).
  4. Input is admitted only between COMPLETED steps, never inside one, and each write uses the model's modes at that admission point (IN-8, IN-9, AM-2).
- **Not varied:** which class D event is dropped (always the oldest, EV-2), and every order that the contract fixes.
- **Shuttle.** The Core handle is `Send` and not `Sync` (TH-1). The `WakeHandle` is used across threads (TH-2). Shuttle explores the cross-thread schedules of the wake handle and of a Hub that drives the testkit under shuttle-tokio (A5-4). The testkit's `Wake` edge has a shuttle variant. The machine crates have no `shuttle` feature (testing rule 8), because the shuttle code is in the edge, not in the machine.

### 2.5 The real host loop needs no thread

`Core` starts no thread. The `WakeHandle` descriptor is a `polling::Poller` (kqueue on macOS, epoll on Linux) plus a self-pipe for "runnable work exists" (TM-6). A kqueue or epoll descriptor is itself pollable, so `WakeHandle::fd()` (section 13 rule 5) works in the host's loop, and `wait(timeout)` from another thread (TH-2) only waits on it. `Poller` is `Sync`, so the owner thread can change registrations while another thread waits. It is elegant because it adds no thread and no lock to the handle.

**Every descriptor that can bring host work is registered, level-triggered, with an interest that follows its queue state:**

| Descriptor | Read interest | Write interest | Clause |
|---|---|---|---|
| a worker or guardian control link | only while the link's **bounded receive buffer** has room and the engine can consume the next message (rule 7) | only while the link's outbound queue is not empty | TM-3, TM-6, IN-6 (`SessionWritable`), EV-5b |
| a service lane listener (`lane-<n>`) | always, while the service exists; a readable listener means a connection to accept | none | SV-2 |
| an accepted lane connection before its preamble is complete | always, until the preamble is complete or the connection is closed. **After Core has read a valid preamble, read interest is removed**: the lane is staged, and no payload frame is read (SV-6) | none | SV-2, SV-6 |
| an authenticated lane socket | **only while the lane's epoch is committed AND that lane's inbound queue has room for one more frame** (`inbound_queue_bytes`). A staged lane of an uncommitted epoch has no read interest (SV-6: "Until then no payload frame is read or delivered"); the commit restores read interest on every lane of the epoch whose inbound queue has room. When the queue is full, Core removes read interest and stops reading, and the kernel buffer back-pressures the service (SV-3: inbound is lossless) | only while that lane's outbound queue is not empty | SV-3, SV-6 |
| the self-pipe | always | none | TM-6 |

**Rules that keep progress without a wake loop:**
1. `pump` reads only descriptors with read interest and writes only descriptors with write interest. It never spins on a descriptor that it cannot serve.
2. `service_recv` that frees room in a full inbound queue restores the lane's read interest before it returns. If the kernel already holds bytes, the level-triggered `Poller` fires, so the next `wait` wakes and the next `pump` reads them. `service_send` that makes an outbound queue non-empty adds write interest, and signals the self-pipe (TM-6: a call that leaves work for `pump` signals before it returns).
3. `ServiceReadable` is level per lane at the event level (SV-3): while frames are unread and none is outstanding, `pump` posts one. Polling the event does not clear the condition.
4. `ServiceWritable` is posted when a lane's free outbound bytes reach the largest refused size since the last wake (SV-3). Write progress on the socket is what frees those bytes.
5. **Lanes are independent:** each lane has its own registration, so a lane without read interest, or a lane whose service does not read, never blocks another lane or a control link.
6. Work that is parked on mandatory-queue room (EV-5b) is not runnable (TM-6). It becomes runnable when `poll_events` frees room, which signals the self-pipe (TM-6).
7. **A control link is read only as far as the engine can consume.** Each link has a bounded receive buffer (one maximal link frame). `pump` decodes a message from it only when the engine can take the step that the message causes. When the message needs mandatory-queue room that is not there (a worker exit whose `SessionState` cannot be posted, EV-5b), the message stays in the buffer (the exit "stays unread on its link"), the link's read interest is removed, and the work is parked. When `poll_events` frees room, Core restores read interest on every link that was parked for room and signals the self-pipe. So a parked link never wakes the host in a loop, and `PumpReport.more` stays false for it (TM-6).
8. **The same rules hold in the testkit:** an in-memory endpoint is ready work only when its flag matches an interest that these rules allow.

**In the testkit** the in-memory link and lane endpoints carry the same two flags (readable, writable) per endpoint, and the `Sim` treats an endpoint with a set flag and a matching interest as ready work. The in-memory lane edge is A6-1's.

### 2.6 Workspace layout

```
botster-core (v1)
├── rust-toolchain.toml            1.97.0
├── Cargo.toml                     [workspace]; [workspace.dependencies] pins every shared dependency
├── .cargo/config.toml             [net] git-fetch-with-cli = true
├── xtask/                         cargo xtask ci | test-budget | taint | public-api | prebuild-worker | slow
├── crates/
│   ├── botster-terminal-ghostty/  P2  the libghostty binding (vendor/ghostty submodule at the pin)
│   ├── botster-core-sys/          P0+ real edges: PTY, process, kqueue/pidfd exit watch, SCM_RIGHTS, flock, fsync, rlimits, descendants
│   ├── botster-core-edges/        P0  edge traits, the Machine trait, the Scheduler trait, the production policy
│   ├── botster-core-link/         P0+ the private control-link wire for workers and guardians: framing, hello, protocol number, token and instance proof, host epoch
│   ├── botster-worker-core/       P3, P4a-c  the Worker machine (worker/, input/, model/, routes/, queries/, files/, webrtc/)
│   ├── botster-guardian-core/     P7  the Guardian machine
│   ├── botster-core-host/         P1, P5, P7  the HostEngine machine
│   ├── botster-core/              P1  the facade: Core::open, impl CoreApi, prelude, contract re-export
│   ├── botster-worker/            P3, P7  the thin binary: roles `session` and `guardian`
│   ├── botster-core-testkit/      P6  Sim, seeded scheduler, scripted edges, TestkitHarness, RealCoreHarness
│   └── botster-mux-example/       P8  section 15
└── tests/conformance.rs           harness = false (libtest-mimic, section 5): the suite on the testkit (default tier) and on real processes (slow tier)
```

**Why separate crates.** Each package owns a crate or a module tree. So parallel pairs rarely edit the same file. The facade crate `botster-core` re-exports only `prelude` and `contract` (section 11, Versioning). `cargo public-api` snapshots it. The Hub's CI check confirms that the Hub imports only the facade.

**What is public.** The facade is the API. `botster-core-host`, `botster-worker-core`, `botster-core-edges` and the others are published with the same version, but they are not part of the API: each crate's README and root doc say "no compatibility promise; use `botster-core`". `botster-core-testkit` is Rust-only, semver-versioned with `botster-core`, and not in the facade or in any FFI crate (A5-1).

## 3. The private control link

The control link between the host and a worker, and between the host and a guardian, is private (P1). It is still versioned, because Core N adopts workers of protocol N and N−1 (AD-4).

- **Framing:** `[u32 LE len][u8 type][payload]`. `len` is checked against a bound before any allocation (the lesson of the old `session_protocol.rs`; the route codec has the same rule, DP-3).
- **Payloads:** JSON for control messages (additive fields keep N−1 compatible); raw bytes for bulk data (tap chunks, TP-1; snapshot pages, ST-6).
- **Hello:** magic, protocol number `P` (one byte), the session `InstanceId`, and proof of the per-worker token (AD-6). The host also sends its **host epoch** (DP-8). The worker obeys only the highest epoch that it has seen.
- **Descriptor handoff:** the route's connected stream goes to the worker by `SCM_RIGHTS` on this link (DP-2). The receiver sets `FD_CLOEXEC` at once (vault: "O_CLOEXEC is process-scoped and does not affect SCM_RIGHTS fd transfer"; `cmsg_len` type differs between Linux and macOS).
- **Tap traffic** is the lowest class and delays any other host-bound message by at most `tap_inflight_bytes` (TP-1).
- **Worker protocol number.** The first v1 worker protocol number is **1** (A6-2). For the out-of-set test of A6-2, the harness makes a real worker announce another number through its hello parameter, which is a construction input of the worker, not a test branch.

## 4. The testkit (`botster-core-testkit`, Core A5-1)

### 4.1 The `Sim`

The testkit is a small discrete-event simulation over the machines of section 2.

- A `Sim` owns one `HostEngine` per open handle, every `Worker` and `Guardian`, the scripted programs, the in-memory links, routes and lanes, the virtual clock and the seeded scheduler.
- `TestkitCore` implements `CoreApi`. `pump(now)` advances the virtual clock to `now`, lets the scheduler run the ready work of every machine (workers progress during the host's `pump`, as real workers progress during real time), and then runs the host engine's own `pump` step.
- A route client of the testkit (`RouteClient::read`) also runs the `Sim` until the route has bytes or no machine has ready work. So a test that reads route bytes without a host `pump` sees a worker that progresses, as in a real process (DP-1: the host is not on the data path; DP-8: bytes keep flowing when the host is gone).
- `drop` of a `TestkitCore` drops only the host engine. The workers stay in the `Sim` (LC-12), and a later `open` on the same `DataDirRef` adopts them (AD-1) through the same link machines.
- **Real libghostty in every worker** (A5-1, R-7). The scripted program supplies the program's output bytes only, never a terminal state.

**Hand-rolling reason** (BUILD.md rule 0). turmoil and madsim simulate a tokio runtime and tokio networking. Core has no async runtime and no tokio sockets: its machines are sans-IO. A simulation over sans-IO machines is a loop of "choose a ready input, hand it to its machine, route its actions". It is about the size of an event-loop driver, and it needs no runtime replacement.

### 4.2 The two harnesses

`botster-core-conformance` takes a `CoreHarness` (foundation design 6.2). The testkit provides both implementations:

| | `TestkitHarness` (default tier) | `RealCoreHarness` (slow tier) |
|---|---|---|
| Core | real `HostEngine` in `TestkitCore` | real `Core::open` with real edges |
| Workers and guardians | in-process `Worker` and `Guardian` in the `Sim` | real `botster-worker` processes, prebuilt (never built by a test) |
| Program | the probe script, interpreted by the scripted program edge | the `botster-conformance-probe` binary from botster-contracts, with the probe script as its argument |
| Controls (6.3) | built from the testkit edges, and the scripted synchronous refusal (4.2a) | built from injected parts of the real `Core` (a failing `Storage` wrapper, a refusing `Process` wrapper, the same scripted synchronous refusal of 4.2a over the real `Core`) and from real OS acts on the test's own process groups (a kill of a verified worker pid). |
| `oracle_encode` and every terminal oracle | `botster-terminal-ghostty` (R-7) | the same |
| `worker(Previous)` | none while `T = 1` (A6-2: no old protocol is fabricated); from `T = 2`, the pinned binary of the last protocol-1 release, run as a real process | the same pinned binary |

### 4.2a Scripted synchronous refusals (A5-3 timing 1)

A5-3 has two failure timings, and the testkit keeps them apart:
1. **A synchronous refusal** is scripted per call. The harness wraps the subject's `CoreApi` in a `RefusalScript` layer (in `botster-core-testkit`; used by both harnesses). A script entry names a call (`begin` with an operation kind, `attach`, `cancel`, `service_send`, and the other sync calls), the occurrence (the n-th such call), and an error code. When the entry matches, the layer **returns the error from the call before the call reaches real Core**. Real Core sees nothing: no `OpId` is minted, no event is posted, no slot is reserved, no state changes, and no ownership is taken (for `attach`, the transport descriptor stays the caller's; for `service_send`, the frame stays the caller's).
   - **Validation.** The layer refuses to load a script entry whose code is not in that call's synchronous column. The column comes from a table in the testkit that has one row per call and cites its source row: the A2-1 operation table (with the `PendingLimit` rule of A2-1), the sync `attach` paragraph of A2-1, ER-0 and the 9.3 Sync column, A2-5 `SendError`, and IN-6 `CancelResult`. P6 checks that table against `botster-core-contract`'s types with a unit test per row. An invalid entry is a harness error, never a run.
   - This layer is not a variation decorator: it never reorders, delays or rewrites a result that real Core produced (A5-2). It only stands in front of a call that real Core never receives.
2. **An asynchronous failure** is never scripted at the `CoreApi`. Only an edge produces it (`Storage`, `Process`, `Link`, `RouteTransport`, `FileSink`), and real Core completes the operation on its own completion path, with its real `OpId`, events, effect and slot release.

**Owners.** P6 builds the `RefusalScript` layer, the sync-column table and the proof of `conf::a5_3_sync_refusal_has_no_op_event_slot_or_state` and `conf::a5_3_only_sync_column_codes_scripted`. P1 provides no hook: the layer uses only the `CoreApi` trait. Each package that owns an async error code wires the edge failure that produces it (`conf::a5_3_async_failure_only_through_edges_and_real_completion`, `conf::a5_3_every_code_reachable_or_listed_real_only`).

### 4.2b Both harnesses prove every id, except the real-OS list

A5-4: "Both run the same suite and both must pass." So:
- **There is no testkit-only exemption.** Every control that a transcript uses is implemented by both harnesses. On the real harness, a control is built from injected parts of the real `Core` (DI at construction of the real edges, never a test branch) or from a real OS act on the test's own processes.
- **The only ids that one harness may not run** are the ids whose proof is a real OS condition that A5-3 names or that the reviewed replacement map classifies `slow` (a real crash, fsync durability, descriptor handoff, `payload_dies_with_guardian`, the real file lock of LC-2, and the other `slow` rows of the map). On the testkit they are listed as real-only (A5-3), with the map row as the reason. **Each one must pass on the real harness or as a named real-process test** that cites its clause.
- **Deferred ids** are the only other exception: exactly the validated entries of `conformance/core-deferred.toml` (section 1 item 5, section 5). They are not run on either harness and are reported as deferred.
- **Every other non-pass is an acceptance failure.** An `unsupported_control`, `inconclusive` or unexpected `not_applicable` on either harness keeps Stage 1 open (section 1). It goes to the owning package as a defect, or to the lead as a QUESTION if the control cannot be built on one harness; the lead takes it to the steward or the Foundation lead. It is never moved to a list.

### 4.2c Seeds and tiers

**Seeds.** The default tier runs every id under the CI seed set (foundation design 6.1: seeds 0 to 31). If the tier passes 60 s, P6 reduces the default set and adds an in-process `sweep` job with the full set. Fake runs never move to `slow`.

**The real-process tier.** The full suite with one seed (a real implementation ignores the seed), plus the named real-process tests. It runs only in the gate (section 8), through `botsterq`.

### 4.3 What Stage 1 needs from Stage 0

| Need | Where it is now | Required by |
|---|---|---|
| The remaining Core transcripts (459 of 591 at final13; the Foundation lands them by package, each with a new `contracts-v0.1.x` tag) (`core-groups` in foundation design 12c) | pending | the package that owns each id (section 6) |
| The real probe follows the probe-script format (ordered steps, `ignore_sigterm`, a direct `signal_self`) | DONE: `contracts-v0.1.3` (probe-fix) | P6 (both harnesses run one script) |
| The probe-script types in a crate that is not a fake | DONE: `botster-probe-script` in `contracts-v0.1.1` (types, parser, validator, `schemas/core/probe-script.schema.json`) | P6 (the scripted program edge depends on it, never on `botster-fake-core`) |
| A released tag of botster-contracts | DONE: `contracts-v0.1.0`, then `contracts-v0.1.1` | P0 |
| The replacement map, reviewed (it assigns each id and fault to its proof: testkit, edge, perturb, slow, static) | `foundation/replacement-map` at `2701038`, last verdict REJECT | P6 (the real-only list), every package's slow tests |
| The `CoreHarness` controls list, closed and documented (6.3) | partly in `driver.rs` | P6 |

A package does not wait for all of Stage 0. It builds against the clauses and the transcripts that exist. An id without a transcript is reported as "pending: no transcript" (section 5); an id whose code is not ready stays in `conformance/core-pending.txt`, which may only shrink.

## 5. How the Core suites run

The pinned runner's `conformance_tests!` macro generates one `#[test]` per transcript, and each test panics unless it passed (`botster-conformance` `run_one_test`). It has no way to mark an id as not yet expected, and it cannot see a ledger id that has no transcript. So the Core repo drives the suite through the runner's **public** functions with its own test harness, and needs no runner change:

```rust
// tests/conformance.rs, `harness = false` (libtest-mimic). One trial per Core ledger id.
// For each transcript in botster_core_conformance::CORE_TRANSCRIPTS (botster_conformance::load_dir):
//   - an id listed in conformance/core-pending.txt becomes an ignored trial, reported as "pending";
//   - an id listed in conformance/core-deferred.toml becomes an ignored trial, reported as "deferred"
//     with its authority and start condition;
//   - every other id becomes a trial that calls botster_conformance::run_transcript with
//     botster_core_conformance::driver_for(harness), the seed set and the selection from the
//     runner's environment variables, and passes only on Outcome::Passed.
// For each id of the pinned ledger that has no transcript: an ignored trial reported as
// "pending: no transcript". The pinned ledger ids are checked in as conformance/core-ledger-ids.txt,
// generated from the botster-contracts ledger at the pinned tag.
// The harness is TestkitHarness (default tier). With the test crate's `slow` feature, a second
// binary runs the same trials on RealCoreHarness.
```

- **libtest-mimic** is a maintained crate for exactly this (custom trials inside `cargo test` and `cargo nextest`, with ignore and filter support). Hand-rolling a test-list format is not needed.
- **The report** prints five counts: passed, failed, pending (split into "pending" and "pending: no transcript"), deferred, and withdrawn; and it lists each not-applicable CASE of an active id separately (the id itself is counted under its run result). A pending, deferred or withdrawn id is never counted as passed. **Stage 1 acceptance needs zero failed and zero pending ids of both kinds; deferred ids are only those that the gate validates.**
- **`conformance/core-pending.txt` may only shrink.** **Initialization, once:** when the base revision has no `conformance/core-pending.txt`, the check validates the new file instead: every id in it must be a Core id of `conformance/core-ledger-ids.txt`, and every ledger id that has no passing proof must be in it (at P0: all 599 ids at final14). **After that,** `cargo xtask ci` fails if the file gains an id that the base revision did not list. **The base revision is `BOTSTER_CI_BASE_REF`** inside the gate (the commit the gate recorded when it started, BUILD.md); outside the gate, `xtask` resolves `origin/v1` ONCE, records the commit in its output, and uses that commit for every check of the run. No check resolves the moving branch reference on its own. A moved contracts pin may add the new ledger's ids to the file in the same commit; that commit lists them. Stage 1 is done only when it is empty (section 1).
- **The contracts' own status files are the source** (from `contracts-v0.1.6`): `conformance/deferred.txt` (deferred ids and not-applicable cases, each with its authority and end condition) and `conformance/withdrawn.txt` (withdrawn ids, each with the id that replaces it). The harness reports a withdrawn id as **withdrawn** (a fifth category, never pending, never counted as a pass) and checks that this repo's `core-deferred.toml` lists exactly the **whole-id** Core deferrals of `deferred.txt` (rules 1 to 4 below still apply). A `not-applicable` line of `deferred.txt` names a CASE of an id that stays ACTIVE (at `contracts-v0.1.6`: the previous-worker case of `conf::dp_12_unreachable_or_n_minus_1_worker_leaves_focused_unknown`, whose unreachable-worker case must pass now); it is never in `core-deferred.toml`, the id runs, and the report lists the case with its reason. **Owner:** P6, in its next scope (P0, which built the harness, is retired).
- **`conformance/core-deferred.toml`** holds one entry per deferred id: `id`, `authority` (the clause, `Core A6-2`, with the manifest tag that accepted it), and `start_condition` (`worker_protocol >= 2`, or `new_worker_feature_over_previous`). The gate (`cargo xtask ci`) validates it on every run:
  1. **Only after acceptance.** While the pinned contracts revision has no manifest entry for A6, the file must be empty. The ids stay in `core-pending.txt`.
  2. **The move.** The commit that moves the contracts pin to a tag whose manifest contains the accepted A6 is the only commit that may move ids from `core-pending.txt` to `core-deferred.toml`. It lists them.
  3. **Exactly A6-2's set.** Every entry must be one of the ids that the accepted A6-2 enumerates, with its start condition. `xtask` holds that enumeration as data citing A6-2 at the pinned manifest tag, and a later accepted text replaces it in the pin-moving commit.
  4. **The start condition is false.** `xtask` reads the worker protocol number `T` and the worker feature set of each protocol from `botster-worker-core` (`WORKER_PROTOCOL` and `WORKER_FEATURES_BY_PROTOCOL`, constants of the crate, P0). `worker_protocol >= 2` invalidates entry 1. A feature in `T` that is not in `T − 1` invalidates entry 2. An invalid entry fails the gate. So the two deferrals end separately, as A6-2 states: entry 1 ends at the first release whose `T` is 2, and its test against the pinned protocol-1 worker binary must then pass; entry 2 ends only at the first release whose protocol adds a worker feature that the pinned `T − 1` worker lacks (a release that adds no worker feature keeps it deferred), and its test then runs with that feature as its subject.
- **Owner and milestone:** P0 builds this harness and the two files at M0, with every id pending. Each package removes its ids from the file in the pull request that makes them pass on both harnesses.
- If Stage 0 later adds pending-list support to `conformance_tests!`, P6 may switch to it; nothing waits for that.

- `cargo xtask prebuild-worker` builds `botster-worker` and `botster-conformance-probe` into `target/candidate/` with a sha256 manifest (the old `script/prebuild-worker` idea; the old `real_worker.rs` manifest check is stolen for `RealCoreHarness`). A test never builds a binary. This also avoids the macOS first-launch stall inside a test (testing rule 7).
- Every real-process test owns its process group and cleans it up on every exit path, panics included (testing rule 10). The xtask's leftover-process check fails the run on a survivor. No test kills by name or pattern.
- Temporary roots are short and canonical (`/private/var/...` on macOS), because a Unix socket path is limited to about 104 bytes.

## 6. Packages

### 6.1 The packages, their clauses and dependencies

Each Core id of the ledger has **exactly one** owning package. `owners.py` assigns E3-1 to P1; A9 to P4a; A10 and A11 to P5; A12 to P4b (with A6-3, the cleanup result it tests); E4 to P4b (EV-8 reply admission; explicit, because its ids name no query); A13 to P3 (the clipboard event and the worker's OSC 5522 acknowledgement); A7-1 to P4a; A8-1 to P1; the A8-2 ids by subject (capture to P1, baseline and resync to P4a, the resume invariant to P2); A6-1 to P7 (except `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone`, which goes to P5 with the other `WorkerUnreachable` work), A6-2 to P5 and A6-3 to P4b. `docs/stage1-clauses/owners.py` assigns them by the rules below (the first rule that matches wins) and writes one list per package to `docs/stage1-clauses/<package>.txt`. The owner makes the id pass on both harnesses. Other packages may write code that the id needs.

| # | Package | Owns (rule) | Ids | Crates | Depends on |
|---|---|---|---|---|---|
| P0 | **skeleton** | TH-1, E1-1 | 2 | workspace, xtask (with the gate of section 8), `botster-core-edges` (including `Entropy`), `botster-core-link` (framing and hello, with its bolero harness), `botster-core-sys` (first module: flock), facade shell, the section 5 conformance harness with `core-pending.txt` and `core-ledger-ids.txt` | none |
| P1 | **session registry and lifecycle** | every id that no other rule takes: LC-1 to LC-10, LC-12 (except the drop id), ID-1, AM-1, AM-3, AM-4, ER-0 and the 9.3 codes, OR-1, OR-2, OR-4, TM-1 to TM-6, EV-2, EV-5, EV-6, EV-9, ST-4, ST-6, TH-2, TI-1, 9B, A2-1, A2-6, A2-7, A8-1, the A8-2 capture id, E3-1 | 124 | `botster-core-host`, `botster-core`, `Storage` real edge | P0; P3 hello and payload milestone (M1 below) for `Start`, `Stop`, `Signal` |
| P2 | **libghostty binding** | A2-8, ST-6b, the A8-2 resume-invariant id | 6 | `botster-terminal-ghostty` | P0 workspace only. It starts with P0. |
| P3 | **session worker, PTY and terminal** | IN-1 to IN-10, 5.1A, SZ-1 to SZ-3, TP-1, AM-2, A2-2, A2-4, ST-1, ST-2, ST-3, ST-5, ST-7, EV-1, EV-3 (as replaced by A13-2), EV-4, EV-7, E2-1 to E2-3, A13-1, A13-1b | 156 | `botster-worker-core` (worker, input, model), `botster-worker` (role `session`), `Program` real edge (PTY) | P0, P2 |
| P4a | **route data plane: stream routes** | OU-1 to OU-12, OU-2b, DP-1 to DP-7, DP-9, DP-12, A2-3, A3-1 to A3-4, TH-3 (ids not taken by P4b or P4c), A7-1, A9-1 to A9-3, the A8-2 baseline and resync ids | 151 | `botster-worker-core/routes`, `RouteTransport` and `SCM_RIGHTS` real edges | P3 (M2: model and admission merge-ready) |
| P4b | **route data plane: queries and files** | EV-8, DP-5b, A2-9, A6-3, A12-1a, A12-1b, E4-1, and route ids that name a query or a file | 68 | `botster-worker-core/queries`, `files`, `FileSink` edge | P4a merge-ready |
| P4c | **route data plane: WebRTC and performance** | DP-10, DP-11, and ids that name WebRTC, the DataChannel, T2 or `connect_deadline` | 15 | `botster-worker-core/webrtc`, UDP edge | P4a merge-ready |
| P5 | **adoption and restart** | AD-1 to AD-7, DP-8, ID-2, LC-11, `lc_12_drop_leaves_workers_running`, and every id that names adoption, restart or survival; A6-2; the A6-1 withheld-control-link id; A10-1, A10-2, A11-1 | 55 | `botster-core-host/adopt`, the link's epoch and token proof | P1, P3; P4a for the route ids; P7 for the service ids |
| P6 | **testkit** | A5-1 to A5-4, OR-3 | 15 | `botster-core-testkit`: the `Sim`, the seeded scheduler and `Entropy`, the scripted edges, the `RefusalScript` layer and its sync-column table (4.2a), both harnesses | P0. It grows with each package. |
| P7 | **services and the guardian** | SV-1 to SV-10, A2-5, A4-1 (except adoption ids); A6-1 (except the withheld-control-link id) | 60 | `botster-guardian-core`, `botster-worker` (role `guardian`), `ServiceLane` and `Process` real edges and the A6-1 in-memory lane edge, `botster-core-host/services` | P1 |
| P8 | **example consumer** | section 15 acceptance script (no ledger id) | — | `botster-mux-example` | P1, P4a, P5 |

Total: 652 active ids at the pinned tag (578 at final12, plus 13 ids of A6, 8 of erratum 2, 11 of A7, 12 of A8, 5 of A9, 3 of A10, 7 of erratum 3, 1 of A11, 2 of A12, 3 of erratum 4 and 11 of A13, minus 2 ids that A9-2 withdrew). `owners.py` skips every id listed in the contracts' `conformance/withdrawn.txt`. When a new ledger lands (a moved contracts pin), the lead reruns `owners.py`, and the diff is part of that commit.

**Split rule.** A package whose review would exceed about 2,500 changed lines is landed as stacked pull requests on its own branch, each one reviewable alone. P3 and P4a are expected to split.

### 6.2 Milestones that let packages stack early

BUILD.md: "Stack on MERGE-READY heads." These milestones are the merge-ready points that other packages wait for:

| Milestone | Package | Content | Unblocks |
|---|---|---|---|
| M0 | P0 | workspace, CI command, edge and machine traits, link framing and hello | everyone |
| M0b | P6 | `Sim`, seeded scheduler, scripted program edge, in-memory link and stream, `TestkitHarness` with `open`, `data_dir`, `drop_handle` | conformance runs for P1 and P3 |
| M1 | P3 | `Worker` hello, payload launch on the `Program` edge (after AD-7 identity), exit with code and signal | P1 `Start`, `Stop`, `Signal` |
| M2 | P3 | the terminal model, the admission point (AM-2), the baseline snapshot | P4a |
| M3 | P4a | the route state machine with baseline, live, output and close on a stream route | P4b, P4c, P5 route ids |
| M4 | P1 | registry, lifecycle, event queue | P5, P7 |

### 6.3 Order and staffing

- **Which package lands first: P0.** Every other package needs its workspace, CI command and traits.
- **Wave 1 (2 pairs):** P0 and P2. P2 needs only the workspace, so it starts at once on its own crate and rebases onto M0.
- **Wave 2:** P6 (to M0b), then P3 and P1. Spawn one pair at a time, by BUILD.md "Resources" at the pinned tag: check both `uptime` and CPU idle (`top -l 1 | grep 'CPU usage'`); spawn when the 1-minute load is under about 12, OR when CPU idle is at least 50% and `botsterq` runs at most one Botster job.
- **Wave 3:** P4a after M2; P7 after M4; P5 after M4 and M1.
- **Wave 4:** P4b and P4c after M3; P8 last.
- **Pairs:** a Sonnet implementer and a Sol reviewer. P3, P4a and P5 get an **Opus** implementer (heavy and subtle: the admission point, the route machine, the epoch fence). Each pair has its own branch from `origin/v1` and its own worktree. Labels: `<package> — implementer`, `<package> — reviewer`.
- **One integration reviewer (Sol)** for the stage. It reviews every pull request that crosses packages or completes the stage.
- **P2 first task: the libghostty capability audit** (R1). It reports the gaps before any other P2 work, so that an orchestrator decision on the Ghostty pin does not stop the stage late.

## 7. Prior art: reuse and reject

Sources: old botster-core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`, read with `git show` only; the vault; the ecosystem. Every steal is its own commit with `Stolen-From: botster-core@72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c:<path>` and `For-Clause: <id>`, and it stays only if it passes that clause's tests unchanged or with trivial edits. Each package repeats the relevant rows in its own Prior art note.

### 7.1 Old botster-core

| Old code (path at the old SHA) | Verdict | Reason |
|---|---|---|
| `crates/botster-terminal-ghostty/src/sys.rs`, `src/lib.rs` (terminal, modes, color profile, `drain_pty_writes`, snapshot export and import), `src/input.rs` (key, mouse, focus and paste encoders over `ghostty_key_encoder_*`, `ghostty_mouse_encoder_*`, `ghostty_focus_encode`, `ghostty_paste_encode`), `build.rs`, `build_support.rs`, `build_data.rs`, `.gitmodules` | **REUSE** (P2) | The binding is libghostty itself (BUILD.md architecture facts). The encoders answer the IN-9 "verify" note: the binding exposes them. Change: the native library becomes unconditional (the old `libghostty-vt` feature is off by default; Core always needs the terminal). |
| `crates/botster-terminal-ghostty/src/client.rs` (`GhosttyClientProjection`) | **REJECT** | A client-side projection. It is not Core's. |
| `crates/botster-core/src/contract/terminal_metadata.rs` (`TerminalMetadataProducer`, a hand-written OSC scanner) | **REJECT** | Terminal parsing outside libghostty breaks BUILD.md architecture rule 1. The pinned libghostty has callbacks for bell, title, pwd, clipboard write, desktop notification (OSC 9 and 777) and modes (`GHOSTTY_TERMINAL_OPT_*` 2, 5, 25, 26, 29, 34). Gaps go to R1. |
| `crates/botster-core-daemon/src/bin/botster-session-worker.rs` (`WorkerLoop`, `WorkerIo`, a hand-rolled `libc::poll` loop) | **REJECT the mechanism, keep the lessons** | It is not sans-IO and cannot run in-process. Lessons kept in the new machine: control before data in each turn; `EgressClass` with one reply slot reserved per admitted input op; the snapshot gate that fences PTY reads and stages a resize; `cancel_operation` and `fail_pending_on_exit`. |
| `crates/botster-core/src/runtime/process_exit.rs` (kqueue `EVFILT_PROC`/`NOTE_EXIT` with `waitid(WNOWAIT)` on macOS; pidfd on Linux) | **REUSE** (P0/P3, into `botster-core-sys`) | A proven child-exit watch with no polling (P8). |
| `crates/botster-core/src/runtime/local_process.rs` (on `portable-pty 0.9.0`) | **REJECT** | `portable-pty` hides the master (vault: "portable_pty MasterPty is private"), and its `setsid` blinds session-scoped censuses. P3 opens the PTY with `rustix` and owns the descriptor. Lesson kept: `killpg` SIGTERM then SIGKILL; closing the master only sends SIGHUP, which agents ignore. |
| `crates/botster-core/src/runtime/session_admission.rs` (Linux and macOS zombie-group probe) | **REUSE the probe** (P5, AD-2 `WorkerGone`) | Vault: leader-pid absence does not prove group absence; `kill(pid, 0)` succeeds on a zombie. |
| `crates/botster-core-daemon/src/registry.rs` (one JSON file per session, `fs::write` then `rename`) | **REJECT** | No fsync, no data-dir lock (LC-2), no start time and no token (AD-6). Kept idea: one file per row. |
| `crates/botster-core/src/contract/session_protocol.rs` (control frames `0x01`–`0x23`, `SPH1` hello, `PROTOCOL_VERSION = 3`) | **REJECT** | Old mechanism; the new link (section 3) has epochs, token proof and descriptor handoff. Kept lesson: check the length before allocation. |
| `crates/botster-terminal-protocol`, `-client` (scheme-2 wire) | **REJECT** | The route codec is `botster-route-codec` from botster-contracts (revision 36), which Core adopts verbatim (DP-3, R-5.3). Same verdict as foundation design 12b. |
| `botster-terminal-protocol/src/keys.rs` (W3C key names) | **REJECT, consult** | The contract uses the 5.1A names. P2 writes the 5.1A-to-Ghostty key table fresh, and may read `keys.rs` for coverage. |
| `crates/botster-core/src/runtime/plugin_process/launch.rs` (`pre_exec` descriptor shuffle, rlimits, `fatal_pipe`) and `supervisor.rs` (deadline owner, group killer) | **REUSE** (P7) | Contract section 10 names them as reusable mechanism for SV-1, SV-4 and SV-9. |
| `plugin_process/host.rs`, `outbound.rs`, `ingress.rs`, `invocations.rs`, `host_port.rs`, `worker.rs`, `capped_allocator.rs` | **REJECT** | Credits, invocations, the `Hello` and in-host confinement are Hub code (section 10, section 14). |
| `crates/botster-core-test-support/src/`: `unique.rs`, `fixture_gate.rs`, `real_worker.rs` (manifest check), `bounded_wait.rs` | **REUSE** (P6, slow tier only) | Foundation design 12b already chose these. Vault gotcha for FIFO gates: use `/bin/cat` and `/bin/echo`, not shell builtins. |
| test-support `fake/*`, `terminal_adapter/*`, `conformance.rs` | **REJECT** | Terminal fakes model a screen (R-5, R-7). Old tests are not ported (BUILD.md rule 4). |
| `script/prebuild-worker` | **REUSE the idea** (P0, `xtask prebuild-worker`) | Tests never build binaries. |

### 7.2 Ecosystem (BUILD.md rule 0: prefer maintained libraries)

The exact versions are fixed by the first package that adds each crate, in `[workspace.dependencies]` and `Cargo.lock`, and recorded in that package's Prior art note.

| Tool | Verdict | Use and reason |
|---|---|---|
| `botster-core-contract`, `botster-route-codec`, `botster-core-conformance`, `botster-conformance-probe` (botster-contracts) | **USE** | The contract in code. |
| `str0m` (sans-IO WebRTC) | **ADOPT** (P4c), after a spike | Worker-terminated WebRTC (DP-2, DP-11) inside a single-threaded sans-IO worker. str0m takes datagrams and time as inputs, so the testkit runs a real DTLS and SCTP stack in-process and deterministically. |
| `webrtc` (webrtc-rs) 0.21 | **REJECT for the worker** | It needs an async runtime and its own tasks inside every worker. That breaks the single-threaded machine and determinism. (The Hub uses it; that is the Hub's choice.) |
| `mio` | **ADOPT** (real worker and guardian drivers) | A maintained readiness loop over raw descriptors. The kqueue or pidfd exit watch registers as a raw descriptor. |
| `polling` | **ADOPT** (the host `Wake` edge) | `Poller` is `Sync` and its descriptor is pollable (TH-2, section 13 rule 5). |
| `rustix` | **ADOPT** (`botster-core-sys`) | PTY open, `flock`, fsync, `killpg`, socket ancillary data, with typed errors and less `unsafe` than raw `libc`. |
| `getrandom` | **ADOPT** (the real `Entropy` edge, 2.3a) | The OS CSPRNG, nothing else, for tokens and secrets. |
| `libtest-mimic` | **ADOPT** (`tests/conformance.rs`, section 5) | One trial per ledger id with pending ids ignored, using the runner's public functions; no runner change. |
| `atomic-write-file` | **ADOPT** (`Storage`) | Atomic replace with fsync of the file and the directory (AD-7 durability). Hand-rolling this needs no reason to exist. |
| `redb` | **REJECT for the registry** | Correct, but it adds a file format and its upgrades for at most `max_sessions` small rows. One file per row is inspectable, and AD-7's "uncertain" maps directly to a failed fsync. |
| `tokio` in Core | **REJECT** | Core is sans-IO. A runtime inside Core would add threads and nondeterminism. |
| `turmoil`, `madsim` | **REJECT** | See 4.1. |
| shuttle and shuttle-tokio, proptest with test-strategy, bolero, cargo-mutants, cargo-nextest | **USE** (already adopted, `docs/tooling.md`) | shuttle for TH-2 and the Hub under A5-4; proptest for the codec-adjacent and limit code; bolero for the control-link decoder (one harness per decoder); cargo-mutants `--in-diff` at landing. |
| loom | **REJECT unless** a hand-written lock-free primitive appears | tooling.md. |

New tools (`str0m`, `mio`, `polling`, `rustix`, `getrandom`, `libtest-mimic`, `atomic-write-file`) are proposed for `docs/tooling.md` of botster-contracts. The lead sends the list to the orchestrator at the first package that adds one.

## 8. Gate, merge and the CI command

- **The CI command:** `cargo xtask ci`. It runs, in order:
  1. `cargo fmt --check`;
  2. clippy with `-D warnings`, including the `disallowed-methods` list of the machine crates;
  3. the taint check: botster-contracts' denylist plus Core's own additions (old crate and mechanism names: `botster-core-daemon`, `botster-terminal-protocol`, `TerminalMetadataProducer`, `SessionRegistry`, `SPH1`, and others that P0 lists);
  4. `cargo public-api` against the checked-in snapshot of the facade;
  5. `xtask prebuild-worker`;
  6. the default tier through `xtask test-budget` (60 s total, 2 s per test, per-test times, leftover processes);
  7. the slow tier (the real-process suite and the named real-process tests);
  8. **mutation tests** (BUILD.md adopted tools: "Mutation tests at landing: cargo-mutants `--in-diff` on the changed crates"): `git diff "$BOTSTER_CI_BASE_REF"...HEAD > target/landing.diff` (the recorded base; never the moving `origin/v1`), then `cargo mutants --in-diff target/landing.diff` over the crates that the diff changes, with the default-tier tests of those crates as the test command, a per-mutant timeout of 5 × the baseline test time, and a total `botsterq --deadline`. The result (`mutants.out/outcomes.json` summary: caught, missed, timeout, unviable) is recorded in the pull request;
  9. **decoder fuzzing** (BUILD.md: bolero, "one harness per decoder; it runs as a property test on stable and as a fuzzer at landing"): every decoder of this repo has one bolero harness (the control-link frame and hello decoder, the service-lane preamble and frame decoder, and any other byte decoder that a package adds; the route codec's decoders are botster-contracts' and are fuzzed there). In the default tier each harness runs as a property test with capped cases and a pinned seed. At landing, each harness of a crate that the diff changes runs as a fuzzer for 60 s with the nightly toolchain, as tooling.md states: `cargo +nightly-<date> bolero test <harness> -T 60s`. P0 pinned `<date>` (`nightly-2026-09-30`), recorded in `xtask`. **The gate never installs or updates a toolchain:** the Linux gate image (`ci/remote/Dockerfile`, owned by the infra engineer; BUILD.md: "The image holds the toolchains of `rust-toolchain.toml`, the pinned nightly") contains that exact nightly before any gate starts, and on the Mac the lead or infra engineer installs it once, outside any gate. Step 9 first verifies that `nightly-<date>` is installed (`rustup run nightly-<date> rustc --version`, no network) and fails with "missing prerequisite: nightly-<date>" if not; it never calls `rustup toolchain install` or `rustup update`. The nightly is used only for this step; everything else, including the property-test runs of the same harnesses in the default tier, uses the stable `1.97.0` of `rust-toolchain.toml`, and a crash input is committed as a regression case.

  **How a result closes.** A missed mutant, a mutation timeout, or a fuzz crash is a review finding on that pull request (BUILD.md: "A surviving mutant is a review finding"). The implementer adds a test that kills the mutant or fixes the crash, or states why the mutant is equivalent (the code change cannot change behavior); the package reviewer accepts that statement or keeps the finding open. The lead merges only when every such finding is closed and the gate is green on the exact head. Steps 8 and 9 run inside the same `botster-gate` run as steps 1 to 7.
- **`conformance/core-pending.txt`** in this repo lists the ids that are not yet expected to pass. The section 5 harness reports them as pending, never as passed. The file may only shrink, and Stage 1 is done when it is empty.
- **THE GATE is `botster-gate`** (BUILD.md "How each stage runs"): `~/botster-sessions/shared/tools/botster-gate <worktree>` runs `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci` on the exact committed HEAD, in a cleared environment with `BOTSTER_CI_BASE_REF` set to the `origin/v1` commit recorded at the start, on the LAN Linux test host (`ci/remote/job.sh`, in `v1` since `ccb04eb`; no network in the gate container, so build-time fetches go to `ci/remote/fetch.sh` through the infra engineer) or on the Mac (`botsterq --exclusive` in a detached gate tree), whichever has room. A green log counts as CI green on either machine. Exit 4 means a SKIPPED step (not a full gate); exit 65 means uncommitted tracked changes. The log path (`~/botster-sessions/gates/<repo>-<branch>-<sha8>-<machine>-<time>-<pid>.log`) goes in the PR and in DONE. The parallelism cap comes from the gate's own command; the xtask still refuses a step when either variable is missing or above 4.
- **One heavy job at a time on the Mac** (BUILD.md "Resources"): every Mac gate, cargo-mutants run and full build runs through `botsterq run --exclusive`; cargo-mutants only `--jobs 1 --in-diff`, inside the gate. Focused single-crate tests may use `botsterq run` without `--exclusive`. On the Mac the agent environment sets `RUSTUP_TOOLCHAIN=1.92.0`, so local cargo commands run with `env -u RUSTUP_TOOLCHAIN` (the gate's cleared environment does this itself).
- **Heavy work** goes through `botsterq` with a `--deadline`. `df -h /` runs before a full run; below 30 GB free, clean first. One `target/` per worktree, cleaned after landing.
- **Zig and libghostty build cost.** Each worktree needs the `vendor/ghostty` submodule (`git submodule update --init`). The Zig caches go under `OUT_DIR` (vault). P2 measures the cold build. If it costs more than about 2 minutes per worktree, P2 adds a shared, content-addressed cache of `libghostty-vt.a` keyed by the Ghostty SHA, the Zig version and the build flags, written by atomic rename.
- **Merge** (BUILD.md, automated): the lead merges when the package reviewer reported CLEAN on the exact head, the gate is green on that head, and, for a pull request that crosses packages or completes the stage, the integration reviewer reported CLEAN. No force-push; no merge of a red or unreviewed head.

## 9. Open questions

| # | Question | To | Blocks |
|---|---|---|---|
| Q1 | **libghostty gaps.** ANSWERED and DONE: the orchestrator approved a pin move in its order (upstream first, then a minimal fork patch series, no upstream contact). The pin moved in revision 18 to `botster/vt-core-stage1-b` at `85a8d8e` on upstream `83edd491` (section 0); the `xterm-ghostty` terminfo entry is unchanged, so `terminfo_source` and the contract tag do not change (A2-8). | — | — |
| Q2 | **AD-4 at the first release.** ANSWERED by the steward (A6-2, accepted): protocol 1; no fabricated N−1 build; exactly two ids deferred with start conditions; the N−1 case of one DP-12 id not applicable. See section 1 item 5 and section 5. | — | — |
| Q3 | **The service-lane edge.** ANSWERED by the steward (A6-1, accepted): yes, with the real-process exceptions listed in the `ServiceLane` row of 2.3. | — | — |
| Q4 | ANSWERED: `botster-probe-script` in `contracts-v0.1.1`. **The probe-script types.** They must move out of `botster-fake-core` before FakeCore is deleted, so the testkit and the probe binary keep one script format. | Foundation lead (through the orchestrator) | P6 scripted program edge |
| Q5 | **Uploads at `Remove`.** ANSWERED by the steward (A6-3, accepted): the teardown always continues; `RemoveReport` is `Deleted` only from a complete trusted result, `delete_failed` with the exact paths, or `outcome_unknown` with no path. See 2.3b. | — | — |

## 10. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | **libghostty does not expose everything the contract needs.** RESOLVED by the Ghostty pin move of revision 18 (section 0); kept for the record. Found at the old pin `eb72ec61`: no callback that reports a terminal **query** with its exact request bytes (EV-8 needs `request_bytes`, the kind, and the held shadow reply; `WRITE_PTY` gives only the reply); no OSC 133 prompt-mark callback with the exit code (A2-4); clipboard **reads** are always ignored (EV-8 `ClipboardRead`); OSC 9 versus OSC 777 source must be checked in `GhosttyTerminalDesktopNotification` (A2-4); the resume invariant at every byte offset (ST-6b) depends on the continuation-bytes option (`GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES`). | P2's first task is a written audit: one row per Core clause that needs terminal semantics, the libghostty API that serves it, or a gap. Gaps go up as Q1 at once. No hand-written parser is a fallback (R-7). |
| R2 | **Stage 0 is not finished.** 459 of 591 Core ids had no transcript at final13; the Foundation lands them by package with new tags; the replacement map has REJECT findings (`Internal`, `SessionEnded`/`WorkerLinkFailed`/`Cancelled` triggers, `SnapshotTooLarge`, limit names); the release tags exist (`contracts-v0.1.0`, `contracts-v0.1.1`), and later transcripts arrive as later `contracts-v0.1.x` tags. | Packages build against clauses; `core-pending.txt` tracks missing transcripts; the pin moves by tag, one commit each. A contract question goes to the steward through the lead; nobody invents an expected value. |
| R3 | **The in-process worker drifts from the real one.** | One machine, two drivers (2.1). The suite runs on both (A5-4). The `disallowed-methods` lint keeps clocks, threads and randomness out of the machines. A behavior that differs is a bug in an edge. |
| R4 | **Default-tier budget.** 652 ids × 32 seeds × a real libghostty per worker, under 60 s. | P6 measures at M0b with the first 100 ids. If needed: a smaller default seed set plus an in-process `sweep` job; a libghostty terminal created lazily; `[profile.dev.package]` opt-level for the hot crates, measured. |
| R5 | **Descriptor handoff on macOS.** No atomic close-on-exec for received descriptors; a fork race seen before. | One module owns the handoff (`botster-core-sys`). `FD_CLOEXEC` is set at once on receipt. The slow tier proves DP-2 with real processes under load. |
| R6 | **WebRTC in every worker** (DP-11 states the costs). str0m is unproven here. | P4c starts with a spike: an in-process str0m pair carrying the TR-3 chunked framing through the testkit, and an audit of every random source of the stack (2.3a). If the spike fails, the lead brings webrtc-rs with a dedicated thread as the alternative to the orchestrator, with its determinism cost. |
| R7 | **The single admission point is a throughput limit** (AM-2). | DP-10's same-run baseline measures it. The perf numbers are "proposal, pending first measurement"; P4c records them, and the orchestrator sets the values (a QUESTION at that time). |
| R8 | **Real-process tier cost on the Mac.** About 580 real-process runs plus about 45 named tests. | Prebuilt binaries; nextest parallelism; one `botsterq --exclusive` gate at a time; orphan checks. Self-hosted Linux runners, when the user approves them, take this tier off the Mac. |
| R9 | **Contract defects found while building.** | No workaround (BUILD.md rule 6). The pair asks the lead; the lead asks the steward. The pair continues with other ids meanwhile. |
| R10 | **Packages P3 and P4a are large.** | Opus implementers, stacked pull requests (6.1 split rule), merge-ready milestones (6.2). |
| R11 | **Old mechanism leaks in.** | Taint check with Core's additions; steals only with trailers; the reviewer judges against the contract, never against the old behavior. |
| R12 | **Process cleanup.** Orphan workers cost machine-wide PTYs and CPU (vault). | Every real-process test owns its group; teardown is TERM, grace, KILL, reap; zombies count as leftovers (attributed by PPID on Darwin); no kill by name or pattern. |

## 11. What the lead does next

1. Get this plan CLEAN from the Sol plan reviewer; pin each revision to `~/botster-sessions/pins/stage1-plan.<sha8>.md` (read-only, hash-verified).
2. Report DONE (plan CLEAN) to the orchestrator with the pin path, and send Q1 and Q4 up at the same time (Q2, Q3 and Q5 are answered by the accepted Core A6).
3. When the orchestrator starts Stage 1: staff P0 and P2 (wave 1), checking `uptime` before each spawn, and continue by section 6.3.

## 12. Revisions

| Revision | Change |
|---|---|
| 1 (`47630cb`) | First plan. |
| 2 | Round 1 review (`stage1/plan-review` `9ba86b2`): F1 the `Entropy` edge and banned hidden randomness (2.3, 2.3a); F2 scripted synchronous refusals (4.2a); F3 host readiness for service listeners and lanes (2.5); F4 no testkit-only exemption (4.2b); F5 mutation and fuzz steps in the gate (section 8); F6 the consumer-side pending harness (section 5); F7 Q1 keeps A2-8's condition; F8 nonblocking file I/O (2.3b). |
| 3 | Round 2 review (`ba9d60b`): F3 read interest follows epoch commit and engine consumption (2.5 rules 7 and 8); F5 the nightly toolchain of the landing fuzzer (section 8); F6 one-time initialization of the pending file and zero pending of both kinds at acceptance (section 5); F8 teardown waits for `Deleted` completions, and Q5. |
| 4 | Round 3 review (`ae09900`) and the steward's answer (Core A6 candidate 1, `37f083b`): F8 cleanup at `Remove` follows A6-3 (the worker deletes on completions; a failure or an unreachable worker is reported in `RemoveReport`, never a silent success; 2.3b). Q2, Q3 and Q5 answered by A6-1 to A6-3; pins and done criterion updated (section 0, section 1 item 5, 2.3, 3, 4.2, 6.1). |
| 5 | Round 4 review (`86ea2af`) and Core A6 candidate 2 (`0aa03e5`): F9 the deferred category: `core-deferred.toml`, a fourth harness category, gate validation (only after A6's acceptance and pin move, exactly A6-2's set, start condition false for the running protocol and features), and agreeing acceptance rules in sections 1, 4.2b and 5; 2.3b cleanup per A6-3 candidate 2 (`outcome_unknown`, exact `delete_failed` paths, trusted complete result only). |
| 6 | Round 5 review (`3de1dbf`): F9 the two deferrals end on their own A6-2 conditions (section 5 rule 4); F10 revision 4's history names candidate 1. |
| 7 | Round 6 CLEAN (`806de7e`) on revision 6. Then Core A6 was accepted as candidate 3 (manifest final13): pins moved to final13 (section 0); the process edge gains the withheld control link (2.3, section 1 item 5); the clause lists regenerated (591 ids; 13 A6 ids assigned); the pre-acceptance wording removed. |
| 8 | Round 7 review (`465e7bc`): F11 BUILD.md did change between `9666bf5` and `2f2996e` (the interim parallelism cap); section 0 corrected; section 8 applies the cap to every gate step, inherited by nested builds, mutation and fuzz runs. |
| 9 | Round 8 review (`043a7f7`): F11 the cap is set in the `botsterq` launch command before `cargo xtask ci` compiles the xtask; the xtask keeps child enforcement (section 8). |
| 10 | Contracts pin moved to `contracts-v0.1.1` (final14, `botster-probe-script`): section 0 pins and the doc changes between pins; erratum 2 ids to P3 (599 ids; owners.py rule `E2-*`); the worker rule of E2-3 (2.4); Q4 and the Stage 0 tag needs done (4.3, section 9); the new BUILD.md spawn rule (6.3). The Ghostty pin move follows in a later revision when P2's patch series is CLEAN. |
| 11 | Round 10 review (`168750c`): F12 R2 no longer claims that no release tag exists. |
| 12 | Contracts pin moved to `contracts-v0.1.3` (manifest final16: Core A7 and A8; the probe fix): section 0 pins and the doc changes between `contracts-v0.1.1` and `contracts-v0.1.3`; owners.py rules for A7-1 and A8 (622 ids; P1 117, P2 6, P4a 148); 4.3 marks the probe fix done; section 8 records the one-heavy-job rule and the `RUSTUP_TOOLCHAIN` override. |
| 13 | Contracts pin moved to `contracts-v0.1.6` (manifest final19: Core A9, A10, erratum 3): section 0 pins and the doc changes between `contracts-v0.1.3` and `contracts-v0.1.6`; owners.py rules E3-1 to P1, A9 to P4a, A10 to P5, and withdrawn ids skipped (635 active ids: P1 124, P4a 151, P5 54); section 5 makes the contracts' `deferred.txt` and `withdrawn.txt` the source, with a withdrawn category (P6 next scope); section 8 replaces the interim botsterq gate with `botster-gate`. |
| 14 | Round 13 review (`93b231f`): F13 `core-deferred.toml` matches only whole-id deferrals; a not-applicable case of an active id is reported separately; the report has five counts; F14 the pending-list check and the mutation diff use `BOTSTER_CI_BASE_REF` (outside the gate, one recorded resolution of `origin/v1`); F15 the gate never installs a toolchain: the pinned nightly is provisioned in the Linux image and on the Mac beforehand, and step 9 verifies it offline. |
| 15 | Contracts pin moved to `contracts-v0.1.7` (manifest final21: Core A11, A12; the P3, P6 and P5 transcripts): section 0 pins; R-18 the only doc change; owners.py rules A11 to P5, A12 to P4b (638 active ids: P4b 65, P5 55). |
| 16 | Contracts pin moved to `contracts-v0.1.8` (manifest final22: Core erratum 4; the e3_1 and P4b transcripts): section 0 pins (no doc changes); owners.py rule E4 to P4b (641 active ids: P4b 68). |
| 17 | Round 16 review (`6b9b689`): F16 the total in 6.1 reads 641. |
| 18 | The Ghostty pin moves to the fork branch `botster/vt-core-stage1-b` at `85a8d8e` on upstream `83edd491` (P2's patch series reviewed CLEAN; Q1 resolved in the orchestrator's order); Zig unchanged; the `xterm-ghostty` terminfo entry is unchanged (src/terminfo untouched; hashes equal at the old pin and upstream), so `terminfo_source` and the contract tag do not change (A2-8); contracts pin recorded as `contracts-v0.1.9` (R-19 crate fix; rulings R-19, R-20). |
| 19 | Round 18 review (`90cb503`): F17 the E2-3 worker rule (2.4) is defined per model step (one completed `vt_write` or `vt_write_until_query` consuming a prefix; suffix retained in order; query capacity honored; modes compared per step; input only between completed steps); F18 the clause lists regenerated against `contracts-v0.1.9` (P4c transcripts present). |
| 20 | Contracts pin moved to `contracts-v0.1.13` (manifest final30: Core Amendment 13; the driver's await-recheck fix; the second P4a batch): section 0 pins and doc changes (R-28 the only Core ruling since v0.1.9); owners.py rule A13 to P3 (652 active ids: P3 156). |
