# Integration review: P5 adoption design (branch stage1/p5-adopt-1, `crates/botster-core-host/DESIGN.md`)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. Design only; no code.

## Round 1 — NOT CLEAN on head 0682aabe

Reviewed head: `0682aabe47f18b6756cc8c5655068f5a9e27e7b9`. P5 announced `d6c2cf6d`; the branch moved two commits past it
(`a19f22f7`, `0682aabe`: the Sim process table is in memory, and `connect_worker` is a `HostEdges` method). Both are
reviewed here. Base: v1 `a0f78fe4`. Contract: core contract v1.17 with the amendments at `contracts-v0.1.13`. Scope: the
cross-package parts (`botster-core-link`, `botster-worker-core`/`botster-worker`, `botster-core-testkit`,
`botster-core-sys`).

### Checked, no finding

- **Direction (the worker listens, the new host connects).** It follows from DP-8 (the hello binds the host epoch, so
  the host speaks first) and from AD-6 ("worker or guardian endpoints"). It matches the old daemon's direction.
- **The role byte in the proof.** The current proof is `SHA-256(DOMAIN, token, epoch, instance)`
  (`botster-core-link/src/proof.rs:18,56`), the same value in both directions. In the adopt handshake the host speaks
  first, so an impostor could echo it. A role byte stops that, and one rule for both handshakes is right. `hello.rs`
  already leaves the proof rule to the AD-6 owner, and protocol 1 is not released, so no adoptable worker breaks.
- **The fence retires the old host's requests (3.4).** Request numbers are per link, so this is required. The test (a
  write in flight at the fence, and the same request number on the new link) is the right red test.
- **Bounded candidates (part 7).** One candidate at a time, the `startup` deadline, and a one-`Hello` frame bound. All
  three are sans-IO decisions with default-tier tests. This matches the decision rule of plan r22 section 8.
- **Testkit (part 6).** The Sim process table is in memory only, and real processes stay in `botster-test-process`.
  `connect_worker` is a `HostEdges` method, so `RealCoreHarness` does not change. This agrees with P6.
- **No rebind and no repair of a missing endpoint.** AD-2: a `Lost` session is never restarted in place.
- **A52 `Option<u64>`.** `None` never matches. This is better than a sentinel 0.

### D1 MEDIUM — "E above every epoch it has seen" blocks the AD-2 retry by the same host

Step 3 requires the host's epoch E to be **above** every epoch that the worker has seen. Step 4 records E before the
host has checked the worker's answer. DP-8 says: "A worker obeys only the **highest** epoch it has seen". So E equal to
the highest seen epoch is a valid epoch.

Failure case:
1. Host H (epoch E) adopts worker W. W passes step 3, records E, fences, and answers.
2. H does not complete the handshake: the report is late (3.7, `startup`), or the link breaks. The row is
   `Lost(WorkerUnreachable)`.
3. AD-2: "`begin(Adopt(id))` may be retried". H retries with the same E. W refuses it, because E is not above E.
4. Under H, the session can never be adopted, and every retry is `Lost(WorkerUnreachable)`, which says "may be retried".

Fix: the worker accepts E when E is **at least** the highest seen epoch. A hello with E equal to the current link's
epoch replaces that link after the proof passes, and the 3.4 fence applies to it too (the request numbers of the old
link of the same host also collide). A lower E is refused, as now. This keeps fencing: only one host holds epoch E,
under the data-directory lock (DP-8, LC-2). Add a test: an adoption that the host abandons after step 4, then an
`Adopt(id)` with the same epoch, which succeeds.

### D2 LOW — the worker has no way to learn `startup`

Part 7 and AD-7 need the worker to use `CoreLimits.startup`: for the self-exit of a worker with no payload and no host,
and for the candidate deadline. `WorkerLaunch` carries only `--role`, `--control`, `--instance` and `--epoch`
(`botster-core-link/src/launch.rs:34-40`), and the design adds only `--endpoint`. Name the launch argument that carries
`startup` (in `botster-core-link`, with a parse test), so the value comes from the host's `CoreLimits` and the worker
invents no deadline.

### D3 LOW — the endpoint file has no owner for its removal

Part 1 creates `<data_dir>/w/<InstanceId>`, but no step removes it. State who unlinks it, and when. For example:
- the worker, when it exits;
- `Remove` (LC-7), after the worker's end is verified. SV-9 names "the endpoints" among the things that a remove
  releases.

Otherwise every removed session leaves a socket file in `w/`.

### D4 LOW — state that the hello and the proof rule are the same for protocols T and T - 1

