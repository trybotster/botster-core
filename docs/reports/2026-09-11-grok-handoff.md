# Grok implementation handoff

Jason requested Grok implementation, Claude detailed review, and limited Astra integration review.
Root paused the previous Codex process after its model-service wait prevented the requested handoff.
This commit preserves incomplete and unbuilt D1(a) work. It does not establish acceptance.

Read `/Users/jasonconigliari/Projects/botster-hub/docs/plans/2026-09-07-foundation-final-acceptance.md`.
Read `/private/tmp/claude-callback-review.OFb8pb/handoff-remaining-blockers.md` and its Core review references.
Root coordinates through Botster session `sess-1788561261-002e-6e11191cb68e3da8e22b8f8cbf0c82d0`.
Claude reviews through session `sess-1789062435-00a8-e8f5a8314c8baff1f0517bc0d4d91e05`.

## Work remaining

- Complete generic prelaunch reservation, reserved launch, definitive release, and exclusion across spawn and adoption paths.
- Review and fix the recorded B1/B2/P1-P4 admission findings against current source.
- Bound records through the existing MAX_PENDING_SPAWNS capacity and lifecycle retirement.
- Preserve external runtime compatibility, session ID reuse, and generation identity.
- Treat ProcessExited as direct-child exit, not confirmed group cleanup.
- The optional captured positive PGID in welcome recovery_identity is approved. Bind it to the validated worker and reservation generation.
- A parent check after ProcessExited may confirm group absence only on ESRCH. Unknown or invalid identity remains unconfirmed.
- An existing explicit release may check again. Do not add polling, timers, owner waits, or permanent operator-only recovery policy.
- Keep the proposed SPF1 startup-failure protocol unapproved until concrete contract review.
- The SPF1 proposal carries request_id, session_id, worker_pid, message, and NotCreated or Created with optional child/group IDs.
- The proposal requires identity validation and old-reader rejection as BadMagic. Worker exit or stderr alone is not non-creation evidence.
- Trace post-child setup and Ghostty failures separately from non-creation failures.
- Implement and test within Core only. Root owns Hub integration. Do not publish or install.

The preceding accepted Core checkpoint is `6a9320393b6407a33230816508c9a7e700bfaee6`.
Its worker metadata evidence does not verify this D1(a) implementation.
Use Rust 1.97.0, at most two Cargo jobs, and no incremental compilation.
Coordinate compiler use with Root and the Hub writer.
