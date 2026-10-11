# Integration review: #245, contracts-v0.1.29

VERDICT: NOT CLEAN (1 open)

## Round 1 and delta round 2

- Initial head: `df991fdf57122c7b59b06069ab7da01c992ca175`.
- Initial base: `f6778ad85c01266ac49e2ededd97487da689e920`.
- Current head: `c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4`.
- Current base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Risk: HIGH. The contracts pin changes on all seven manifest dependencies.
- Package supplement: `f774f13238d3499e9af5ae45811ff8488435ebeb`, read in full.
- The package supplement covers the initial head. Its delta artifact remains pending at this record.

The initial head had two open findings. The current head closes PIN-SI-1 and retains PC-PIN-1.
Sol reviewed source and retained evidence. Sol ran no build, test, gate, mutation, reversal, or executable merge-check job.

## PC-PIN-1 MEDIUM: EV-5 fails on the real Mac tier

Status: OPEN.

The real Mac run fails `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded` at seed 6, step 21.
The transcript expects `"success"`. Core returns `PendingLimit`, category `capacity`, detail `pending_ops is 4`.
This active ID must pass. The cause remains unknown.
The EV-5 transcript has the same blob, `bc95b3f2cfc3e7b37be2c80b22835fd82e9499ad`, at the old and new contracts pins.
That equality does not establish whether the failure is a pin regression, a Core defect, or a transcript race.

Mac evidence head: `f5bb74ceab93cbb8e12d070fb15806375ce0d791`, macOS 27.0.1, arm64.
Log: `~/botster-sessions/gates/botster-core-pc-pin-v0.1.29-mac-f5bb74ce-20261011-005918.log`.
Sol read the complete summary and the failure block in `.step2.raw`.
The strict testkit run passes 206 tests. The real run passes 127 tests and fails EV-5; it exits 101 after 158.69 seconds.
The pending-real run reports zero passes. Two held testkit IDs pass and stay held.

Five isolated Mac runs pass in `botster-core-pc-pin-v0.1.29-ev5-repeat-mac-f5bb74ce-20261011-010317.log`.
Sol read all five results. Those passes do not resolve the failure under load.
PC reports 13 failures in 30 later trace runs, all at step 21, in message `msg_plugin-w_1791706585_982028`.
Sol has not received or reviewed that trace log. PC reports that the old-pin comparison is still running.
Sol will wait for its receipt. Sol will not poll the job.

Required closure: determine the cause and retain the evidence.
The correct owner must resolve the failed contract check and retain proof under the conditions that caused the failure.
If the resolution changes the transcript, obtain the lead and steward instruction first.
Do not weaken an expectation. Do not use isolated passes as closure.

## PIN-SI-1 LOW: R-59 was absent from the pin description

Status: CLOSED at `c57ec826`.

The initial Cargo.toml comment and PR summary listed R-54 through R-58.
The pinned tag also contains R-59. Sol reported this finding to PC and the package reviewer.
The package reviewer agreed in supplement `f774f132`.
The current comment lists R-54 through R-59 and identifies start_held and its fences as later work.
The current PR summary also states that unfenced AD-7 does not count toward the real minimum.
Sol read both corrections. No runtime change was needed.

## Pin, source, and dependency checks

Plan 23ak is `4eceef06434d643bf31e965a35a75d4252d27d19`.
Its published pin has SHA-256 `db7ba8e5c0748aa53257fb53a5e6f8923a9549c6c9871127a7d2ef2c97000813`, independently verified by Sol.
The required contracts-v0.1.29 commit is `65f76ab9829d7a0fda2835c10abc7ffae2c35215`.
Core moves directly from contracts-v0.1.27, `b161db87ca82873b4f6b21d8433a69f086abb7cf`. Core never pinned .28.

Sol parsed the manifests and lockfiles. Only seven manifest tag values and nine lock source values change.
No other manifest value, lock package, version, checksum, or dependency changes.
The other two PR paths change comments only. All Core pending and real-pending IDs remain unchanged.
The contracts delta has fourteen paths. It changes no frozen manifest, ledger, pending list, or deferred list.
The PR changes no mutation exclusion and removes no ID.
EV-2 and IN-5 remain pending until #233 supplies its controls and its final gate passes on the new pin.
The six IDs that need steward approval remain pending.

Sol reviewed the dependency logic in the preceding plan reviews and this pin review.
R-56 adds await_pty fences to six transcripts and keeps their literal assertions.
R-57 adds await_title and EV-2 sentinel bytes. The driver pumps and reads cached terminal state without polling events or advancing the clock.
The title wait keeps the launch and adoption scope limit and uses the existing step bound.
R-58 uses the host's first RouteClosed reason for each handle generation and route.
The driver uses A17's five-reason mapping and ignores a route endpoint's claimed cause.
The pin PR reuses those dependency implementations. It adds no alternate driver.

R-59 keeps unfenced AD-7 outside the counted real minimum.
The current counted minimum is 35/68, plus one unfenced pass. Historical reports show 36/68 and accepted 37/69.
The start_held control and fences remain later work. R-60 and R-61 also remain outside this pin.
This review awards no minimum progress.

Prior art: Sol checked OLD CORE `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and its workspace manifest.
OLD CORE has no contracts pin to reuse. This PR uses Cargo's existing tag and lock mechanisms and adds no custom implementation.
The PR's Prior art note matches that scope.

## Import and gate evidence

The Mac head and initial reviewed head differ only in manifest comments, xtask merge logic and tests, and real-pending comments.
They change no relevant runtime, harness, transcript, or dependency value.
The retained Linux gate uses the initial reviewed head and base.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.29-df991fdf-pool-20261011-010434-15999.log`.
All ten steps pass. The summary reports 191.4 seconds; the job and wrapper exit 0 after 254 seconds.
Default tests pass 1610 cases. Slow tests pass 427 cases. EV-5 passes on both tiers.
The strict report has 373/394, 21 real-only IDs, two held IDs, and testkit minimum 55/69.
The pending-real report has 77 IDs and zero passes.
The mutation step reports no mutant in the diff. The fuzz step reports no changed decoder harness.
Sol read the header, relevant test records, mutation result, and complete final summary.

The current head imports v1 #247 through `9f106e3ed01a317ad91738519c7f4212f4903158`.
Its tree, `5f672ba97fc43e4e2543405c31f774c7d0a81632`, equals Sol's independent git merge-tree result.
The correction after that import changes only the R-59 comment.
All other PR files equal the initial reviewed files. The imported sha2 profile is the reviewed #247 profile.
The retained executable base-merge-check fails conditions 2 and 3 because Cargo.toml changes on both sides.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-df991fdf-c57ec826-20261011.log`.
Sol read the full result and completed this source delta review.
The initial gate is not a final gate on the current head. The final gate remains required after both reviewers give CLEAN.
PC-PIN-1 prevents CLEAN now. This verdict grants no merge approval and does not close #233's separate mutation finding.
