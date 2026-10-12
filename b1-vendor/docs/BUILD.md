# Building Botster v1 from the contracts

The user signed off on the frozen design (manifest final6) on 2026-09-30. This file is the build plan that every lead and every implementer/reviewer pair works from.

## Roles

| Role | Agent | Owns |
|---|---|---|
| Orchestrator | Claude (Opus) | Cross-side decisions; brings product decisions to the user; staffing. |
| Contract steward | The plan auditor | What the contracts mean; the amendment path; the joint manifest in this repo. |
| Lead (one per side) | Claude (Opus) | The side's internal design doc; the feature queue; decisions about which old code to steal; staffing the pairs. |
| Implementer | Sonnet | One feature (a group of clauses), on its own branch and worktree. |
| Reviewer | Sol | Reviews each feature against the contract clauses and this file's rules. One Sol ACCEPT lands it. |

Leads by side:
- **Phase 0:** a Foundation lead, for this repo.
- **Phase 1:** Core, Hub, and Clients (the TUI, Web, the CLI and a shared client core).
- **Phase 2:** Plugins.

**Messaging rule** (the user, 2026-09-30): "questions and terminal events (completed, question, done) get bubbled up to the next layer and continue up as necessary."
- **Layers:** pair (implementer and reviewer) → lead → orchestrator → user. The steward is a side channel for **spec meaning only**, not a layer.
- **Only three event kinds ever go up:**
  - **QUESTION:** a decision this layer cannot make;
  - **BLOCKED:** stuck, with what was tried;
  - **DONE:** a feature landed, or a phase is complete.
- **Nothing else goes up:** no progress reports, no candidate-ACK notices, no FYIs.
- **What reaches the ORCHESTRATOR** (the user, 2026-10-01: "don't let agents eat up your context pointlessly"):
  - a QUESTION only the orchestrator or the user can settle;
  - a BLOCKED that the lead cannot clear itself;
  - a STAGE milestone (a stage complete, or a named plan milestone such as M0).
  Routine per-PR merges, amendments that need no decision, rulings and handoffs go ONLY to the state log (`~/botster-sessions/botster-v1-orchestrator-state.md`) and to whoever is directly affected. The steward reports to the orchestrator only an amendment that changes scope or needs a decision.
- **The same rule applies at EVERY layer** (the user, 2026-10-01). Every agent that commands others protects its own context the same way: the steward with the Amendment reviewer, each stage lead with its pairs, any coordinator or infra agent with its helpers, and every future one. By default, its reports send up only QUESTION, BLOCKED and terminal events (DONE, CLEAN, or a verdict).
  - **The commander may allow MORE** when that genuinely helps its own work (for example, a lead asking a pair for a mid-feature checkpoint on a risky change). It says so explicitly in that report's brief, and only for that need.
  - **Every spawn brief written by a commander includes this rule,** so it passes down the chain.
- **Resolve at the lowest layer that can.** Each layer forwards up ONLY what it cannot settle. The forwarded message carries its context, so the next layer never has to ask back.
- **DONE goes up one layer at a time.** A lead rolls its feature DONEs into one milestone DONE for the orchestrator. The orchestrator reports phase milestones to the user.
- **Answers come back down the same path.**
- **Never wait silently on a layer above.** Ask, and continue other work meanwhile.

## Phases

