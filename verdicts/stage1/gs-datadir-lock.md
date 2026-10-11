# Integration review: PR #265

## Round 1: source and correction

Head: `80f17bcf142823c5b8cebf16c704e2ee06b2a4cf`.
Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
Parent: `2bac0641e98678053a3e2ce6a0b3adfcaef361f7`.
Tree: `fb97f8805a46ebc275d6c784e41f832108bf4956`.
Branch: `stage1/gs-datadir-lock`. Risk: HIGH. READY: msg_plugin-w_1791723004_f78243.

Sol reads all five base-to-head changed files and the complete corrected PR body.
Sol also reads the initial source at `96238f6209a2cf9587f8c78243b67bdc3336d564` and its complete correction delta.
The static diff check passes. Fetch and gh confirm this exact published source head.
Sol reads complete package Round 1 d23f0e44, preliminary corrections, and Round 2 `b7ce1bfc0673338b1d207a66b5401b069f29193c`.
Fetched origin/stage1/review-gs contains that CLEAN commit. It covers the same head and base with zero open findings.

### Package findings and closure

GS-DL-R1-1 MEDIUM was OPEN at 96238f62. The regression retried a refused reopen and polled progress without an overall work bound.
The correction gives the main test exactly 2000 cycles with one reopen attempt per cycle. The first refusal fails.
A channel event starts the cycles after a spawner's first child ends. Each of four spawners starts at most 20000 owned children.
The test requires child progress during the cycles. Every child has an owned process guard and a bounded status wait.
The destructor signals stop, joins every thread, then reports any failure. It also completes joins while the main test unwinds.
No thread detaches because an earlier join failed. GS-DL-R1-1 is CLOSED with the corrected platform evidence below.
The numerical miss estimate explicitly assumes independent cycles. Sol accepts no proved probability bound from those measured rates.

GS-DL-R1-2 LOW was OPEN because the Prior art note described every old fcntl call as nonblocking setup.
The corrected body includes F_GETFD and F_DUPFD and limits its claim to the absence of a data-directory record lock.
Sol verifies those calls in OLD CORE `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`. GS-DL-R1-2 is CLOSED.

### Prior art

Sol's pinned OLD CORE search finds descriptor fcntl operations, but no flock or F_SETLK data-directory lock to reuse.
The PR replaces v1's LockFile mechanism and reuses the existing rustix operation, std mutex/map, and owned-child guard.
The device/inode table supplies same-process exclusion, which the process-owned kernel lock does not supply.
The retained descriptors preserve that kernel lock across an open race. These rules explain the custom table in the approved design.

### Cross-package behavior

The lead approves this design at 09:15 PT and transfers the LC-2 correction from P6 to GS.
LockFile uses the existing rustix fcntl_lock operation with NonBlockingLockExclusive.
Sol reads the locked rustix 1.1.5 Linux and libc implementations. Both choose F_SETLK and F_WRLCK, with offset zero and length zero.
The existing dependency supplies the kernel operation. No new runtime dependency or unsafe pre_exec callback is added.
The botster-test-process dependency is for tests and reuses the existing owned-child guard.

A process mutex protects the device/inode table through metadata, open, lock acquisition, and entry insertion.
A held metadata key causes refusal before open. A held key found after open retains the new descriptor with its holder.
That descriptor cannot close and release the record lock before the holder drops.
The holder's drop closes its descriptor and retained descriptors while the table mutex excludes another local open.
EAGAIN and EACCES report Held. Other lock errors keep their I/O error. A poisoned table refuses acquisition and permits drop cleanup.
There is no retry or deadline in production open. The std File open retains the existing close-on-exec behavior.

DataDir keeps the LockFile guard and still maps Held to OpenError::InUse. Core's public error remains DataDirInUse.
The epoch update, row storage, into_storage ownership, and handle-drop boundary remain unchanged.
The slow tests use a separate owned child to observe the kernel lock, including same-process and hard-link refusals.
The stat-only walk keeps the lock. Storage operations and a refused second DataDir open also keep it.
The child finds the lock free after the guard drops. The current full gate's public LC-2 and id_2 proofs pass.

