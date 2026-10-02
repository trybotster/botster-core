# P0 skeleton review

Reviewed head: `468d35b0c0641e83640ac756ada5c872706c77a5`.

Previous reviewed head: `74f2e9afcd16af8483093f3f12765ca397c31b73`.

VERDICT: CLEAN (source review; final gate pending)

The second review checks logic against BUILD.md, contracts-v0.1.1 (`366bca41`, manifest final14), the P0 brief, and plan pin `stage1-plan.555bc433`.
The first review used manifest final13 and plan pin `stage1-plan.a24efe7e`.
The Prior art note exists in PR #125. I read the note and its proposed decisions.
I ran no tests or full gate. The findings below follow from the submitted source and the installed libtest-mimic source.

## F1 — HIGH — The decoder allocates before it checks the length

Status: CLOSED at `7d30cb47`.

Closure: The decoder checks the fixed header before storing payload bytes. It returns the consumed length and holds one frame. The oversized-input regression checks zero retained capacity.

Evidence: `crates/botster-core-link/src/frame.rs:95-97` appends every supplied byte to a `VecDeque`.
The decoder checks the header length only at lines 119-127, in `next_frame()`.
For example, `FrameDecoder::new(16).push()` can receive a header that announces 17 bytes followed by a large body.
The decoder copies that body before it rejects the header. Repeated calls to `push()` can also grow the buffer without a bound.
The header-only rejection tests do not prove the allocation rule.

Requirement: plan section 3 and brief item 3 require the length check before allocation.
Plan section 2.5 also requires one maximal link frame as the receive bound.

Required change: check each frame header before buffering its body. Keep buffered bytes within an explicit bound.
If the decoder cannot consume all supplied bytes within that bound, return the consumed length or apply explicit backpressure.
Add a focused regression for an oversized header and body in one input chunk.
Check retained capacity as well as the eventual error.

## F2 — MEDIUM — The hello does not use the planned extensible control payload

Status: CLOSED at `7d30cb47`.

Closure: The hello uses serde JSON. It requires the five fields and accepts unknown fields. The required-field and unknown-field regressions match the rule.

Evidence: `crates/botster-core-link/src/hello.rs:88-94` writes a fixed binary payload.
Lines 114-115 reject every trailing field.
The hello therefore cannot accept an added field from a newer worker that retains the same protocol number.
The PR records the layout but gives no approved change to plan section 3.

Requirement: plan section 3 specifies JSON for control messages so additive fields keep the previous protocol compatible.
The hello is a control message. Core AD-4 supplies the version compatibility requirement.

Required change: use the planned JSON control payload for the hello. Accept unknown additive fields while checking the required fields.
Preserve the framing bound, the protocol number, the instance, the token proof, and the host epoch.
If a binary hello is necessary, ask the lead to decide the plan change before treating the layout as accepted.

## F3 — MEDIUM — Prebuild overwrites an existing executable inode

Status: CLOSED at `7d30cb47`.

Closure: Prebuild copies each executable to a fresh file and renames it over the candidate. It writes the manifest after both installations.

Evidence: `xtask/src/prebuild.rs:65` and `:85` call `std::fs::copy` directly onto the candidate executable paths.
When a candidate already exists, the copy truncates and rewrites the same file.
The pinned prior art, `script/prebuild-worker`, explicitly replaces the file before copying.
Its comment records that macOS can kill a re-signed binary written over an inode that already ran.
The same problem applies to the probe, which P0 already builds.

Requirement: brief item 8 and plan sections 5 and 7.1 require the prebuild mechanism and its prior-art lessons.
The prebuilt candidate must remain usable by the real-process tier.

Required change: copy each executable to a fresh file and rename that file over the candidate path.
Alternatively, remove the old candidate before copying, as the pinned script does.
Write the manifest only after both replacements succeed.

## F4 — MEDIUM — The mutation configuration excludes all gate logic

Status: CLOSED at `7d30cb47`.

Closure: The configuration removes the blanket xtask exclusion. It records narrow function exclusions for process and file operations. Pure checks have mutation tests. The gate must confirm that the six previously missed mutants close.

