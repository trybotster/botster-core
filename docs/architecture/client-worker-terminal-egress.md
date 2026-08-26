# ClientWorker terminal egress and subscription teardown

Core ClientWorker is the sole terminal egress and teardown authority for
**adapter-bound** subscriptions.

This is an advanced host/adapter seam. The embedder start-here path remains
spawn → attach → drain → input → shutdown. Bind is not part of that prelude.

## Ownership

Core owns:

- Subscription identity `(session_id, subscription_id, generation)`
- The only subscription egress queue and slow-client policy
- Incremental attach phases (READY / PAGE / FINISH) and input/resize barriers
- Mechanical detach when an adapter returns `Closed`
- Control-plane inventory without terminal state

Hub owns host session policy: spawn, worktrees, retention, and cleanup after
process exit. ProcessExited closes terminal subscriptions. It does not shut
down the host session.

## Bind sequence

1. A host that will bind an adapter calls `expect_terminal_adapter` with the
   exact `client_id`, `session_id`, and `subscription_id` before `attach`.
   `cancel_expected_terminal_adapter` retires an unconsumed declaration. The
   declaration is not a reservation generation.
2. `attach` assigns a monotonic generation and publishes an inventory row with
   `adapter_bound=false` and `capabilities=None`. A matching declaration sets
   `hold_until_bound` on the new owner. `AttachedSession.client_egress` is then
   empty for that route.
3. While `hold_until_bound` is set and no adapter is bound, ClientWorker keeps
   terminal frames in a per-owner hold. Capacity is
   `QueueSource::ClientWorker.default_capacity()` (512). Overflow hard-stops
   that owner. `AttachState { Detached }` still reaches the host. `ProcessExit`
   is held and does not hard-stop before bind. Unbound owners without a
   declaration keep today's `TransportEgress` drain path, including the
   unbound `ProcessExit` hard-stop.
4. `bind_terminal_adapter` must present that live generation and a required
   `TerminalCapabilitySet`. Omission does not compile. An empty set is valid.
   Pre-attach bind is a typed error. There is no reservation generation. A
   second bind of the live generation returns `AlreadyBound` and does not
   replace the adapter or the set. Bind only installs the adapter. It does
   not flush.
5. The next host drain tick flushes the hold through `encode_terminal_frame`
   into `owner.queue`, then `pump` writes through
   `TerminalAdapter::try_write`. Held frames precede live frames from the same
   tick. Encode failure or defensive queue overflow hard-stops through
   `ingest_bound_terminal_frames` and the production `UnsubscribeSession`
   path. Snapshot frames encode only when the stored set contains
   `snapshot_delivery=ready_then_history`. A held Snapshot is dropped, not
   upgraded, when the set omits that token. Live `TerminalOutput` and
   `Scrollback` still encode for an empty set.
6. After bind, that route's terminal frames leave only through
   `TerminalAdapter::try_write`. `drain` / `drain_subscription` do not also
   return those terminal frames.

## Queue and retry

ClientWorker queue capacity is `QueueSource::ClientWorker.default_capacity()`
(512). A new frame that would exceed 512 fails the subscription. The same head
frame fails after 512 unsuccessful `try_write` results (`WouldBlock` or
`Full`), or 512 host ticks where an accepted write stays non-Ready, on host
pump ticks. Terminal frames never retry. Recovery is detach plus a fresh attach.

A lost READY / PAGE / FINISH / other snapshot frame fails that subscription.
There is no terminal-frame replay helper.

## Delivery and ProcessExited

`Ok(())` occupies the adapter's one-slot write. A frame is delivered only after
that active write completes (pressure returns to Ready, or an auto-completing
adapter is already Ready).

On session `ProcessExited`, each live subscription:

1. Delivers remaining output
2. Delivers `process_exit`
3. Then Core hard-stops: `close()` and drop on the host tick where
   `process_exit` completed (pressure returned to `Ready`)

Close stays non-blocking. A one-slot adapter cannot accept a second frame
until the first write completes, so live bytes complete before `process_exit`
occupies the slot. After process exit, `ReadScreen` still pumps bound
adapters so a one-slot adapter can complete the accepted write and accept
`process_exit`. Shutdown teardown still closes. If the 512 write budget
expires or the adapter returns `Closed` first, Core fails that subscription
without claiming `process_exit` was delivered.

Worker-backed incremental attach polls snapshot frames and does not replace
`drain_output`. While attach is unfinished and a bound adapter is present,
each host drain also pulls live PTY bytes and `ProcessExited`. Unbound
attach keeps one snapshot frame per host tick and still suppresses duplicate
`TerminalOutput`. If the worker stops at `ProcessExited` before snapshot
`FINISH`, Core ends the incremental attach so later ticks use the normal
drain path.

## Close hard stop

There is no ClientWorker OS thread. The hard stop is ownership teardown, then
a contractually non-blocking `close()`, then drop of the adapter on the same
host tick. `close()` and `Drop` must not wait for transport I/O. Core does not
spawn a closer thread. A blocking `close()` is an adapter defect and fails the
published conformance harness.

## Inventory

`list_terminal_subscriptions()` reports `client_id`, `session_id`,
`subscription_id`, `generation`, `adapter_bound`, and `capabilities`. Unbound
rows report `capabilities=None`. Bound rows report `Some` of the stored set,
including `Some` empty. Inventory does not report attach phases, snapshot
bytes, queue contents, or decoder state. The set is protocol tokens only.
Core does not store host grants.

## Production path

`CoreDaemon::bind_terminal_adapter` plus the existing host tick
(`CoreDaemon::drain` / `drain_runtime_once`) pumps ClientWorker. Unix and
WebRTC adapters are later Hub tickets.