**Testing architecture: real components with injected edges** (orchestrator decision, 2026-10-01; the Foundation lead's, the steward's and the amendment reviewer's assessments converged: `~/botster-sessions/shared/fake-mode-steward-assessment.md`).
- Conformance runs against the REAL implementations, in-process, through injected edges. There are no hand-written stand-in implementations.
- **Scripted players** exist only where an independent peer is needed, and they play the hand-authored clause transcripts, never re-implementing a component: the client-subject runner (the runner plays the Hub), the TypeScript player for Web (the Rust player compiled to wasm), and the HP plugin-side FakeHub and FakePlugin as frame-script players (plugin authors need no Hub or Core).
- **Record and replay is never an oracle.** Expected values come from clause-cited transcripts.
- **Fault injection goes through edges** (storage, process/program backend, transport, plugin loader), plus a clause-written, permute-only **perturbation layer** for the orders the contracts leave open, plus **shuttle** for internal schedules.
- The **slow tier is unchanged**: fsync, real crashes, fd handoff, adoption after a real host crash, the PTY, the packaged worker and the C ABI are proven only by named real-process tests.
- The frozen fakes of Phase 0 (FakeCore, the fake route endpoint) stay unchanged, with no new fidelity, until their real counterparts pass the same suites. Then they are deleted. `docs/fake-replacement-map.md` maps every conformance id and fault to its replacement proof; nothing fake is deleted before that map is reviewed and the replacement is green.
- The contract text that requires fakes (Core §11, HC §12, HP §19) is amended through the steward.

0. **Foundation (this repo).**
   - The contract crates, with the generated JSON Schemas (Hub→plugin Appendix B, and the client kind schemas) and the generated TypeScript.
   - Every clause's conformance transcript, validated structurally and by the scripted players.
   - The runners, with **real-subject harness traits** (the Core harness, the Hub subject, the plugin hub and host subjects, the client subject), so a Phase 1 repo runs the suites against its real implementation in-process.
   - The scripted players: the client-subject runner (the runner plays the Hub), the HP frame players, and the **TypeScript player** for Web.
   - The leaf **scripted program backend** (probe scripts) that the Core testkit uses in place of a real PTY program.
   - `docs/fake-replacement-map.md`.
   - The README that tells a Phase 1 lead how to depend on the crates and run the suites against a real implementation.

   **Done when:** every clause's test id has its transcript; every transcript passes its structural check and, where a scripted player or a frozen fake exists, passes against it under the CI seed set; the replacement map is reviewed; the taint check and the test budgets run in CI; and the README is complete.
**Pipeline (the user, 2026-10-01): serial stages, parallel inside each stage.** Each layer tests against the REAL layer below it, so the layers are built in dependency order, and each stage is wide internally.

1. **Stage 1, Core** (botster-core, `v1`). Packages run in parallel:
   - the session registry and lifecycle;
   - the session worker, with the PTY and libghostty;
   - the route data plane (codec, backpressure, resync, file input);
   - services and the guardian;
   - adoption and restart;
   - the testkit.

   **Done when:** Core's conformance passes on real Core in-process AND with real worker processes (slow tier).
2. **Stage 2, Hub** (botster-hub, `v1`). Packages:
   - the client protocol (kinds, watches, describe);
   - pairing, auth and WebRTC signalling;
   - the plugin host and the Lua host;
   - the plugin I/O service (net, ws, secrets, MCP proxy);
   - the plugin manager and lockfile;
   - events, notifications and push;
   - the MCP server.

   **Done when:** Hub's conformance passes on the real Hub plus Core in-process.
3. **Stage 3, clients and plugins.** The TUI, Web and CLI, plus each plugin port, run in parallel against the real Hub. The plugin ports use `frozen/final6/audits/plugin-parity-20260930.md` and the client parity audit as the zero-regression checklists.
4. **Stage 4, cutover:** journeys J1–J7 pass on the real implementations.

**Keeping the critical path short:**
- **Plan one stage ahead.** While stage N builds, stage N+1's planner writes its plan and has it reviewed. Design never waits; only code waits.
- **Stack on MERGE-READY heads.** A package that needs only part of the stage below starts when that part's PR is merge-ready. It does not wait for the whole stage.
- **Early exceptions.** Work that depends only on the contract crates may start early: UI rendering, the TS player, and generated types.

## How each stage runs (adapted from the HyperFlex playbook, `~/botster-sessions/orchestration-playbook-from-hyperflex-2026-10-01.md`)

- **The stage lead (Opus, agent "claude")** plans and coordinates. It writes the stage plan, splits it into packages, staffs pairs, and never writes product code. It stays running to answer questions.
- **Plans are pinned.** Every plan revision is copied to `~/botster-sessions/pins/<stem>.<sha8>.md`, made read-only with `chmod 444`, and hash-verified. Implementers build from a named pin.
- **Plan review overlaps building.** The reviewer reviews the plan while the implementer starts, and the implementer adapts to plan revisions.
- **Pairs:**
  - one per package: a Sonnet implementer (Opus for heavy or subtle work) and a Sol reviewer, each pair in its own worktree and branch;
  - pairs talk to each other directly, with full session UUIDs;
  - one integration reviewer per stage.
- **Reviews check logic only.** Reviewers run tests only to prove a finding. Which findings must close before CLEAN depends on the PR's risk tier (below).
- **Risk tiers (the user, 2026-10-09: rigor scales with risk).** Every PR has one tier: HIGH or STANDARD. The PR description states the tier and the rule that decides it.
  - **A PR is HIGH when ANY of these is true.** The lead and the reviewer may raise a tier. Nobody may lower a tier that one of these rules sets.
    1. It changes gate-decision code: CI steps, the mutation profile, base-merge-check, budget or leak checks, the taint check, or any code whose result decides pass or fail. A change to a HIGH-path list is gate-decision code.
    2. It adds or changes `unsafe` code, FFI, or the C ABI.
    3. It crosses packages: it changes code owned by more than one package, a shared crate (for example the testkit or `botster-test-process`), or a workspace-wide config (`Cargo.toml` lints, `clippy.toml`, `.config/`, `.cargo/`).
    4. It moves a pin: the contracts tag, the Ghostty fork, or the toolchain.
    5. It changes a path on the repo's HIGH-path list. Each repo keeps one checked-in list (botster-core: `ci/high-tier-paths.txt`). It holds process control (spawn, signal, wait, reap), the PTY, fd handoff, adoption and restart, durable storage, and auth, token and proof checks. A repo that has no list yet treats every PR that touches any of these areas as HIGH.
  - **The reviewer checks the stated tier first.** A wrong STANDARD tier is a HIGH finding, and the PR moves to HIGH.
  - **Every other PR is STANDARD.** For example: sans-IO logic inside one package, codecs, routing and data shaping, docs, and test-only changes inside one package.
  - **HIGH keeps the full rules:**
    - two reviews: the package reviewer (Sol, cross-model) AND the integration reviewer;
    - every finding closes before CLEAN, LOW included;
    - every surviving mutant is killed by a behavior-level test, or is listed with its reason;
    - real-process behavior is proven by a named real-process test before the code merges.
  - **STANDARD is lighter:**
    - one review: the package reviewer (Sol, cross-model). There is no integration review.
    - HIGH and MEDIUM findings close before CLEAN. The reviewer files LOW findings as ONE follow-up issue that the PR links; they do not block CLEAN.
    - Mutation runs `--in-diff` as on HIGH. A survivor in a decision (a branch or comparison that chooses an outcome) is killed. Any other survivor may instead be listed in the PR with a one-line reason.
  - **Both tiers keep:** the gate on the exact head, the merge rule, the contract-citation rules, and the testing rules below.
- **Round limit.** After three NOT CLEAN rounds on one PR, the reviewer tells the lead. The lead decides one of these before round four: split the PR, settle the disputed point, or replan the root cause. A fourth round never starts by default.
- **The gate is ONE CI run on the exact head after CLEAN.** No local full suites; implementers run focused tests only. Any commit after CLEAN gets a re-review of the delta and CI on the new head.
  - **Pre-review gate (orchestrator, 2026-10-09, plan r22 Q8):** an implementer may run ONE full `botster-gate` on a head before its first READY to reviewers. If the head that reviewers report CLEAN is the same head that gate passed on, that gate counts as the merge gate; otherwise the head after CLEAN gets its own gate as above.
  - **Disjoint base merge (orchestrator, 2026-10-09, plan r22 Q7):** a commit after CLEAN that only merges the base branch (v1 or main) needs no reviewer delta round when a script proves all of these: the merge has no conflicts (`git merge-tree`); no path changed on the base since the reviewed head's merge base is also in the PR's own diff; and the PR's own diff against the new merge base is byte-identical to the reviewed one. The script's output goes in the PR. The gate still runs on the exact new head, and the integration reviewer's CLEAN is still required where rule 3 below applies. Any other commit after CLEAN gets the delta review.
  - **Gates run on the shared Nomad pool (2026-10-05):** `botster-gate` keeps its command line, with `jobq` (Tonksthebear/compute-pool) as backend. Botster is tier 3 (HyperFlex 1, Dexter 2); see that repo's DESIGN.md. The testq and botsterq details below describe the previous backend (`botster-gate.testq`).
  - **CI capacity (2026-10-01):** GitHub-hosted Actions are blocked by billing ("spending limit / payments failed"), so jobs fail instantly with zero steps.
  - **THE GATE is `botster-gate` (2026-10-01).** Run it as `~/botster-sessions/shared/tools/botster-gate <worktree>`. It runs ONE gate on the exact head of the worktree, either on the LAN Linux test host (`structvr@192.168.1.185`, through its shared queue `testq`) or on this Mac (through `botsterq run --exclusive`), whichever has the resources. A green `botster-gate` log counts as "CI green" for the merge rule, whichever machine ran it. Record the log path in the PR or handoff.
    - **The same gate on either machine:**
      - the same command: `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci`;
      - the same commit, never the live worktree: untracked and ignored files, and a checkout made while the gate waits, cannot change the result;
      - the same environment: a cleared one with `TZ=UTC` and `BOTSTER_CI_BASE_REF` set to the base commit recorded when the gate starts;
      - a gate with a SKIPPED step is not a full gate: `botster-gate` exits 4;
      - the log's header names the repo, the branch, the head, the base, the machine, the reason for the choice and the command, and its last line gives the exit status and the time.
    - **The machine (auto):**
      1. Linux, when the host answers, testq is not paused, no job waits in its queue, the job's CPUs (default 4) are free, and a Botster slot is free there (3, or 4 while testq has no HyperFlex job).
      2. Otherwise the Mac, when its 1-minute load is under 12 or its CPUs are at least 50% idle, and botsterq runs no job.
      3. Otherwise the shorter line: Linux when its waiting jobs are no more than botsterq's jobs, else the Mac.
      - The Mac is the only choice when the host does not answer, or when the head has no `ci/remote/job.sh`. `--on linux` or `--on mac` forces the machine.
    - **Capacity share on the host:** testq has 20 CPUs for all projects. A Botster gate takes 2 to 4 (the gate caps cargo and nextest at 4), and at most 3 Botster gates run there at once (12 CPUs), so HyperFlex keeps at least 8, enough for its system suite. **Elastic 4th slot (orchestrator, 2026-10-02):** a 4th Botster gate (16 of 20 CPUs) may start only while testq has no HyperFlex job queued or running; new gates stop taking it as soon as HyperFlex has a job, and a gate already on it runs to its end (testq never interrupts a running job). HyperFlex CI jobs (`--priority`) go first. The limit is enforced on the host: before it submits, the client's holder process takes a slot lock (`~/testq/projects/botster-locks`), and a gate that finds no free slot waits without taking testq CPUs.
    - **How it runs on Linux:**
      - A holder process on the host owns the gate. It takes the branch lock (one gate of a branch at a time, because they share its target volume), then a Botster slot, then the job's lease, then the worktree's source lock while the client pushes. It runs `testq submit` itself, so a submission never lacks an owner.
      - The client pushes the head and `origin/<base>` (main for botster-contracts, v1 for botster-core) into a git checkout on the host, one per worktree, while the holder holds the source lock. Then it asks the holder to submit the job, and calls `testq follow`.
      - Only committed content travels. `target/`, `.env`, `mise.local.toml` and every untracked file stay on the Mac.
      - The repo's `ci/remote/job.sh` runs the command in the repo's `ci/remote/Dockerfile` image, limited to the job's CPUs. The image holds the toolchains of `rust-toolchain.toml`, the pinned nightly, cargo-nextest, cargo-mutants, cargo-bolero and node (botster-contracts), or cargo-public-api and Zig 0.16.0 (botster-core).
      - The cargo registry is in a volume per repo, and the target directory is in a volume per repo and branch, so a branch's gates build incrementally. A detached head (the gate of a merge commit) uses its base branch's volume. A target volume that no gate used for 7 days is removed.
      - botster-core's private git dependencies are fetched in a separate container with the host's GitHub token (a read-only 0600 file). The gate container then runs with no network and no token.
      - Ctrl-C, or any exit of the client before the job ends, cancels the job. The job always removes its containers and its snapshot.
      - The client sends the holder a heartbeat every 10 s. When the client ends (normally, Ctrl-C, a kill, or no heartbeat for 180 s; a loaded Mac can stall the heartbeat for tens of seconds), the holder cancels the job if it is still queued or running, waits until it has stopped and its containers are gone, and only then frees its locks. A job that does not stop keeps the locks, and the holder reports it every minute. If the holder itself is gone, the job stops itself (`job.sh --lease`).
    - **Several gates can run at once** on the host, within the share above. `botster-gate --queue` shows both queues.
    - **macOS-only tests** (none on 2026-10-01 in botster-contracts or botster-core): code under `#[cfg(target_os = "macos")]` does not compile on Linux. `botster-gate` prints a warning that lists such files when it picks Linux. The Mac slow tier keeps them covered: a PR that adds macOS-only tests also runs `botster-gate --on mac` on its head.
    - **How it runs on the Mac:** `botsterq run --exclusive` in a detached gate tree of the worktree (`~/botster-sessions/gate-trees/<repo>-<worktree>`, with its own `target/`). The job checks the tree out at the head when it starts, and removes every untracked and ignored file except `target/`.
    - **botsterq on the Mac is the fallback**, for the cases above and for macOS-only tests. Never run `cargo xtask ci` through botsterq directly for a gate; use `botster-gate --on mac`, so the log has the same header.
    - **Target:** self-hosted GitHub Actions runners on the same host, with the label `botster-ci`, whose jobs go through `testq submit --priority`.
- **MERGE IS AUTOMATED (the user, 2026-10-01): "merge directly without me."** The stage lead merges a PR itself, with no human step, when ALL of these hold:
  1. the package reviewer reported CLEAN on the exact head;
  2. GitHub CI is green on that exact head;
  3. for a HIGH-tier PR (which includes every PR that crosses packages), and for a PR that completes a stage, the integration reviewer reported CLEAN.

  The lead never merges a red or unreviewed head and never force-pushes. Contract amendments still land through the steward and the Amendment reviewer.
- **botster-contracts: a rulings-only main moves no gate (the steward, approved by the orchestrator, 2026-10-11).** In botster-contracts, a PR's gate and verdicts are on the exact head that `main` fast-forwards to. There is one exception, and it covers exactly one file: `docs/steward-rulings.md`.
  - **When it applies:**
    1. every commit on `main` since the PR's gated base changes only `docs/steward-rulings.md`;
    2. the PR itself does not touch that file.
  - **What the steward does then:**
    1. merge the gated head into `main` with a merge commit (no new gate, no re-review);
    2. check that the merge was clean, with no conflict;
    3. check that `git diff <gated head> <merge commit> --name-only` is exactly `docs/steward-rulings.md`;
    4. record that check, and its output, in the merge commit's message.
  - **Why it is safe:** no gate step reads `docs/steward-rulings.md`. The only xtask commands that walk every tracked file (`taint` and `timers`) skip `docs/` (`xtask/src/taint.rs` `SKIPPED_PREFIXES`; `xtask/src/timers.rs`). Every other `docs/` file can be gate input (for example `core-testkit-controls.md`, `fake-replacement-map.md` and `taint-denylist.txt`), so none of them is covered.
  - **Limits:**
    - Adding any other file to this exception needs the orchestrator's approval.
    - The exception ends at once if any xtask command, CI job or check starts to read `docs/steward-rulings.md`. A PR that makes such a change also removes this rule.
- **Resources:**
  - ramp up gradually, starting with 1–2 pairs;
  - before each spawn, check BOTH `uptime` and CPU idle (`top -l 1 | grep 'CPU usage'`). Spawn one pair at a time when the 1-minute load is under about 12, OR when CPU idle is at least 50% and botsterq is running at most one Botster job. The load average on this Mac is often inflated by macOS indexing (mediaanalysisd, spotlightknowledged, mds) while the CPUs sit idle (orchestrator ruling, 2026-10-01);
  - gates run through `botster-gate`, which prefers the Linux test host; heavy work stays off the Mac where it can;
  - **on the Mac: ONE heavy job at a time.** Every Mac gate (`botster-gate --on mac`), mutation run (cargo-mutants) and full build goes through `botsterq run --exclusive`. cargo-mutants runs with `--jobs 1`, on `--in-diff` only. On 2026-10-01, two concurrent cargo-mutants jobs, plus macOS scanning each new binary (XProtect, mds), left the CPU 5% idle.
  - **on both machines:** every gate and local build caps its parallelism, with `CARGO_BUILD_JOBS=4` and nextest `--test-threads 4` (or `NEXTEST_TEST_THREADS=4`). An uncapped gate drove the load to 46 on 12 cores on 2026-10-01.
- **Spawning:**
  - `create_agent` always takes `target_id`, or nothing spawns; verify every spawn with `list_hubs`;
  - cut branches from origin (`git branch <b> origin/v1`) before spawning;
  - map paired agents by label;
  - copy UUIDs and SHAs from tool output, never from memory;
  - `post_message` takes `{session_uuid, payload}`, with `payload` as a string.
- **A running state log:** `~/botster-sessions/botster-v1-orchestrator-state.md`. Every spawn, merge, decision and cleanup is appended to it.

**HARD Phase 1 requirement: sans-IO edges and a deterministic scheduler (Core and Hub).**
- Every source of nondeterminism or I/O is an injected edge: the clock (Core already takes `now` from the host, TM-1, TM-3), storage, the process/PTY or program backend, the route and control transports, and the plugin loader.
- The event loop takes an **injected scheduling policy**, so a test runs it deterministically under a seed, and the perturbation layer can explore every order the contracts leave open. Shuttle covers internal concurrency.
- The edges are dependency injection, never test branches in production code (Testing rule 8). The Core test edges live in a Rust-only `botster-core-testkit` crate, off the public facade and off the C ABI (Core §13 rule 1: no host callbacks).
- **The session worker runs in-process without forking its logic:** the worker is a library type with a program-backend edge and a route-transport edge. The `botster-worker` binary is a thin main that wires the real PTY and sockets; the testkit runs the same type on a thread with the scripted program backend. **The proof is explicit:** the same Core conformance suite runs against the real Core with real worker processes AND against the testkit, and both must pass (Core A5-4, candidate). A worker behavior that differs between the two is a bug in an edge, never a reason to branch the worker logic. (The binary name `botster-worker` is the install layout's, HC A2 RT-3.)
- The real Core with real libghostty is the terminal oracle (R-7). Terminal assertions run against it, never against a model.

## Starting fresh: no taint from old code

0. **LEARN FIRST, THEN STEAL DELIBERATELY** (the user, 2026-09-30). The taint rules stop old MECHANISM from leaking in. They never mean ignoring what is already known.
   - **Prior-art pass, before designing any feature.** The lead or the implementer:
     - (a) searches the knowledge vault (`~/knowledge/notes/`; start from `botster-architecture.md`) for the area's decisions, conventions and gotchas;
     - (b) reads the existing code for the same job at pinned SHAs, read-only, with `git show <sha>:<path>` or `git grep`. Examples: botster-core's `crates/botster-terminal-protocol` (an earlier route wire), `botster-terminal-ghostty` (the libghostty binding), `botster-core-test-support`, and the Hub's test harness and fixtures;
     - (c) reads the parity audits (`frozen/final6/audits/*parity*`) and the Core rebuild plan (`~/botster-sessions/shared/core-rebuild-plan-v1.md`).
   - **A "Prior art" note** goes in the feature's design note or PR description. It names what exists, what was REUSED (by the steal procedure below: `Stolen-From` and `For-Clause` trailers, kept only if it passes the clause tests), and what was REJECTED and why.
   - **The reviewer rejects a feature that has no prior-art note.**
   - **Prefer established, maintained libraries and tools over hand-rolled infrastructure; hand-rolling needs a recorded reason in the Prior art note** (the user, 2026-09-30). Prior art includes the ecosystem, not only our old code. `docs/tooling.md` in botster-contracts records the adopted tools.
   - **Taint protection is unchanged:** no checkout of old trees into v1, no ported old tests, the denylist stays, and the contract decides the behavior. Knowledge and proven code are welcome; old mechanism is not.
1. **An empty `v1` branch per repo.** It is created with `git checkout --orphan v1`, has no history and contains no old files. Feature branches start from `v1`. At cutover, `v1` becomes `main`. The old `main` stays as history.
   - This rule applies to repos that contain old code. botster-contracts was created clean and builds on `main` (orchestrator, 2026-09-30).
2. **Agents start from the contracts,** this file and their side's design doc, never from the old tree.
3. **Stealing is explicit.**
   - Read old code at a pinned commit only: `git show <sha>:<path>`. Never check it out into a v1 worktree.
   - Bring it in as its own commit, with the trailers `Stolen-From: <repo>@<sha>:<path>` and `For-Clause: <id>`.
   - It stays only if it passes that clause's conformance tests unchanged or with trivial edits.
   - The reviewer judges it against the contract, never against how the old code behaved.
4. **Old tests are not ported.** Tests come from the clauses.
5. **The taint check (CI):** the v1 tree must not contain banned old-world mechanism names or old crate paths. The list lives in `docs/taint-denylist.txt`, and each side may extend it. A hit fails the build.
   - The check scans **code and generated artifacts**.
   - It **excludes `frozen/`**: the immutable contract text names some old mechanisms, but only to forbid them (for example HC P3's "credits") or in non-normative "Today" and "Reusable from" notes.
   - Contract crates and generated code **cite clause ids**. They never copy contract prose verbatim into doc comments.
6. **No contract workarounds.** If the code needs the contract to change, file an amendment with the steward.

## Architecture facts every agent must know (orchestrator, 2026-09-30)

**Terminal semantics are libghostty.**
- Botster's terminal semantics come from libghostty, Ghostty's terminal library, built from the trybotster/ghostty fork at a pinned, frozen SHA with Zig 0.16.0.
- The real Core embeds it through the `botster-terminal-ghostty` crate of botster-core.
- libghostty parses PTY output, keeps the shadow terminal, answers terminal queries, takes GHOSTSNP snapshots, and encodes key, mouse, focus and paste input for the current modes (Core IN-9 relies on the encoders of the pinned Ghostty).
- Botster never re-implements any of that.

**Rules that follow (binding on every side):**
1. **No terminal emulation, parsing, mode tracking or input encoding anywhere outside libghostty.** In botster-contracts, a fake route carries scripted bytes only (steward ruling R-5).
2. **Expected terminal bytes are never hand-written.** A conformance id whose subject is terminal bytes (the Core 5.1A encoding, snapshots, query answers, terminal state) is **real-only**: it runs in the Core repo, where libghostty itself is the oracle. Against a fake, such a transcript is checked only structurally.
3. **Every expected value in a transcript is fixed by a cited clause sentence.** Each expectation names the clause that fixes it. If no clause fixes a value, the pair asks the steward through its lead; nobody invents one. The reviewer checks the citation of every expected value, not only the clause id. A suite that passes only against a fake written by the same pair proves internal consistency, not correctness.
4. **No hand-rolled stand-in for a real component.** A fake models only what its clauses state about the boundary. It never re-implements a real component's internals (a terminal, a protocol peer's internals, a store) to produce expected values.

- **Ghostty fork policy (the user, 2026-10-02):** "I don't want to patch ghostty without first pulling in from upstream main. It's always moving and I want to be sure we sync with that regularly."
  - BEFORE any new patch to trybotster/ghostty, sync with upstream ghostty-org/ghostty `main`: rebase the patch stack onto the latest upstream main, rebuild, and re-run the libghostty audit and the Core tests. Drop any patch that upstream now covers.
  - Sync at least at every stage start and before every pin move, as well as before patching. Record the upstream SHA and the patch list in the plan and the state log.
  - Keep the patch stack small and rebasable, one concern per commit. Contacting upstream (issues or PRs) still needs the user.

## Testing: fast and lean (the user, 2026-09-30)

The old modular repos ended with slow, flaky suites: 55–70 min queue waits, a 15-minute lifecycle target, and campaigns to reproduce flakes. v1 tests must stay fast and lean from the first commit. These rules bind every side.

**Shape**
1. **Most tests are in-process.** Conformance transcripts run against fakes or an in-process implementation, with no child processes, sockets on disk or daemons. Unit tests are the same.
2. **Real-process tests are few and named.** They cover only what in-process tests cannot prove, such as the PTY, fd handoff, a real WebRTC peer, adoption across a real restart, and journeys J1–J7. They live in a separate `slow` tier, run at landing, and each one cites the clause it proves.
3. **Every test proves a clause or a real bug.** No test duplicates another, and none tests a helper's internals. A deleted test must name what still covers its clause.

**Speed**
4. **Budgets, enforced in CI.**
   - The default tier of a repo (`cargo test`, or `npm test`) finishes in **under 60 s** on this machine.
   - A single default-tier test that takes **over 2 s** fails CI. It is either fixed or moved to `slow`, with the move justified in the commit.
   - CI prints per-test times (`--report-time` or equivalent). A doubling against the previous main is a review finding.
5. **No sleeps or polling.** Tests wait on the real event (a completion, a frame, a pipe, a process exit). The only timers allowed are marked deadline timers: `// timer: deadline — reason`.
6. **No global test locks and no serialized suites.** Each test gets its own temporary root (canonicalized; macOS /var is /private/var) and its own ports or sockets. A test that needs to run alone is a design bug.
7. **Build the test profile for speed.** Optimize hot dependencies in dev (for example `[profile.dev.package.sha2] opt-level = 3`). Do not spawn a CLI binary when a direct call proves the same thing: the first launch of a new binary on macOS can stall for seconds.

**Adopted tools** (botster-contracts `docs/tooling.md` has the versions and reasons; every repo inherits these):
- Property tests: **proptest** (with **test-strategy** derives) in Rust, **fast-check** in TypeScript. Capped cases and pinned seeds in the default tier; generated values are validated against the checked-in JSON Schemas.
- Decoder fuzzing: **bolero**, one harness per decoder; it runs as a property test on stable and as a fuzzer at landing.
- Mutation tests at landing: **cargo-mutants** `--in-diff` on the changed crates, and weekly on fakes and codecs; **StrykerJS** for TypeScript packages once they are substantial. A surviving mutant is a review finding, handled by the PR's risk tier ("Risk tiers" above).
- Runtime schema validation in TypeScript: **Ajv** (`Ajv2020`).
- Concurrency (Phase 1): **shuttle** with shuttle-tokio for the Hub and the Core handle; **loom** only for a hand-written lock-free primitive.
- Not adopted: cargo-fuzz alone, deku or binrw, ts-rs, specta, typeshare, zod-from-schema, Pact, Specmatic, TypeSpec, insta. See `docs/tooling.md`.

**Hygiene**
8. **No test hooks in production code.** Nothing like `BOTSTER_ENV=test` branches. Fakes and dependency injection replace them.
9. **Seeded interleaving instead of reruns.** A flake is fixed structurally, by waiting on the real event or by fixing the race. One isolated run classifies a flake. There are no reproduction campaigns and no "rerun to confirm".
   - **One fail-closed exception (orchestrator, 2026-10-11): the mutation profile.** In the mutants nextest profile ONLY, a test that catches a mutant may run once more. Fail then pass is FLAKY, and the mutant counts as MISSED, so it must be caught by another test or listed with its reason. A rerun may only remove a catch, never turn a failure into a pass for any verdict. Every FLAKY result names its test in the gate output and opens a structural fix under this rule. Every other test profile keeps zero retries.
10. **Nothing is left behind.** Every real-process test owns its process group and cleans it up on every exit path, panics included. CI fails if a test leaves processes behind.

## Capturing decisions and learnings

Leads run `/capture` in their own sessions to record decisions and learnings in the knowledge vault. They do it at each milestone, and right away for anything a future agent would otherwise rediscover: a design decision, a convention, a gotcha, a ruled ambiguity. The steward captures spec rulings. Nothing is captured twice: check the vault first.

## Agent cleanup (the user, 2026-09-30)

- **When:** a feature's pair is retired once the feature is MERGED to `main` AND `main`'s CI is green on the merge commit. Not before: until then the pair keeps its context for review fixes.
- **How, in order:**
  1. Check that the worktree's HEAD is merged or pushed and that `git status` is clean. Save any untracked file worth keeping into the repo, or archive it under `~/botster-sessions/archive/`.
  2. Clean its `target/` and `node_modules`, and remove its Mac gate tree with `botster-gate --clean <worktree>`.
  3. Call `delete_agent` for the reviewer, then for the implementer.
  4. Check that the worktree directory is gone. If it remains, remove it with `git worktree remove` (and delete the local branch), or list it in your DONE for the orchestrator. The remote branch stays.
- **Helpers:** only the lead staffs. An implementer or reviewer never spawns agents; it asks the lead to split a feature instead. A helper the lead spawns for a parallel chunk counts toward the concurrency limit, and it is retired the moment its chunk is merged into the feature branch, not at the end of the feature.
- **Labels** are readable to the user: `<feature> — implementer`, `<feature> — reviewer`, or `<feature> — helper (<chunk>)`.
- Long-lived roles (a lead's design reviewer) are retired at the phase DONE in the same way. A lead stays until the orchestrator confirms the phase. A phase DONE includes a line "agents retired: N; live: <list>".

## Amendments

- **An amendment is not FINAL until its crate change is merged on the same final branch.** The steward does not publish an amendment or an erratum as FINAL while a type, field, variant, error code, limit, default or feature that it adds or changes is missing from its crate.
- **`cargo xtask crate-symbols --check` enforces this rule.** Each file that `contracts.toml` lists has a table `crate-symbols/<file stem>.toml`. The table maps each code span of the file, by line, to the crate symbols that the span names, or to `n/a` with a reason. The check fails when a symbol does not resolve, or when a word of a span is neither covered by a symbol nor listed in `uncovered` with a reason. The CI `ledger` job runs it.
- A base file with no table is PENDING. The PENDING list may only shrink against the base branch.
- A crate gap is a span or a word that a contract defines and no crate symbol holds yet. Its reason starts `GAP(<closer>): `, where the closer names the PR that adds the symbol (`core`, `codec`, `hc`, `hp`, `lb`, `ui`, `cli`, or `final36`). The gap list may only shrink against the base branch: a new gap fails CI. Add the crate symbol instead.
- A new contract file is a new file (the manifest job checks each hash). For its table, run `cargo xtask crate-symbols --draft <file>`: it prints a draft row for each span that the table does not have. Review each row by hand, then run the check.

## Practical rules

- A gate goes through `botster-gate` ("How each stage runs"). Other heavy work on the Mac (cargo, npm, live suites) goes through `botsterq run`, with a `--deadline` on every job.
- Use one `target/` per worktree, and clean it after landing. Run `df -h /` before a full run, and clean first if free space is under 30 GB.
- Shared state lives in git or in `~/botster-sessions/shared`. Never use `/private/tmp`, which the OS erases at reboot.
- Never kill processes by name or pattern. Kill only your own verified process groups.
- **CI access:** this repo is private. A public repo whose CI depends on it needs a read-only token or deploy key for `trybotster/botster-contracts`.