AD-4 requires an N - 1 worker to adopt (`conf::ad_4_previous_worker_version_adopts`) and any other version to give
`Lost(WorkerVersion)`, not `WorkerUnreachable`. That works only if a worker of protocol P can read the hello of a host of
protocol T and check its proof. The hello is JSON, and its decoder ignores unknown fields (`hello.rs`), so the format
supports this. Write it into part 2 or 3 as a rule: the five hello fields and the proof rule (domain, role byte, field
order) do not change between protocol numbers. A future change to them would turn `WorkerVersion` into
`WorkerUnreachable`.

VERDICT: NOT CLEAN (4 open: D1 MEDIUM; D2, D3, D4 LOW)

## Round 2 — no integration finding open on head 78f4be5c; NOT CLEAN for the F22 QUESTION

Reviewed head: `78f4be5c11de2bab2a48d01cc24b07e8069f5aaa`. Delta `0682aabe..78f4be5c`, `DESIGN.md` only.

- **D1 CLOSED.** 3.3 accepts E at least the highest seen epoch, and refuses a lower E. An equal E replaces the current
  link after the proof passes, with the 3.4 fence. The test is named: an abandon after step 4, then `Adopt(id)` with the
  same epoch adopts, and a lower epoch after it is still refused. This is also the P5 package reviewer's F20.
- **D2 CLOSED.** `--startup-ms` (a `u64` in milliseconds) is a new launch argument. `WorkerLaunch::parse` refuses a
  missing or malformed value, with a parse test. The worker has it before any `Launch`, so its orphan deadline covers every
  AD-7 crash point.
- **D3 CLOSED.** The worker unlinks its endpoint at `Exit` and on `Terminate`. The host unlinks the path after `Remove`
  has verified the worker's end. A failed unlink goes to `diagnostics()` and does not fail `Remove`. The `InstanceId` is
  unique, so this unlink cannot reach another session's endpoint.
- **D4 CLOSED.** The rule is written in part 2: the five hello fields and the proof rule never change between protocol
  numbers. The link docs state it, and a test pins the encoded hello and the proof of a fixed input.
- The P5 package reviewer's F21 (the adoptable set at T = 1 is {1}, from one function) and F22 (a worker with no running
  payload) are theirs. F22's reading of AD-1 with AD-7 goes to the lead as a QUESTION before it is coded.

### Observation for the F22 QUESTION (not counted)

The F22 table gives `Lost(Other)` for a `Running` or `Exited` row whose worker reports `NotLaunched` or `Spawning`. AD-2
names `Other` only as the reason that a **host** maps an unknown reason to. It does not list `Other` as a reason that Core
reports. Put this in the same QUESTION: may Core report `Lost(Other)`, or must it use a listed reason?

VERDICT: NOT CLEAN at 78f4be5c11de2bab2a48d01cc24b07e8069f5aaa (0 integration findings open; the design waits for the
lead's answer to the F22 QUESTION and for the P5 package reviewer)

## Round 3 — CLEAN on head 4fec490a

Reviewed head: `4fec490a97f291c05622aa92be2f02c74a891c30`. Delta `78f4be5c..4fec490a`, `DESIGN.md` only: the no-payload
table and the retry rule.

- **The table matches steward ruling R-35** (contracts `main` `f969f5e`, corrected by `c3ed727`, both read here):
  - `Starting` with `NotLaunched`: one `Launch` (AD-7 step 4). A failed launch takes the ordinary `Start` outcome (LC-4).
  - A spawn in progress is not repeated.
  - `Stopping` with no launch: `Exited{cause: HostStop}`, with no `Launch`.
  - `Running` or `Exited` with no payload: `Lost(RegistryCorrupt)`.
- **The observation of round 2 is resolved.** `c3ed727` says that Core never reports `Lost(Other)`. The design removes P1's
  placeholder, and the PR must list every Core-side construction of `Lost(Other)` with its fix (the pattern rule).
- **The retry keeps the intent.** A `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` adoption never rewrites the row.
  The retry applies the tables with the row's recorded state (the intent) and the new report's payload state (the facts).
  So the payload is launched once, and a `Stopping` row never gets a `Launch`. Two tests are named. This is the P5 package
  reviewer's F22 remainder, which is theirs to close.
- **The interaction with D1** (an equal epoch is accepted) is safe: the retry runs the full handshake and the 3.4 fence,
  and it takes the payload facts from the new report only.
- The ids stay pending until the real harness can run them (plan 5).

VERDICT: CLEAN (0 open) at 4fec490a97f291c05622aa92be2f02c74a891c30