Evidence: `.cargo/mutants.toml:9` excludes `xtask/**`.
This excludes the pending and deferred checks, protocol start conditions, caps, timing checks, and other decisions with unit tests.
The PR says that a normal gate run exercises this code. A normal gate run does not prove that the tests detect a changed decision.
The reported 51 caught mutants do not cover these excluded decisions.

Requirement: BUILD.md requires mutation tests at landing. Plan section 8, step 8, applies them to the changed crates.
The plan gives no blanket exemption for xtask.

Required change: remove the blanket xtask exclusion. Keep any necessary exclusion narrow and record the reason for each excluded operation.
Run mutation tests on the pure gate decisions. Close every missed mutant or timeout, including an accepted explanation for an equivalent mutant.

## F5 — MEDIUM — The harness can report pending or unselected ids as passed

Status: CLOSED at `7d30cb47`.

Closure: Pending, deferred, and excluded trials return Completion::ignored_with at runtime. The harness applies selection before creating runnable trials. The deferred report includes authority and start condition.

Evidence: `tests/conformance.rs:89` and `:105` create pending and deferred trials with closures that return `Ok(())`.
The ignored flag does not prevent execution with `--ignored` or `--include-ignored`.
The installed libtest-mimic 0.8.2 source confirms this behavior in `Arguments::is_ignored`.
Such a run increments `conclusion.num_passed`, which lines 120-125 print as conformance passes without any proof.
Lines 58-59 also return success for a transcript excluded by the runner selection, once a harness exists.

Requirement: plan section 5 says that pending and deferred ids are never counted as passed.
Only `Outcome::Passed` establishes a passing proof.

Required change: make pending and deferred trials remain ignored at runtime, even when the caller requests ignored tests.
libtest-mimic 0.8.2 provides `Trial::ignorable_test` and `Completion::ignored_with` for this purpose.
Apply the runner selection before creating runnable trials, or report excluded trials as ignored.
Keep the passed count limited to transcripts that actually return `Outcome::Passed`.
Include the authority and start condition in the deferred report, as plan section 5 requires.

## Accepted scope

The facade shell uses a `Cell` marker for `Send` and non-`Sync` behavior.
The workspace pins the contracts tag. The worker protocol constant is 1.
The conformance records contain 599 ledger ids, 597 pending ids, and the two A6-2 deferrals.
Commit `497d255` contains only the contracts pin, lockfile, and eight added ledger and pending ids.
The owned ids remain pending until the later packages provide their conformance proofs.
P0 does not need terminal semantics or a second terminal implementation.

Every finding must close before CLEAN. Any new head requires a review of its changes.

## F6 — MEDIUM — The JSON fuzz property requires byte identity

Status: CLOSED at `468d35b0`.

Closure: The property compares decoded values. The deterministic regression includes whitespace, reordered fields, and an unknown field.
The PR now says xtask is mutation-tested with per-function glue exclusions.

Evidence: `crates/botster-core-link/tests/props.rs`, the final assertion in `link_decoder`, still requires `again == bytes`.
The hello decoder accepts field order, whitespace, and unknown fields. The encoder emits its fixed field order and removes unknown fields.
A valid hello with a leading space therefore fails this assertion. A valid hello with an unknown field also fails it.
The changed comment promises an encode and decode check, but the assertion still checks the old binary property.
The random default run can miss valid JSON. The fuzz gate can report a crash for correct decoder behavior.

Required change: compare `Hello::decode(&again)` with the decoded `hello`.
Add a deterministic regression with noncanonical JSON and an unknown field so the check cannot depend on random discovery.
Update the PR decision that still says xtask is excluded from mutation. The later mutation section contradicts that decision.

Second review: I ran no tests or heavy jobs. I checked the production changes, the property logic, the mutation exclusions, and PR #125.
The final gate must run after the source review is CLEAN. Gate failures remain findings and require closure.

Third review: Only `crates/botster-core-link/tests/props.rs` changed after `7d30cb47`. I checked the change and the corrected PR decision.
All six source findings are closed. I ran no tests or heavy jobs. The implementer must run the final gate.
The final mutation run must confirm closure of the six previously missed mutants. Every gate finding must close before landing.
