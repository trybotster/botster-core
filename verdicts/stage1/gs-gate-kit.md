# Core #331: integration review

## Round 1: complete source and member evidence CLEAN

- Reviewer: Sol-C.
- Branch: `stage1/gs-gate-kit`.
- Exact head: `57cb242f5d3dda40e524090239222e4824021798`.
- Tree: `9b1b569b8f78ab9e70298c28c322fddc3ec9cc66`.
- Base: `644f632f194f62ad7fe14e66482ee01083706e30`.
- Initial READY: `28a29d32d191604221eb4cbeeb02e47371fa3daf`, tree `c4542104426cfdc0b4f042da9d22a409308097cf`.
- Lead assignment: `msg_plugin-w_1791846711_50d7cd`; current scope/ruling: `msg_plugin-w_1791850643_e5ca11`.
- Initial READY message: `msg_plugin-w_1791849312_18b007`.
- Current full-index diff SHA256: `e97fd3e5a3096f4fe3992684f9180c0ef72143ed8b19b96b43a6a3683c9dad84`.
- Complete package Round 207 NOT CLEAN: `c1240a8c79a43e46dc2d9f54cc18526211d820c0`, read in full.
- Complete package Round 208 CLEAN: `a01e5ccd9a31fcd78de0bc2fa52e2d66e05f07ca`, read in full.
- Matching package message: `msg_plugin-w_1791850577_da157c`.

The lead's handoff records the revised function moves, Policy split, moved tests, taint repair, and existing exclusion path change.
Those decisions replace the initial pure-move scope. This PR moves gate code and makes one approved logic change.
I read every new gate-kit module, every changed Core shell/configuration/document path, both complete bodies and their corrections, and the supplied evidence.
The old Core 72b2e335 tree contains no xtask or gate-kit path. The current base implementations supply the applicable prior art.
The move creates no new check family, Core product algorithm, conformance removal, public Core signature, pin, or retry authority.

### Source and path rules

mutant_logs.rs is byte-identical to its base module. Its status parser, first-failure rule, known-flaky guard, and retry rules remain exact.
tracked_files and walk_files retain their bodies. The latter becomes available outside cfg(test); Core keeps its existing helper names through re-exports.
parse_junit and unescape retain their bodies. Core's test-budget caller uses the same parser through its public library export.
The high_tier import changes to files. Its repository-root test moves up two parents instead of one because the crate is deeper.
Its list path, matching rules, errors, outcome, command, and eight tests remain unchanged.
Core's CLI and CI keep their existing command names through module imports.

Core retains its exact denylist additions, two exception records, singleton FFI manifest, and I/O shells.
The library Policy replaces only the former constant lookups. All other lint/token/source/filter/verdict function bodies retain their rules.
The three policy-taking functions match the old bodies after normalization of that parameter and its lookups.
The real Core-file assertions remain in xtask. The moved rule tests use synthetic paths with the same allowed item identities and adversarial cases.
The tooling clippy restrictions equal the old configuration except its first comment. The library has no production Core-crate dependency.
The manifests reuse workspace dependencies; the lockfile adds only the new package and its dependency edge.

The taint scan visits each UTF-8 character boundary and retains the old before/after ASCII word tests.
For a nonempty token, a matching prefix establishes an in-bounds UTF-8 end boundary. parse_denylist excludes empty tokens.
The finite iterator preserves overlapping matches, token order, and one result per matching token without the old cursor progress risk.
The test-only cursor body equals the exact old implementation after whitespace normalization.
The equivalence test checks all 19531 lines through six characters, including multibyte characters and overlapping/non-word tokens.
The separate overlap test checks ba-a-a. Neither test adds a production branch or alternate gate rule.

caps.rs, ci.rs, lists.rs, public_api.rs, the HIGH-path list, known-flaky data, and conformance files equal the base.
The HIGH list explicitly leaves gate-code classification to risk rule 1. It needs no new rule-5 entry for gate-kit.
Every exclusion string equals the base after normalization of the approved high_tier path move. No exclusion is added or widened.
The .config nextest change is a comment only. Path-based unsafe, taint, file selection, API, and mutation decisions retain their accepted rules.
The whitespace check passes.

### Findings and test preservation

**F161 / R331-I1 — LOW — CLOSED.** The initial body counts 408 -> 382 included fixture text in strings.
The corrected body distinguishes source scopes and runtime tiers. All tracked xtask Rust files have 357 -> 331 source test attributes.
The binary's own xtask/src files have 343 -> 317; separate fixture workspaces retain fourteen attributes and all twenty tracked blobs.
Gate-kit has 31 named tests plus its generated build-root test. Those fixture attributes are not direct tests of the xtask binary.
My first explanation attributed the fourteen attributes to xtask/tests, which does not exist. I corrected that explanation before acceptance.
The package independently identifies xtask/fixtures as their actual location. The final body names that exact scope.

I compare the retained base gate and both submitted gate inventories. All 27 moved default test names remain present in gate-kit.
Only the JUnit module name changes from test_budget to junit. Core adds one real-file test.
Gate-kit adds four named tests and its generated root test. All 27 slow xtask names remain exact.
Runtime xtask counts are default 330 -> 304 and slow 27 -> 27. Gate-kit runs 32 default tests.
The base gate is inventory evidence only, not another train-10 final receipt.

