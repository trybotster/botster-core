// The client's view of a binding (design 6.6, HC TB-0): send a frame, receive frames, progress, close.
//
// Handwritten. A binding defines frame delimiting, peer authentication, progress and the close signal (TB-0 items 1 to 4).
// The transport hides all four: a caller sends and receives decoded frames. Authentication happens before the first frame (HS-7),
// delimiting is the binding's job (TR-1, TR-3), and a close arrives as an event, never as an exception.
// A client core implements an adapter per binding against this interface. This package never depends on a client's own interfaces.
import type { ClientFrame, CloseReason, ServerFrame } from "./generated/hub.js";

/** Why a transport ended. `close_frame` carries the Hub's typed reason (HS-9). `eof` is a bare end of stream: the cause is unknown (TS-9). */
export type TransportEnd =
  | { readonly cause: "close_frame"; readonly reason: CloseReason }
  | { readonly cause: "eof" }
  | { readonly cause: "transport_error"; readonly error: unknown };

/** What a pull-receive resolves with: the next frame in order, or the end of the transport (TB-0 items 1 and 4). */
export type TransportEvent<R> =
  | { readonly kind: "frame"; readonly frame: R }
  | { readonly kind: "closed"; readonly end: TransportEnd };

/**
 * How much the binding has accepted of what this side sent (TB-0 item 3).
 * It is monotone. A readiness report that takes no bytes is not progress (TB-L4, TB-W4).
 */
export interface TransportProgress {
  readonly acceptedBytes: number;
}

/**
 * A transport that sends `S` frames and receives `R` frames. Frames of one transport arrive in order (WT-2, TB-0 item 1).
 * A route's transport is the same shape with route frames for `S` and `R`.
 */
export interface FrameTransport<S, R> {
  /** The binding's name, an open string (TB-0). */
  readonly binding: string;
  /** Sends one frame. It resolves when the binding has taken the frame for delivery, never when the peer has processed it. */
  send(frame: S): Promise<void>;
  /** Pull-receive: resolves with the next event. After the end, every call resolves with the same `closed` event. There is no polling and no timer (P8). */
  receive(): Promise<TransportEvent<R>>;
  /** Backpressure progress (TS-7, TB-0 item 3). */
  progress(): TransportProgress;
  /** Closes the transport. It resolves after the close is complete. A second call has no effect. */
  close(): Promise<void>;
}

/** The control connection of a client to a Hub: it sends `hello`, `req` and `cancel`, and receives `welcome`, `res`, `watch`, `event` and `close` (EN-1, EN-2). */
export type HubTransport = FrameTransport<ClientFrame, ServerFrame>;