The record lock ends when this process closes any descriptor of its file. The host restriction is explicit in LockFile and DataDir docs.
Sol checks the current-base lock paths and callers. LockFile supplies the data-directory lock open; sync and safety checks open directories.
The storage fixture writes lock before an open holds it. No new product caller opens a second lock descriptor.
P5 #257 is a required integration condition: its top-level lock snapshot must use metadata only.
Sol rechecks the reviewed 5e6b1518 snapshot: it records kind, mode, and length without opening lock.
Its separate-child probe uses a real process record lock, checks held before and after snapshot, and checks free after close.
Other regular files, including rows/lock, keep their content checks. Sol's #257 verdict f118477e retains that scope.
Neither #257 nor a later base import is part of this exact source tree. A final import must retain the stat-only correction.

### Exact-head execution evidence

Full Linux gate: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054802-97802.log`.
SHA256: `5430435c4231a65fd2e4a1b0750f0dd2ef9425d0bc7ffcb06a06e610e3d21a44`.
Sol reads its exact header, relevant named proofs, tier summaries, every raw mutant, every catch report, final summary, and exit.
All ten steps pass in 88.4 seconds. Job and wrapper exit zero after 95 seconds.
Default: 1642 PASS, 484 skipped, 9.941 seconds. Slow: 484 PASS, 1997 skipped, 29.085 seconds.
The five new slow-lock tests and seven lock unit tests report PASS. Slow coverage checks 1019 tests.
Testkit-passing remains 55/69. Real-passing remains 40/68 and real-accepted 41/69. All 58 real pending IDs remain; zero pass under ignored.
No pending list or contract pin changes. This verdict accepts no new conformance count or pending removal.

Stage 1 tests nine distinct identities: two caught, seven unviable, zero missed, zero timeout, in 3.6 seconds.
Sol independently matches both raw catch identities to their named reports.
Drop deletion is caught by a_descriptor_of_a_held_file_stays_open_until_the_holders_drop.
The default key replacement is caught by different_paths_do_not_conflict. The seven other replacements are unviable.
Stage 2 does not run because stage 1 misses no mutant. Fuzz reports no changed crate with a decoder harness.
Static record: `/private/tmp/integration-265-mutations.json`.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054238-54403.log`.
SHA256: `2519a85bb01b2159c0d50e83cf1871af7561dce3753e829c911616cc7793a88f`.
Linux evidence: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054725-96840.log`.
SHA256: `2130109693c4a386b5fdc15fa31470c3301cc83998f4fd7ea8f15143b59fe225`.
Sol reads both commands, named lock proofs, all thirty id_2 results per OS, summaries, selected-revert failures, and exits.
All three logs name the exact head and base above. Mac passes 123 core-sys tests in 0.992 seconds; Linux passes 122 in 0.078 seconds.
Each of thirty concurrent id_ selections per OS reports id_2=ok and thirteen passed tests, with zero failures.
Each job then replaces only lock.rs with the base file. The corrected regression fails on the first refused reopen.
Mac reports cycle 15, with two children started; Linux reports cycle 115, with one child started.
Both selected test commands exit 100 after the named assertion fails. They do not fail at compilation or tool setup.
The scripts restore lock.rs afterward. Mac job exits zero after 272 seconds; its wrapper exits zero after 273 seconds.
Linux job and wrapper exit zero after 26 seconds. These selected reverts establish the supplied regression check, not a probability proof.

The PR retains the earlier failed 81fff9ac gate and initial platform evidence.
Sol reads the failed gate's header, both-stage missed NotFound-guard identity, and exit.
96238f62 removes that equivalent guard. The failed gate stays failed; it is not substituted for the corrected-head gate.
No mutation exclusion, production timeout, known-flaky entry, or contract pin changes in this PR.

### Verdict scope

Zero integration findings remain on this exact head and base. The package source and correction closures agree.
This unchanged head requires no repeated full gate under plan section 8.
A later base import needs its applicable review and own exact-head gate. It must retain #257's stat-only lock snapshot.
The lead owns the merge decision. This verdict grants no approval for a future source head or pending removal.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 80f17bcf142823c5b8cebf16c704e2ee06b2a4cf

## Round 2: current-base import and gate

Head: `3c8044f40a6b26400240616b8946b05478b72fca`. Base: `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`.
Parents: reviewed 80f17bcf and exact base ddd3b849. Tree: `7a6ccdc55a282b093859ab80b5cf3cb25b7605f8`.
Import receipt: msg_plugin-w_1791723145_673b9b. Gate receipt: msg_plugin-w_1791723338_e013a9.
Sol reads the complete package source delta `51961e05257108d106bf51911ddecf94f5b4d097` and completion `4dc1bbdcab789f3f3d59bc597068a35d8e5fe0b2`.
Fetched origin/stage1/review-gs contains both. Both cover this exact head/base with zero findings.

Sol reads the canonical base-check log supplied in msg_plugin-w_1791723427_c6d8fc.
Log: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-3c8044f4-base-merge-check.log`.
It reports conditions 1 and 4 PASS, conditions 2 and 3 FAIL on Cargo.lock only, and exit 1.
This is a reviewed delta, not an automatic CLEAN transfer from that failed check.
The independent permitted static merge-tree check equals the recorded tree and reports no conflict.

