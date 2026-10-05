# P6 terminal-oracle controls review

VERDICT: NOT CLEAN (3 open)

Reviewed head: `c57e0da22fb3ea461cd85dc756bbc0abcff5a2f7`.
Branch: `stage1/p6-oracle`.
Review base: `a3b8af5be21389423439fb3c09d6a81d924c987d`.
The reviewer ran no tests or gates.

## Open findings

### O1 — HIGH — Two required oracle controls have no public helper

Evidence: `oracle.rs` exposes state, modes, screen, cursor, notification, and encoding helpers.
It provides no `oracle_query_reply` helper.
`snapshot_controls.rs::hyperlinks` is private and returns cell matrices only for restore equality.
There is no public `oracle_hyperlinks` control that reports `{uris}` from capture pages or a route baseline.
The planned dispatch in `DESIGN.md` does not cover either control.

The brief requires the `oracle_*` controls from `docs/core-testkit-controls.md` at `contracts-v0.1.13`.
That document defines both controls under EV-8 and EV-7.
Existing transcripts use both controls, including `conf::ev_7_hyperlink_in_output_and_snapshot`.
Protected harness dispatch may wait, but the public helper APIs remain in this PR's scope.

Required change: add both public helpers and unit tests in new testkit modules.
The query helper must use a fresh native terminal with the session size, optional prefix, and optional color profile.
It must report native answerability and reply bytes.
The hyperlink helper must decode the supplied Core capture or route baseline through libghostty and report native URIs.
Document both planned dispatch paths without editing protected files.

### O2 — HIGH — An offered capture can hide failed retention within the limit

Evidence: `every_cut.rs:142` reads subject continuation before capture.
Only the `SnapshotTooLarge` branch uses that result, at lines 195–202.
The offered branch ignores unavailable retention and checks only final snapshots after the suffix.
The helper also reads the semantic failure flag before capture, rather than checking it again immediately after capture.

A subject can lose pending retention within the independently measured limit and offer a preceding ground-state capture.
If replay and the suffix restore final equality, this helper reports no retention mismatch.
The control's condition (2) requires failed retention to remain visible in the report.
The lead's accepted classification in the P2 audit makes unavailable retention within the independently measured limit a mismatch.
Final resume equality does not replace that check.

Required change: check retention independently of the capture outcome at each cut.
Record a mismatch when independent pending state is within the limit but subject retention is unavailable.
Preserve inconclusive classification when the pending state cannot be established.
Check the semantic failure flag after capture, before any suffix can change the observation.
Add an offered ground-state capture regression with failed within-limit retention and successful final replay.

### O3 — MEDIUM — Known non-fitting cuts can report a conclusive success

Evidence: `every_cut.rs:134–137` sets `inconclusive` only when `fit` returns `None`.
When `fit` returns `Some(false)`, an offered capture follows the normal resume comparison.
If final snapshots match, the report can contain no mismatches and `inconclusive: false`.

R-30 requires the every-offset corpus to contain cuts whose snapshots fit `max_snapshot_bytes`.
A known non-fitting cut does not establish that prerequisite.
An oversized offer also needs a failure report, rather than a conclusive success.

Required change: prevent a known non-fitting cut from reporting conclusive success.
Make the corpus prerequisite visible in the report, and classify an oversized offered capture as a mismatch when its size is established.
Add a test with a maximum below the independent native size and an otherwise valid offered capture.
Keep `UnknownFraming` inconclusive until P3 supplies worker framing.

## Scope checks and supplied proof

- The delta does not edit `harness.rs`, `program.rs`, `core.rs`, or `worker.rs`.
- Production crates gain no test branches. Terminal expected values come from libghostty.
- Pending ids remain unchanged. Added comments identify the P3 dispatch and framing dependencies.
- The public cut adapter requires real capture and page reads. Native unit adapters explicitly do not prove conformance ids.
- Statement helpers preserve missing probes and call `run_script_events` without event normalization.
- Real-process tests retain group guards. Their children block on parent-owned pipes and receive EOF when the parent exits.
- The parent-exit test waits for child EOF before outer group cleanup. No child busy loop remains in these tests.
- Mutation exclusions remain unchanged. Entries name individual functions and state their reasons.

The supplied focused log reports 168 tests passed at `f1522582`; that command then failed clippy on a test-only clone.
The exact-head checks log reports clippy, formatting, and eight slow tests passed at `c57e0da22fb3ea461cd85dc756bbc0abcff5a2f7`.
The mutation log reports 111 tested, 103 caught, eight unviable, zero missed, and zero timeouts at `f1522582`.
The delta to the reviewed head changes only that test's clone to a copy.

Every finding must close, including LOW findings. Any later head requires review of its delta.
The reviewer kept `verdicts/p6-testkit.md` unchanged and left the pre-existing `.gitignore` edit unstaged.
