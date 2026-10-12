# Train 7b integration review

## Combined source inspection

- Reviewer: Sol-C.
- Train head: `007efbde6a59af403d0937c38361c137a8961a48`.
- Tree: `6dbc71d0ded09ba4a27bd30e501b0fd61315fa13`.
- Base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Lead assignment: `msg_plugin-w_1791836260_4fe40f`.
- Complete full-index diff SHA256: `7ecd274c9c350c483b5010dd5ab48e2c6025d174e85a7742881b5fdbb0e23029`.

The frozen merge order is:

| Member | Frozen head | Merge |
| --- | --- | --- |
| #316 | `f1ac08b998f15f062f123a996e2688344c33c3ba` | `e9485b94bfc2476be07cb1642ccec4a5437b9952` |
| #323 | `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307` | `cd9af56fd6b0d238ad606aae9fec3ab9b431ebbf` |
| #302 | `953aa2ba300df85bff1df7a8be15f5214b6ca67b` | `abdae8913136fe68f51121d95cf40b15149ced99` |
| #322 | `25fbc9ec7bae580d28e3b29f7f43f0d6e0ff78b2` | `007efbde6a59af403d0937c38361c137a8961a48` |

I read every merge's parents. Each joins the previous train state and the listed frozen member.
The lead reports no conflicts. The complete difference has eight paths, 278 additions, and 39 deletions. Whitespace checks pass.
#318 is absent. The lead retains unpushed train-7 fed2d361 as the earlier failed compilation record.
This review accepts no #318 repair or future train membership.

### Review records read

I read these complete applicable records:

| Member | Package | Integration |
| --- | --- | --- |
| #316 | `9ac22d233b6bfec7f49f07edcd74f8ec490e4ea9`, rc-real-controls.md, train-7b delta | `1b68f648`, rc-pin-46.md, Round 3 |
| #323 | `f0a5525aefc42ee0ae38232fc7d63833c53cd280`, gs-gate-speed.md, corrected #323 section | `f3696d7efa3cd92352b7258f00bb74d966ecbee7`, st-real-pending-reentry.md, complete corrected READY |
| #302 | `715ec92e58205a6530bce7d0f504c5b51bdf6dc7`, p3-worker.md, Round 200 | `56f6db60a152feef89522891d6319d9725bb9915`, p3-route-semantic-input.md, source inspection; completed delta remains due |
| #322 | `9e1880e3c1d423ef28396ed0c836c0ce9c56db2d`, p5-fifo-release.md, Round 2 | `56f6db60a152feef89522891d6319d9725bb9915`, p5-fifo-release.md, Round 2 |

The earlier #302 SLOT records and #322 complete initial records retain their accepted finding closures and evidence limits.
The lead permits #323's carried CLEANs because its path is disjoint and it changes no list data.
The lead's check freeze explicitly permits approved #323. This train adds no check beyond that reviewed member.

### Independent composition checks

All six #316 member blobs equal the frozen member in the final train.
The #323 lists.rs blob equals its frozen member. Its complete inserted additions and deletions match its reviewed patch.
The #322 inserted additions and deletions also match its reviewed patch. Its complete fixed cancel test remains exact.
The only additional session source is the base's existing real PTY resize test. Its temporary root, FIFOs, and session are separate.
The #302 insertion has no difference. Its preceding and resulting trees both equal `3a73d15508bed6bb3546a6e660157a1fb2d67e9e`.
Its accepted worker changes already exist through #311 in the base. The insertion adds ancestry and changes no source or list.
The final worker input, route, and route-test files equal the train base. Resize/Latest and report-order additions remain present.

#302 has two merge bases: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c` and `78d7057fca48480e2e8e3174baa2e5c2ab7a70c3`.
I check all nine conformance text sets against each candidate. I check the same formulas after every other merge.
All 45 comparisons pass. No removed ID returns, and no member addition disappears.
Final counts are ledger 692, pending 388, real-pending 38, proofs seven, minimum 69, held four, real-only 26, deferred three, and withdrawn 27.
Only #316's a22_1 ID joins ledger and pending relative to the train base. Real-pending stays identical to the base.

The old #302 comment says route resize is refused. #311 implements that operation, so the comment is obsolete.
The base and final core-pending file contain neither that comment nor the dp_3 route-codec ID.
The ID remains in core-real-pending. Dropping the pending comment restores no refusal and grants no real-tier removal.
#328's later removal evidence is separate and does not alter this frozen train.

### Pin and list interfaces

The .46 pin retains seven direct dependencies and nine lock records at `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1`.
The imported Core constructs no RouteAdopted. Its field-independent queue matches remain valid with A22's optional tag and owner.
#302's semantic-input types and route codec do not change across .44 to .46. The existing input bounds and frame charges remain.
#322's cancellation outcomes, byte counts, and cleanup interfaces also remain unchanged across those pins.
No new Core production code or control enters this train difference.

The lists command reads the ledger and replacement map from the selected .46 contracts source.
It parses real-pending IDs and re-entry tags from the same text. The new tag branch changes only the entry rule against the base.
It retains the ledger, pending, deferred, withdrawn, real-only, and testkit-proven membership refusals.
The real-pending file has no re-entry tag. A22_1 remains pending and outside real-pending, so the branch creates no exemption here.
The tag is a comment. Existing conformance, CI, coverage, and merge readers retain their ID interpretation.
Future re-entry still needs review of its actual authority, cause, and fix owner. #298 is absent.

The EV-2 entry and its structured signature leave together because .46 carries its title fence.
IN-8 remains the sole entry, with owner PC, removal #321, and the exact slow/step_timeout/any-seed/step-6 text signature.
The parser and mutation filters equal the accepted #308 implementation. The pin does not carry PC's R-111 fix.
Normal slow and unmutated baseline failures remain RED. This source review grants no retry.
All mutation configuration and exclusions equal the base. No new private subject-state assertion or terminal model appears.

#322 retains the count FIFO until the done marker and received-byte equality prove consumption.
The newline reader requires no EOF. Release follows those observations; Kill, exit, Remove, and guard cleanup retain their order.
The separate resize test changes no shared cancel fixture. The .46 pin changes no test-process fixture.

### Evidence limits and remaining receipt

The lead reports successful list checks, locked workspace test-target compilation, and strict seeds 0–31 at 281/0.
The reported clean time is 2026-10-12T20:14:04Z. No retained pre-check log is supplied; these remain operator reports.
Historical member gates retain their original heads and scopes. No completed exact-train-head gate is supplied here.
No composition finding is identified. The completed #302 integration delta remains due before formal combined CLEAN.
After source CLEAN, the lead runs one train gate. I then review its log and send the one whole-train final receipt under BUILD Green.
Equal-tree fast-forward and remote push verification remain the lead's duty. I grant no gate, retry, future head, or merge acceptance here.
I ran no build, test, gate, mutation, reversal, measurement, or pool job.

No formal combined verdict is issued in this inspection.
