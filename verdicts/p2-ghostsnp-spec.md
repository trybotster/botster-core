# P2 GHOSTSNP specification review

VERDICT: CLEAN

Reviewed head: `d8b84543ab444f2cd04799eb9de6c9c7a7734d0c`.
Branch: `stage1/p2-ghostsnp-spec`.
Reviewed delta: `38bbe580013d02175636395a28a4c359b20c32b2..d8b84543ab444f2cd04799eb9de6c9c7a7734d0c`.
Open findings: 0, including LOW findings.

## Scope and source checks

The lead authorized this separate documentation review under Core ST-6, A8-2, and steward ruling R-30.
The delta adds `crates/botster-terminal-ghostty/GHOSTSNP.md` and a link in `snapshot_format` documentation.
It changes no executable code, tests, mutation exclusions, or fork source.

- `envelope.zig` at fork `3f8eb6810bb673aa782b047de21783ac81fb1121` defines the magic, version 1, and 10-byte envelope.
- `record.zig` defines the 10-byte record header, tag values, unsigned little-endian fields, and CRC32C coverage.
- `snapshot.zig::encode` confirms the listed group order. The screen and history codecs define their page sequences.
- `continuation.zig` confirms empty continuation at ground and canonical replay input for pending state.
- The binding defines the 1,048,576-byte continuation limit. The spec states that value and cites the binding.
- The native decoder and binding support the stated unsupported-version classification and refusal behavior.
- The native encoder has no capture identifier, timestamp, nonce, or capture metadata. Its additional per-capture field size is zero.
- The native encoded length already includes its records and headers. The spec prevents double counting.
- The cited native options and binding constructors support the graphics exclusion and zero image storage limit.
- Host `take_capture` counts stored page bytes. Host `read_page` returns the stored page unchanged.
- Host capture identity, expiry, and summary do not add bytes to that count. These facts do not establish worker framing.

## Explicit pending boundary

The lead allowed worker paging to remain pending until P3 M2b supplies the capture producer.
The spec states that boundary and requires source citations when P3 completes it.
It invents no segmentation, page framing, or worker metadata size.

Under R-30, the every-cut fit check remains inconclusive until worker framing is specified.
This CLEAN verdict approves the stated native format and pending boundary.
It does not approve a complete capture paging specification or a passing every-offset conformance result.

## Verification

The reviewer checked the cited pinned source and host source without running tests or a gate.
The implementer reports that formatting and `git diff --check` pass.
The implementer must run the lead's requested full gate on the exact CLEAN head before merge.
Any later commit requires review of its delta.

The reviewer kept `verdicts/p6-testkit.md` unchanged and left the pre-existing `.gitignore` edit unstaged.
