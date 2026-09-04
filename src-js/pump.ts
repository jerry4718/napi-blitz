// Pump-loop driver for a `BlitzApp`.
//
// `BlitzApp` itself lives entirely on the native side; the async pump loop
// (timed cadence anchored to `performance.now()`, aborts, the `pump:*`
// event stream) is the one piece that depends on the JS event loop, so it
// stays here as a top-level function.

import type {BlitzApp, PumpResult} from "./native";
import {Event} from "./native";

/**
 * Options for `startPumpLoop`. All durations are in milliseconds.
 */
export interface PumpOptions {
  /**
   * Target cadence: the nominal interval between two pump iterations.
   * Defaults to `16.67` (~60fps).
   */
  targetPeriod?: number;
  /**
   * How long a single pump may block waiting for events before returning.
   * Defaults to `targetPeriod`.
   */
  timeout?: number;
  /**
   * Optional stop signal. When aborted, the loop exits after the current
   * pump returns and `pump:end` fires with `end.kind === "abort"`, the abort
   * reason carried on the `PumpEnd`. The handle-side counterpart
   * `PumpHandle.stop()` reports `end.kind === "stop"`.
   */
  signal?: AbortSignal;
}

/**
 * Carried on the internal controller's abort reason by `stop()`, so
 * the loop can tell a stop request from an external abort.
 */
class PumpStopRequest {
  constructor(readonly reason?: unknown) {
  }

  toPumpEnd(): PumpEnd {
    return this.reason === undefined
      ? {kind: "stop"}
      : {kind: "stop", reason: this.reason};
  }
}

/**
 * How a pump loop ended:
 *   - `{ kind: "exit" }`    — the native side reported exit (all windows closed)
 *   - `{ kind: "stop" }`    — `PumpHandle.stop()` was called; `reason` carries
 *                             the optional stop reason passed to it
 *   - `{ kind: "abort" }`   — an external `AbortSignal` was aborted; `reason`
 *                             carries the signal's abort reason
 */
export type PumpEnd =
  | { kind: "exit" }
  | { kind: "stop"; reason?: unknown }
  | { kind: "abort"; reason: unknown };

/** `pump:start` event carrying the loop start. */
export class PumpStartEvent extends Event {
  constructor() {
    super("pump:start");
  }
}

/** `pump` event carrying each iteration's result. */
export class PumpEvent extends Event {
  constructor(readonly result: PumpResult) {
    super("pump");
  }
}

/** `pump:end` event carrying how the loop ended. */
export class PumpEndEvent extends Event {
  constructor(readonly end: PumpEnd) {
    super("pump:end");
  }
}

/** `pump:error` event carrying the thrown error. */
export class PumpErrorEvent extends Event {
  constructor(readonly error: unknown) {
    super("pump:error");
  }
}

/**
 * Handle returned by `startPumpLoop`: `done` settles when the loop ends —
 * it resolves with a `PumpEnd` describing how it ended, or rejects with the
 * thrown error (also broadcast as `pump:error`). The rejection is consumed
 * internally, so awaiting it from top-level setup and ignoring it
 * fire-and-forget are both safe — no unhandled rejection either way.
 */
export interface PumpHandle {
  done: Promise<PumpEnd>;
  stop: (reason?: unknown) => void;
}

/** Apps with an active loop, so a second `startPumpLoop` rejects. */
const running = new WeakSet<BlitzApp>();

/**
 * Start a background pump loop that keeps driving the event loop until
 * the native side reports exit (all windows closed), `signal` aborts, or
 * `handle.stop()` is called.
 *
 * The loop targets a stable cadence of `targetPeriod` ms per iteration,
 * anchored to an absolute `performance.now()` timeline: each pump may
 * block up to `timeout` ms waiting for events, and the loop sleeps until
 * the next target tick (so a pump that returns early does not make the
 * cadence faster, and `setTimeout` imprecision does not accumulate). If a
 * pump overruns its period (e.g. heavy rendering), the next iteration runs
 * immediately, aligned to now + targetPeriod.
 *
 * Only one pump loop may run per app; calling again while one is active
 * throws. Start the loop from top-level setup, not from inside an event
 * handler — pumping from within a pump re-enters the native loop.
 */
export function startPumpLoop(
  app: BlitzApp,
  options: PumpOptions = {},
): PumpHandle {
  if (running.has(app)) {
    throw new Error("pumpLoop: a pump loop is already running");
  }
  const {
    targetPeriod = 16.67,
    timeout = targetPeriod,
    signal,
  } = options;

  // Single stop source: an external `signal` and `handle.stop()` both
  // converge on this controller, the only signal the loop checks.
  const controller = new AbortController();
  if (signal) {
    if (signal.aborted) controller.abort();
    else signal.addEventListener("abort", () => controller.abort(signal.reason), {once: true});
  }

  running.add(app);
  app.dispatchEvent(new PumpStartEvent());

  const done = runLoop(app, targetPeriod, timeout, controller.signal);

  // Consume the rejection so a fire-and-forget caller never hits an
  // unhandled rejection; the rejection itself is preserved for awaiters,
  // and the error is broadcast as `pump:error`.
  done.catch((error) => {
    app.dispatchEvent(new PumpErrorEvent(error));
  });

  return {
    done,
    stop: (reason?: unknown) => controller.abort(new PumpStopRequest(reason)),
  };
}

async function runLoop(
  app: BlitzApp,
  targetPeriod: number,
  timeout: number,
  signal: AbortSignal,
): Promise<PumpEnd> {
  let end: PumpEnd = {kind: "exit"};
  try {
    // Absolute target tick on the `performance.now()` timeline. Each
    // iteration takes exactly one `now` sample, sleeps until the target
    // tick, then advances it. Anchoring to absolute timestamps (rather
    // than a fixed per-iteration sleep) means `setTimeout`'s imprecision
    // never accumulates into cadence drift.
    let next = performance.now() + targetPeriod;
    while (true) {
      if (signal.aborted) {
        // `stop()` aborts the controller with a `PumpStopRequest`; any
        // other abort is external and its reason is carried through.
        const abortReason = signal.reason;
        if (abortReason instanceof PumpStopRequest) {
          end = abortReason.toPumpEnd();
        } else {
          end = {kind: "abort", reason: abortReason};
        }
        break;
      }
      const result = app.pumpAppEvents(timeout);
      app.dispatchEvent(new PumpEvent(result));
      if (result.exit) {
        end = {kind: "exit"};
        break;
      }
      const now = performance.now();
      const diff = next - now;
      await new Promise<void>((resolve) => setTimeout(resolve, Math.max(diff, 0)));
      next = (diff <= 0 ? now : next) + targetPeriod;
    }
  } finally {
    running.delete(app);
    app.dispatchEvent(new PumpEndEvent(end));
  }
  return end;
}