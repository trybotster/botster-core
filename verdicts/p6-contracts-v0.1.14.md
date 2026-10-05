# PR #161 latest round: contracts-v0.1.16

VERDICT: CLEAN

Implementation head: `257d2ed31dbb5af06551de463091e110eabcc06d`.
Reviewed delta: `b27fdacfad36149cc194a3e60416a03fd0e134af..257d2ed31dbb5af06551de463091e110eabcc06d`.
Open findings for this pin move: 0, including LOW findings.

## Delta review

The lead retargeted PR #161 to `contracts-v0.1.16` in the shared lead handoff.
The reviewer confirmed GitHub's exact PR head and read the updated PR body, including its Prior art note.

- The tag resolves to `7778b1e13bc0328b145434725934849d786cff4a`.
- Manifest final34 includes accepted Core Amendment 16, candidate 3. Core Amendment 14 remains included.
- R-33 excludes MIME data that libghostty ignores from the decoded size. R-30 through R-32 remain included.
- All six workspace dependencies use the new tag. All nine contracts lockfile entries use its exact commit.
- Only those source entries change in Cargo.lock. No contract crate or BUILD.md changes between the tags.
- The generated Core ledger matches the pinned ledger, byte for byte: 671 IDs, up from 664.
- Exactly seven `conf::a16_1_*` IDs join the ledger and pending list. No existing ID leaves either list.
- The new header gives the lead's reason: `A16: P7 services`.
- Removing only the seven new pending lines and their header restores the previous pending file, byte for byte.
- Both copied contracts status files match the new tag, byte for byte. The Core deferred table remains unchanged.
- The three citation changes name the new tag. No executable Rust statement changes.

A16 changes StopService results and the handling of Lost services. This PR records its seven IDs as pending under P7.
This verdict does not prove A14 or A16 behavior, close other packages' findings, or review the RealCoreHarness scaffold.
All earlier verdict rounds remain below without changes.

## Gate boundary

The reviewer ran no tests, mutation jobs, builds, or gates.
Direct Git and Python comparisons establish the artifact checks above.
The gate remains parked behind the lead's merge order: the P3 guard fix, #162, #163, #142, then #161.
The future v1 merge requires a delta review. Merge requires a green gate on the final exact CLEAN head.
The earlier red gate remains red evidence. This verdict does not close or waive its failures.

---

# PR #161 latest round: contracts-v0.1.15

VERDICT: CLEAN

Implementation head: `b27fdacfad36149cc194a3e60416a03fd0e134af`.
Reviewed delta: `5a34cf84d840c9424ef83b1bb8e37677b4524b0c..b27fdacfad36149cc194a3e60416a03fd0e134af`.
Open findings for this pin move: 0, including LOW findings.

## Delta review

The lead retargeted PR #161 to `contracts-v0.1.15` in the shared lead handoff.
The PR body retains its Prior art note and lists all ten new Core IDs.
The reviewer confirmed GitHub's exact PR head and inspected the complete delta.

- The tag resolves to `69327d52cedb05ee9b9c63912b58d6f62a36917f`.
- Manifest final33 includes HC Amendment 6 and accepted Core Amendment 14, candidate 3.
- All six workspace dependencies use the new tag. All nine contracts lockfile entries use its exact commit.
- Only the contracts source entries change in Cargo.lock. No contract crate changes between the tags.
- BUILD.md and the steward rulings remain unchanged between the tags. R-30 through R-32 remain in the new tag.
- The generated ledger matches the new tag's Core IDs, byte for byte: 664 IDs, up from 654.
- Exactly ten `conf::a14_*` IDs join the ledger. No existing ID leaves the ledger.
- Exactly those ten IDs join the pending list. No existing pending ID changes or leaves the list.
- The header gives the lead's reason: `A14: P3 worker (M2b) + fork binding (option 39)`.
- Both copied contracts status files match the new tag, byte for byte. The Core deferred table remains unchanged.
- The three citation changes name the new tag. No executable Rust statement changes.

A14 requires decoded size and contents size checks, in that order, and a model decode limit equal to `clipboard_bytes`.
This PR records those requirements as pending. It does not implement or prove the A14 behavior.
This verdict preserves all earlier review history. It does not review the RealCoreHarness scaffold.

## Gate boundary

The reviewer ran no tests, mutation jobs, builds, or gates.
Direct Git and Python comparisons establish the artifact checks above.
The supplied gate log for the earlier head `5a34cf8` reports two slow-test failures:

- `a_worker_is_not_left_when_the_cleanup_of_a_test_fails`.
- `the_pty_counts_output_and_delivers_input_to_the_program`.

The log reports mutation and fuzz steps as NOT RUN. It is not a green gate for either head.
The lead parked execution under FULL HOLD and requires the A10 and A31 fixes in v1 before the next gate.
This verdict does not close or waive A10 or A31, or establish their cause.
The future v1 merge needs a delta review. Merge still requires a green gate on the final exact CLEAN head.

---

# P6 contracts-v0.1.14 pin review

VERDICT: CLEAN

PR: https://github.com/trybotster/botster-core/pull/161
Implementation head: `5a34cf84d840c9424ef83b1bb8e37677b4524b0c`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Open findings: 0, including LOW findings.

## Scope and source checks

The lead assigned this pin move to P6 in the shared lead handoff.
The PR contains one commit and changes five files.
The PR body contains the required Prior art note.

- All six workspace dependencies on botster-contracts use `contracts-v0.1.14`.
- The annotated tag resolves to `9a00db8b85bc165ff93ee620db3c54ba3ddeac32`.
- All nine contracts packages in Cargo.lock use that tag and commit.
- The lockfile changes only those source entries. Package versions and dependency lists remain unchanged.
- The contracts diff under `crates/` is empty between the two tags.
- Manifest final31 adds Hub-to-plugin Amendment 8. The tag includes steward rulings R-30, R-31, and R-32.
- The new BUILD.md fork policy matches the policy already binding on this review.
- The checked-in Core ledger equals the sorted Core IDs from the new tag's ledger.json, byte for byte: 654 IDs.
- Both copied contracts status files equal the new tag's files, byte for byte.
- The Core pending list and deferred table remain unchanged.
- The R-30 citations name a tag that contains the ruling.
- The status test changes only its tag citation. No executable Rust statement changes.

This PR adds no terminal behavior, test branch, process cleanup change, or mutation exclusion.
R-31's permitted helper change and R-32's required fork patch remain separate work.
This verdict does not close their behavioral proof requirements or review the RealCoreHarness scaffold.
All earlier verdict files and finding histories remain unchanged.

## Verification and acceptance boundary

The reviewer inspected the exact implementation head and the PR description.
The reviewer compared the pinned artifacts directly with Git and a Python script.
The reviewer ran no tests, mutation jobs, builds, or gates.
The PR reports the Linux gate as pending.
The implementer must supply the gate result on this exact CLEAN head before merge.