**F162 — MEDIUM — CLOSED.** The first focused record loses its scenario logs and prints empty catching-test fields.
The retained records run supplies all 164 scenarios, positive named catch proof, and actual compile errors, as audited below.
The earlier missing-record limits remain historical. No empty citation receives catch credit.

**F163 / R331-I2 — LOW — CLOSED.** The newly touched design line was 185 columns, contrary to pair-common's 120-column rule.
Commit 57cb242f changes only that document's wrapping. The complete word sequence remains identical; its added lines are 85 and 99 columns.
All Rust, tests, manifests, lists, and CI configuration retain their parent blobs.

**F164 — LOW — CLOSED.** The focused producer's filter misses FLAKY 2/2 and other nextest status forms.
I independently confirm that limit. The body now calls those records FILTERED and rejects a global absence claim for FLAKY or retries.
The positive catch proof uses each recorded final failure and its own earlier failure, rather than absence of omitted statuses.
The complete current body SHA256 is `064555b4b15c1702d775ee8fd6e93db046e7ef0a13353cd993428be10fdab498`.
I read all body corrections at the unchanged applicable heads. Earlier body hashes and the package's intermediate records retain their original scope.

### Focused mutation and reversal evidence

The original 808e90e6 focused log SHA256 is `887d6bf198ad52e583d20dc7174297aff5ef5442993027db94bbc2efa2b96956`.
It reports 166 mutants, three taint cursor TIMEOUTs, and the existing shell miss; its wrapper exits three.
The first 28a29d32 focused log SHA256 is `147a90637dc406caf8160bf408cad32c65dc1b08090c683978bf56fdb2e14766`.
It reports 163 mutants, 155 caught, one missed, seven unviable, and zero timeout; its wrapper exits two.
Its catching-test extraction is empty. The next retained-output attempt also loses its records, as GS reports.
Neither earlier record supplies the later positive named catch proof.

Completed retained record: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-gate-kit-28a29d32-focused-mutants-records.log`.
Verified SHA256: `1ec805762bf6aba6dc256df1baf027b6cb0c48ee92b399760d939cf8a51182c8`.
Its header names exact parent head and base. The path filter includes both renamed files as additions; --no-config tests the mutant's own package.
Its full outcomes JSON has one successful baseline and 163 mutant scenarios. Every outcome has exactly one corresponding retained record.
All identities and outcomes match the earlier focused record. The baseline records all 32 normal PASS tests.
All 155 catches have a first recorded final TRY 2 FAIL whose same test has its own TRY 1 FAIL.
Every named catcher passes that baseline and the normal member gate. A different earlier first-attempt failure supplies no independent catch.
The two recorded SIGTERM lines supply no catch credit. Filtered global FLAKY/retry absence is not established or claimed.
Each caught test phase exits 100. All seven unviable build phases exit 101 with actual E0277 errors.
The missing Default types are State once, Regex twice, StageReport twice, and LineColumn twice. These outcomes supply no catch credit.
The renamed modules contribute thirteen high_tier catches and twenty-four mutant_logs catches.
The sole miss is high_tier command -> Ok(()), exactly the approved existing shell exclusion. Its decision helpers remain caught.
The focused job exits two; its wrapper exits two after 105 seconds. This is expected negative focused evidence, not a green full gate.

Reversal log SHA256: `e556cfec21153ddf1d688ec70d60e389d9f5a286fb3092b2a843217f26ec647e`.
Path: `/Users/jasonconigliari/botster-sessions/gates/botster-core-gs-gate-kit-taint-reversal-7851a63b.log`.
I read the complete scratch diff and both runs. A non-overlapping scan fails the direct overlap test in both runs.
The five-character equivalence test passes the first reversal. The six-character version fails the second on aa-a-a.
The later exact source and member gates supply the restored positive result. The reversal records no numeric restoration result or independent target-writer history.
Local Mac base_merge timeouts retain their unclassified local scope. Reported load and an isolated pass establish no cause or structural diagnosis.

### Exact member gate

Initial 28a29d32 gate SHA256: `27fb00fb67d3db49aa9fc18356511c450b01f60fd3534d7f5a6086f9d5cf806d`.
Its 120 selected mutants are files thirteen, JUnit nine, taint twenty-seven, and unsafe_exception seventy-one.
Its 118 catches match citations and normal PASS. The two renames select no mutant in that rename-aware gate, hence the separate focused proof.

Current gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-gate-kit-57cb242f-pool-20261012-170702-93546.log`.
Verified SHA256: `979d346cc5def815ec09fc6702b740f4a06e349c795e7d9bb8bf728716cae1e9`.
The header names the exact current head and unchanged base. Both concurrency caps are four; the Linux pool runs on msa1.
All ten steps pass. Default records 2109 passes and 412 skips. Slow records 695 passes and 2065 skips.
Every ordinal in both tiers appears exactly once. The current gate retains all 120 parent-gate mutant identities and outcomes.
All 118 unique raw catches match citations and normal PASS. Two outcomes are unviable. No miss or timeout remains.
Stage 2 does not run because Stage 1 misses no mutant. No actual FLAKY or retry result appears in this full gate.
Fuzz selects no changed registered decoder crate; this move adds no decoder algorithm or harness family.
Strict reports 281 passes and zero failures. Minimum is 69/69, including one budget-held ID; all named real-process minimum proofs pass.
All 96 held-full trials pass at seeds zero through 31 for the three budget IDs, with a maximum recorded time of 1.295 seconds.
The retained lists, pending real trials, pin, and evidence debts change no status through this PR.
CI takes 252.0 seconds. The job exits zero after 262 seconds; the wrapper exits zero after 263 seconds.
The original member gate and all focused runs retain their exact parent head. They are not future train execution.

