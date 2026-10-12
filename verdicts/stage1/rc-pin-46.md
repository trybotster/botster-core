# Core #316: contracts .46 pin

## Round 1: source and initial evidence

- Reviewer: Sol-C.
- Branch: `stage1/rc-pin-46`.
- Head: `0844f9dfadee637197d1bbbfc4ea5dcd7c951325`.
- Tree: `e6e94ec9c9294fc10a21ac23252ba386c759598a`.
- Source and gate base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Reviewed delta: complete six-file diff, 37 additions and 24 deletions; dependency pin, ledger, pending entry, and known-flaky replacement.
- Lead assignment: `msg_plugin-w_1791831854_ba5eeb`.
- Package receipt: `dca58cff40d50021d3a50484a7e02bca33bbec94`, complete #316 round in `verdicts/rc-real-controls.md`.

I read the complete same-head package verdict. It closes RC-PIN46-1 and retains zero findings, including LOW findings.
I read the complete corrected body. Its exact UTF-8 SHA256 is `86f7dc94df55c4a413dbd70e455a08364af3d425f12e565e4b5a8ade66d3a358`.
Its strict checks identify c34bfe9f as the earlier check head. The exact READY head remains 0844f9df.

Seven direct dependencies and nine lock records move from .44 to .46. No other lock record changes.
The .46 tag resolves to `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1`, with tag object `b82ff81fa5909064281784d8a9e10acf73936251`.
Manifest final41 freezes A22. A23, A24, and A25 remain candidates.
The pin changes no control, probe source, route codec, or test-support source. It does not import the unserved pty_output_written control.
BUILD.md and shared job-common.sh retain their .44 content.

A22 changes RouteAdopted.route_tag and owner to Option<String>. None serializes as absent; Some(empty) retains the empty string.
Core constructs no RouteAdopted at this base. Its queue uses field-independent matches, so this signature change needs no Core source repair here.
The ledger adds only a22_1. Only that ID joins pending; #313 owns construction, optional fields, and its later removal.
If #313 lands first, this pin must adapt the imported construction. If this pin lands first, #313 must adapt it.
Neither merge order receives advance acceptance.

The driver clones bindings only for pty_input reads. Input-log retention and let propagation keep their previous order outside that conditional branch.
The package review covers the changed active transcripts, order lint, schemas, and new transcript. Those closed scopes remain applicable.
I independently check the exception sets: fifteen IDs lose their final exception, with zero overlap with real-pending.
Eleven real-pending IDs lack exceptions both before and after this pin. No real-pending entry is removed.
The real-pending and real-proof files, mutation configuration, and unrelated conformance files retain the accepted base content.
The package record retains 692 ledger IDs, 404 pending IDs, 34 real-pending IDs, and minimum 69.

R107-SI-2 requires removal of the EV-2 entry with .45 or later. The .46 tag contains the await_title fence before Stop.
The final known-flaky list contains only IN-8. I check its actual RED signature: seed zero, step 6, limit step_timeout.
R-111 authorizes that named test at any seed, with owner PC. The entry records that exact signature and names removal PR #321.
I read OPEN draft #321 at `5a4dd4d9021974f6df8fa4344442993dfb09e12c`, branch `stage1/rc-pin-r111`.
Its plan pins PC's structural driver fix after the steward tags it, then removes this entry.
The .46 pin does not contain that fix. Its removal trigger is not satisfied.
The repository-list test requires exactly IN-8. Production parsing and mutation filtering remain unchanged.
Normal slow and unmutated baseline failures remain RED. Stage 1 exit still requires an empty list.
#308's future structured signature table requires a composition review. This verdict accepts no future parser or list composition.