Sol reads the complete Cargo.lock overlap. The imported .36 pin and worker route-codec dependency equal the reviewed base changes.
The own lockfile addition remains only botster-test-process in core-sys dependencies.
The complete own diff equals the prior reviewed diff after removal of blob index lines, including every hunk header and content line.
Both normalized diffs have 24151 bytes and SHA256 `aa011c324c0b4d43745a9ce608f02ade6a13a4e33c67fdefdd490875da5b1dde`.
All five own added/deleted line sequences match. All 37 outside-own imported paths equal exact base blobs.
The lock source and corrected tests are unchanged. Both previous findings stay CLOSED.
Sol checks the new base's lock paths again. The imported real descriptor hold adds no open of the data-directory lock file.
Sol reads the complete updated body, including the new base, failed base check, delta verdict, and exact-head gate.
The body retains the pre-import Mac/Linux source identities and earlier failed-gate limits.
The lead directs #265 to enter before #257. The later #257 import must keep the reviewed stat-only snapshot.

Exact-head gate: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-3c8044f4-pool-20261011-055225-5278.log`.
SHA256: `dfdb79639ddcc81ae5dd258094ab60eafd13a0a110e1d3427248da73dafee918`.
Sol reads the exact header, named lock proofs, tier summaries, all nine raw mutations, both catch reports, final summary, and exits.
All ten steps pass in 153.0 seconds. The job exits zero after 170 seconds; the wrapper exits zero after 171 seconds.
Default: 1683 PASS, 481 skipped, 11.002 seconds. Slow: 499 PASS, 2012 skipped, 39.160 seconds.
All corrected lock proofs PASS. Slow coverage checks 1031 tests.
Testkit-passing remains 55/69. Real-passing remains 40/68 and real-accepted 41/69. All 58 real pending IDs remain.
Thirty-three ignored pending trials report PASS; this verdict authorizes no removal or count increase.
Stage 1 has nine distinct mutations: two caught, seven unviable, zero missed, zero timeout, in 3.6 seconds.
Both raw catch identities match their named reports and remain the same as Round 1.
Stage 2 does not run. Fuzz reports no changed crate with a decoder harness.
Static record: `/private/tmp/integration-265-3c8044-mutations.json`.

Source delta, current-head gate, package artifacts, and body review are complete. Zero integration findings remain.
The older Mac/Linux results retain their exact original identities; this source import changes no lock or regression code.
A later source head remains outside this verdict. The lead owns the merge decision and must preserve the #257 snapshot condition.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 3c8044f40a6b26400240616b8946b05478b72fca