### Disposition

The complete current-head package Round 208 matches this source, body, findings, positive catch proof, filter limits, and exact member gate.
F161, F162, F163, and F164 are closed. Neither review has an open finding, including LOW.
The lead's current ruling makes the train-12 gate the merging gate and requires no member gate.
The supplied current member gate is accepted within its own scope. It does not replace a later train gate.
Applicable current-base/train deltas, combined review, exact merging gate, ONE final train receipt, and equal-tree/remote verification remain due.
Train 11 has now landed under the lead's report; this member still has base 644f632f. No future-base composition is accepted here.
No Stage 2 repository change, new check, ceiling amendment, Mac IN-6 diagnosis, F28 closure, or A24 implementation is accepted.
I ran no build, test, gate, mutation, reversal, reproduction, measurement, pool job, or executable merge-check job.

VERDICT: CLEAN (0 open, including LOW; complete #331 source and supplied exact-head member evidence)


### Matching package acknowledgment

Package message `msg_plugin-w_1791850901_c57d9f` confirms that the reviewer read complete integration Round 1 and the status update.
The reviewer names `65aa602f2e7ffa037690c28976b57f1604d89ff5` and reports that the remote review ref equals that commit.
Both reviews match exact head 57cb242f, tree 9b1b569b, base 644f632f, all four closures, body hash, and member gate hash.
The positive focused catch proof matches. The filtered records establish no global absence of FLAKY or retries.
No new package round or final receipt follows. Current-base/train reviews, the exact merging gate, and lead merge checks remain due.

## Round 2: train-12 member source delta

Exact train head: `7b37e787209e839feb2220b7216d1e417b3838b8`.
Tree: `6adc74c7fcc7573548997fcc83a9280a3260dc3c`.
Train base: `131024f3936898a3106af299e6751553c55bbc82`.
Frozen order: #330 `99209ea598aec0c182f35a1645b2a938d2f7d8cc`, then #331 `57cb242f5d3dda40e524090239222e4824021798`.
Lead assignment: `msg_plugin-w_1791851132_a30e77`.
The #331 insertion follows `848800a3dc44f3b575f4b4ab627d2f022eb13b7f`.
Its unique merge base is `644f632f194f62ad7fe14e66482ee01083706e30`.

I read complete package Round 209 at `abd067494b7ba5f8ee4a48564d43662483133f4b`, verdicts/p3-worker.md.
It is CLEAN on these exact identities and this composition. Complete Round 208 remains the prior member record.
All 22 no-rename own-path added/deleted line sequences survive the insertion exactly.
Eighteen present own blobs equal the frozen member. Both old module paths remain absent.
The only different own blobs are the accepted Cargo.lock and mutation-configuration compositions.
The base move overlaps only Cargo.lock. The preceding member overlaps only .cargo/mutants.toml.

The lockfile retains every base package record except xtask's single added gate-kit edge.
The only new record is the accepted gate-kit package. The host and worker bolero edges remain present.
The parsed mutation configuration equals the preceding insertion after the approved high-tier path replacement.
Both #330 obsolete sign exclusions remain removed. No other configuration value or exclusion changes.
The final CI module equals the train base. Its ordinal_decoder and receipt_decoder registrations remain present.
Core's main module still imports high_tier and mutant_logs under their existing names.
The existing command table and CI calls therefore retain the moved implementations.
The Core unsafe policy retains its two exceptions and FFI entry. The moved pure parser receives that same Policy.
The library has no production Core dependency and adds no new check family.

All ten conformance blobs equal the train base, including deferred TOML.
The final member insertion passes all nine full-row list formulas at its actual merge base.
The public facade snapshot change belongs only to #330. #331 changes no Core public signature.
The contracts .46 pin, HIGH-path list, known-flaky data, budgets, and seed counts remain unchanged from the base.
The source, tests, body corrections, and F161-F164 closures retain Round 1's exact scope.
The filtered focused records still establish no global absence of FLAKY or retries.
The supplied member gate is not execution of this train head.

VERDICT: CLEAN (member source delta; 0 open, including LOW).
The combined review, exact train gate, ONE final receipt, and lead merge checks remain due.
I ran no build, test, gate, mutation, reversal, measurement, pool job, or executable merge-check job.