Initial gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-pin-46-0844f9df-pool-20261012-115709-55550.log`.
Verified SHA256: `e9bbd089dc393c27718d1c3e7928e16d7157f4408f3c63ecf2eb3b6052792cfb`.
The header names this exact head and base. Both parallelism caps are four. All ten steps pass.
Default tests pass 1957 cases; slow tests pass 633 cases. EV-2 and IN-8 pass normally on both tiers.
Strict checks 317 of 334 pending transcripts; seventeen real-only transcripts do not run there. Four held IDs retain their rules.
Three budget IDs run at all 32 seeds. Twenty-six real-pending passes authorize no new removal in this pin.
The mutation step selects no mutant. Fuzz selects no changed decoder harness. These scopes supply no behavioral proof of absent execution.
CI takes 146.7 seconds. Job exits zero after 314 seconds; wrapper exits zero after 315 seconds on msa1.
The default seed set is 0–31; slow uses seed zero. No full-seed real-tier claim follows.

The earlier c34bfe9f gate fails IN-8 at the recorded step-6 signature. Its first seven steps pass; mutants and fuzz do not run.
That historical RED remains. R-111 authorizes the new entry and one fresh gate; this verdict grants no additional retry.
I read strict.sh and the retained c34 strict log: 264 passes, zero failures, seeds 0–31, and exit zero.
Its quiet clean marker supplies no separate numeric clean exit. The exact earlier head and tree are recorded.
The contracts fence-lint log names the .46 commit and reports 99 passes, zero failures, and exit zero.
These local records retain their separate worktree and clean-history limits. The pool gate uses its own tree.

No integration finding remains. The package's body correction is verified at the same exact source head.
Applicable train delta and combined reviews, final gate receipts, and equal-tree merge verification remain due.
This source CLEAN accepts no later head, #313/#308 composition, PC driver fix, future control, or final train result.
I ran no build, test, gate, measurement, mutation run, or pool job.

VERDICT: CLEAN (0 open, including LOW)


### Matching package acknowledgment

I read the matching package record at `e1de55b4dc26776ef2e7b607d0f1f71eca9173a0`.
Message `msg_plugin-w_1791832508_a82180` confirms that the package reviewer read complete integration Round 1 at `adda76b49f727bb88db1e59949968586b57e1edb`.
Both reviews match the exact head, base, tree, body correction, initial gate, and remaining scope conditions.
No new review round or final acceptance follows. Applicable train reviews, final receipts, and merge verification remain due.


### Import delta in progress: f1ac08b9

Owner READY `msg_plugin-w_1791835547_aca25b` supplies head `f1ac08b998f15f062f123a996e2688344c33c3ba`.
Its tree is `76fab1bfcbc23fafbd358964620a6929795f97bf`. It imports v1 `3e655eff46d36f8c337339a1a17d401c8c662594`.
The merge base is `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`. Only the known-flaky list and repository-list test have conflict repairs.
I read those repairs. The pin stays identical; the structured IN-8 signature preserves the recorded step-6 timeout.
The parser and all other tests equal the accepted base. All nine conformance formulas match.
No current real-pending ID loses its final exception from .44 to .46.

Package message `msg_plugin-w_1791835641_f583f4` identifies RC-PIN46-2 LOW in the body.
I independently confirm its count: eleven real-pending IDs lack exceptions at 0844f9df; twelve lack exceptions at f1ac08b9.
The additional ID is `conf::dp_3_route_codec_matches_client_appendix_a4_a5` from the accepted base import.
Checks item 3 must scope eleven to the earlier head or state the current count. No source change or extra execution is required.
The exact-head package closure and corrected complete body remain due. The integration import round remains in progress.
No formal replacement verdict or final gate acceptance follows from these source checks. I ran no execution job.


## Round 2: train-7 SLOT import delta

- Head: `f1ac08b998f15f062f123a996e2688344c33c3ba`.
- Tree: `76fab1bfcbc23fafbd358964620a6929795f97bf`.
- Imported source base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Previous member: `0844f9dfadee637197d1bbbfc4ea5dcd7c951325`.
- Three-way merge base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- READY: `msg_plugin-w_1791835547_aca25b`.
- Package: `723c6ec226ffd32b347bd4ca39f5f98062e1aa52`, complete train-7 SLOT import section in `verdicts/rc-real-controls.md`.
- Reviewed delta: the complete six-file difference from the imported base and both known-flaky conflict resolutions.

I read the complete same-head package verdict and corrected body. Both retain zero open findings, including LOW findings.
The merge parents match the previous member and imported base. The owner reports rerere off and two conflicts.
The manifest and lock equal the previously reviewed .46 pin byte for byte. No additional dependency revision appears.
The ledger and pending files add only a22_1 relative to the imported base. Core still constructs no RouteAdopted.
The imported frame-fit and resize implementations change no RouteAdopted field use. #313's future composition remains unaccepted.

The known-flaky resolution retains #308's structured-signature header and removes EV-2 with its signature table.
The sole remaining entry is IN-8, owned by PC and removed by #321.
Its signature names slow, step_timeout, any seed, and literal text `step=6 limit=step_timeout`.
That signature matches R-111 and the retained c34 RED. It includes no step-8 failure or additional symptom.
The repository-list test uses the exact binary, test, and removal-PR tuple. It expects only IN-8 and #321.
The parser, other tests, and mutation filters equal the imported base. No production check behavior changes in this resolution.
Normal slow and unmutated baseline failures remain RED. Stage 1 exit still requires an empty list. No retry is authorized here.

All nine three-way conformance formulas match. Counts are ledger 692, pending 388, real-pending 38, proofs seven, and minimum 69.
Four held IDs, 26 real-only IDs, three deferred entries, and 27 withdrawn entries retain the merged sets.
The import preserves sixteen pending removals and four real-pending additions. Their accepted owner reviews retain their scopes.
No real-pending ID loses its final exception across .44 to .46. No new pending removal follows from this pin.
The mutation configuration equals the imported base. No exclusion is added.

I read R-114 at contracts `e22baecc1be444c96c3b7c099162eeb53b605164` and the current R-115 freeze.
The separate oracle_resume fence work does not enter this pin. The delta adds no check family or lint.
R-111's structural driver fix remains outside .46; #321's removal trigger remains unsatisfied.

I read strict-slot.sh and its exact-head retained log without executing them.
The log names this head, tree, and imported base, with no tracked difference. Both parallelism caps are four.
The script unsets RUSTUP_TOOLCHAIN and runs quiet cargo clean. The marker supplies no separate numeric clean exit or removal count.
Fmt, all-feature workspace test-target check, lists, ledger, six known-flaky tests, and prebuild exit zero.
Strict seeds 0–31 pass 281 cases, with zero failures and exit zero. Seventeen real-only pending transcripts do not run there.
The four held IDs keep their rules. These local records establish no full-seed real-tier result or completed train gate.
Historical gates retain their original heads and bases. The lead uses train 7's gate as the merge gate.

**RC-PIN46-2 — LOW — CLOSED.** The corrected body gives eleven no-exception IDs at 0844f9df and twelve at f1ac08b9.
The added ID is the imported dp_3 route-codec transcript. It already lacks an exception at .44.
I read the complete corrected body and verify exact UTF-8 SHA256 `1e730f0684003f90945e0b382babad88f9c47a955aaf05ebc419d9757d0aa687`.
The source head remains fixed. The earlier finding and its closure remain recorded.

No source or body finding remains. Applicable train and combined reviews, the exact-head train gate, and lead merge verification remain due.
The combined reviewer sends the single final train receipt for an unchanged member. I supply this source delta only.
This verdict accepts no later head, future #313 composition, PC fix, new control, specific retry, or final gate result.
I ran no build, test, gate, measurement, mutation run, or pool job.

VERDICT: CLEAN (0 open, including LOW; import source delta only)
