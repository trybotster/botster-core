# PC package review: PR #242

VERDICT: CLEAN

- Reviewed head: `75cb8b37c2691bf156d15f62b06b45fb3879b010`.
- Reviewed base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (`v1`).
- Branch: `stage1/pc-title-order`.
- Risk tier: LOW. The PR adds tests only in botster-core-host.
- Open findings: 0.
- Scope: the 116 added lines in `crates/botster-core-host/src/tests/driver/observations.rs`.

The lead assigns this PR to the PC package reviewer alone. No integration review is required.
Lead message: `msg_plugin-w_1791699598_53afe2`, quoted by the implementer in `msg_plugin-w_1791699652_879436`.
READY: `msg_plugin-w_1791703733_716b5f`.
The binding plan is revision 23aj, `stage1-plan.6dbe1c43.md`.
Its verified SHA-256 is `25f26484bc439817c213a1766fc6392f774b276b0bd4796eaea3bb890a5e82e9`.
The review uses BUILD.md and steward ruling R-57, condition 3, including its launch and adoption scope limit.
The current code still pins contracts-v0.1.27. Plan 23aj assigns the later .28 move to a separate HIGH PR.

## Logic

The reviewer read the complete delta and the existing Rig, driver, observation handler, sync terminal-state read, and event queue.
The first test sends a Bell observation and then a title observation over the same worker link.
It uses pump_events 1. After each pump, it reads terminal_state without polling events.
It requires a pump to end before the sentinel title appears.
When the sentinel appears, the test drains the queue and requires Bell before the sentinel's TitleChanged.
This checks the required budget split and the event order at the sync read.

The second test also sets event_queue 1. It sends Bell, Notification, Bell, and then the title on the same link.
It pumps without polling until the title appears.
It then requires the loss marker to name both Bell and Notification, and requires the sentinel's TitleChanged.
The marker therefore includes the earlier observations when the title read supplies the fence.

HostEngine::observe sets the cached title and posts TitleChanged in the same call.
The driver applies each observation frame in link order and checks its budget between frames.
The sync terminal_state read clones the cached state. It does not pump or poll the event queue.
The tests use the existing driver Rig and limits helper. They add no production instrumentation or private-state accessor.
They change no production code, transcript, expected value, timeout value, list, pin, or mutation exclusion.

A launch or adoption report sets the terminal baseline without a per-field event.
The PR body states that no launch or adoption can fall between sentinel output and await_title for this fence.
The tests finish launch before they send the fenced observations. They perform no adoption during the wait.
This verdict clears R-57 condition 3 within that scope. It does not clear the later contracts step or EV-2 transcript change.

## Prior art and reversal

The reviewer read the PR body and checked the pinned old Core references at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Old Core has the named title event and forwarding path. It has no pump_events budget.
The note records no reused old code or ported old test. The tests use the existing driver helpers.

The reviewer read the selected reversal log:
`~/botster-sessions/gates/botster-core-pc-title-order-reversal-20261010-231854.log`.
It reports both tests failing when observe(Title) omits the TitleChanged post.
The exact-head gate below supplies the normal-run passes. The reversal log is not a full gate.

## Gate evidence

Gate head and base equal the reviewed head and base.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-title-order-75cb8b37-pool-20261011-001710-40110.log`.
The gate uses `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci` on the Linux pool.
The reviewer read the header, both new test PASS records, mutation result, and complete final summary.
All ten steps pass: fmt, clippy, taint, lists, public-api, prebuild-worker, test-budget, slow, mutants, and fuzz.
The diff has no mutant. The diff changes no crate with a decoder harness.
The gate exits with code 0 after 689 seconds. Its CI summary reports 364.1 seconds.
The reviewer confirmed the same head and base in the PR and checked the source diff for whitespace errors.

The reviewer ran no build, test, gate, mutation, reversal, or executable merge-check job.
A later code change requires review. This verdict clears no pending ID and does not complete the PC group.
PR #233 remains NOT CLEAN with SI-5 open in its separate verdict.
