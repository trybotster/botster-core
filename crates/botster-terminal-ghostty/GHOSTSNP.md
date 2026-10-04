# GHOSTSNP format for Core Stage 1

This document specifies the existing format at fork `3f8eb6810bb673aa782b047de21783ac81fb1121`.
The binding uses the fork's encoder and decoder. It adds no terminal parser.
Core ST-6 requires this spec. Core ST-6b and A8-2 define the restore and refusal rules.
Steward ruling R-30 defines the independent size measurement for every-cut tests.

## Name and version

The name is `GHOSTSNP`. The version is `1`.
The binding reads both values from a library-generated snapshot in [`snapshot_format`](src/snapshot.rs).
The native [`envelope.zig`](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/envelope.zig) defines both values.
The fork marks version 1 as a work in progress without a binary compatibility guarantee.
See [`snapshot.h`](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/include/ghostty/vt/snapshot.h).

## Envelope and records

All integers are unsigned and little-endian.
The envelope occurs once, at byte zero.
Its total size is 10 bytes.
These values come from `envelope.zig` and `snapshot.h`, linked above.

| Offset | Size in bytes | Field |
|---|---|---|
| 0 | 8 | Magic: all eight bytes of `GHOSTSNP` |
| 8 | 2 | Version: `u16` |

Each record has a fixed header followed by its payload.
The header size is 10 bytes.
The CRC32C covers the encoded tag, the payload length, and the payload. It excludes the CRC field.
These values come from [`record.zig`](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/record.zig).

| Offset | Size in bytes | Field |
|---|---|---|
| 0 | 2 | Tag: `u16` |
| 2 | 4 | Payload length: `u32` |
| 6 | 4 | CRC32C: `u32` |
| 10 | The declared payload length | Payload |

The encoder emits the following record groups in order.
Tag values come from `record.zig`.
Order comes from [`snapshot.zig::encode`](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/snapshot.zig).
The linked codec for each payload defines its complete field layout.

| Group | Tag | Payload and sequence |
|---|---|---|
| TERMINAL | 1 | Terminal state and declared screens. See [terminal.zig](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/terminal.zig). |
| SCREEN | 2 | One active-area manifest per declared screen. See [screen.zig](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/screen.zig). |
| PAGE | 3 | The pages declared by each SCREEN manifest. Contents include cells, attributes, and hyperlinks. See [page.zig](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/page.zig). |
| CONTINUATION | 7 | Canonical unfinished parser input. An empty payload represents ground state. See [continuation.zig](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/continuation.zig). |
| READY | 5 | Empty marker. The terminal can render and resume at this point. See [checkpoint.zig](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/checkpoint.zig). |
| HISTORY | 4 | One history manifest per declared screen, followed by its PAGE records, newest to oldest. See [history.zig](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/history.zig). |
| FINISH | 6 | Empty marker. The complete snapshot ends here. See `checkpoint.zig`. |

The decoder requires the declared counts, tags, CRCs, and group order.
It rejects EOF before the required READY or FINISH marker as malformed data.
Trailing transport bytes after FINISH are outside the snapshot.
See `snapshot.h` and [`snapshot/main.zig`](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/snapshot/main.zig).

## Per-capture fields

The native format has no per-capture identifier, timestamp, nonce, or variable capture metadata.
The largest additional size of those fields is therefore 0 bytes.
The encoder receives only the terminal and continuation as state inputs.
It writes the envelope and records listed above, then FINISH.
See `snapshot.zig::EncodeOptions` and `snapshot.zig::encode`.
The C adapter obtains the terminal's continuation and calls that encoder without adding fields.
See [`c/snapshot.zig::encode`, `encode_buf`, and `encode_alloc`](https://github.com/trybotster/ghostty/blob/3f8eb6810bb673aa782b047de21783ac81fb1121/src/terminal/c/snapshot.zig).

Record headers, counts, payloads, and CRCs are part of the native encoded length.
An independent native encoding already counts those bytes.
The fit calculation must not add them a second time.
Allocation hints in PAGE are terminal page state, not per-capture metadata. See `page.zig`.

## Continuation limit and refusal

The Stage 1 binding names the continuation limit `CONTINUATION_LIMIT`.
Its value is 1,048,576 bytes, or 1 MiB.
This is the largest pending sequence state that the binding's snapshot format carries.
It is a format property, not a `CoreLimits` value.
See [`snapshot.rs::CONTINUATION_LIMIT`, `decode`, and `Terminal::from_snapshot`](src/snapshot.rs).

At ground state, the continuation is empty.
When pending input exceeds the limit or retention fails, the binding returns `SnapshotError::ContinuationUnavailable` without snapshot bytes.
Core maps that result to `SnapshotTooLarge` under A8-2.
The native C encoder checks continuation availability before it writes the envelope.
See `c/snapshot.zig::continuationOwned` and `encode`, and `snapshot.zig::encode`.
The binding never returns a partial snapshot. See `snapshot.rs::Terminal::snapshot`.

The native envelope decoder refuses an unknown version before it reads records.
The C decoder maps that refusal to `GHOSTTY_INVALID_VALUE`.
The binding reports `SnapshotDecodeError::UnsupportedVersion { version }` when the envelope has the native magic and an unsupported version.
See `envelope.zig::decode`, `c/snapshot.zig::decoderMapError`, and `snapshot.rs::decode_error`.
Other malformed input remains a typed `SnapshotDecodeError::Library` error.

## Graphics exclusion

The snapshot contains no Kitty image or placement state and no image storage limit.
The model constructor and snapshot decoder set the image storage limit to zero before input or continuation replay.
Libghostty enforces this limit on both screens.
See `snapshot.zig::DecodeOptions.kitty_image_storage_limit`, [`Terminal::new`](src/lib.rs), and `snapshot.rs::decode`.
The binding's `snapshot_format` documentation states the graphics exclusion.
The worker must report `snapshot_graphics` absent while it uses this model configuration (ST-6b).

## Core capture pages

The host keeps the worker's `Page` values unchanged.
It counts only the sum of `page.bytes.0.len()` against `max_snapshot_bytes`.
The host's page metadata therefore adds 0 counted bytes.
The host creates the `CaptureId`, expiry, and capture summary outside the stored page bytes.
See [`inbound::take_capture`](../botster-core-host/src/inbound.rs).
The host returns each stored page unchanged. See [`Engine::read_page`](../botster-core-host/src/engine.rs).
These facts do not define how the worker forms the page bytes.

## Worker paging: defined by the PR that implements CaptureSnapshot (P3 M2b); pending

The current [`botster-worker-core`](../botster-worker-core/src/lib.rs) has no capture producer.
No worker segmentation, additional page framing, or per-capture byte fields are specified here yet.
The P3 M2b PR must complete this section with citations to its paging code.
The every-cut fit check stays inconclusive until this section is complete.
It must not assume that host metadata accounting proves the absence of worker framing.
