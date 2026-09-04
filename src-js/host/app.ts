// `BlitzApp` — JS-side wrapper for the underlying winit-driven blitz
// application. Each app owns one event loop and any number of windows.
//
// Lifecycle:
//   1. `BlitzApp.create()` builds the loop and registers a weak self-ref
//      with the native side so Rust can dispatch lifecycle events here.
//   2. `app.openWindow(document, options?)` attaches an existing
//      `HTMLDocument` to a new native window. Returns a `Window`. While
//      creating the window (before the promise resolves), Rust dispatches
//      the cancelable app-level `window:open` event; `preventDefault()`
//      rejects the `openWindow` promise and no Window is handed out.
//   3. `app.pumpAppEvents(ms)` drives the loop. Call once per frame.
//   4. `app.closeWindow(window)` (or `window.close()`) closes the window
//      asynchronously. Rust dispatches a cancelable `close` on the window
//      and a matching cancelable `window:close` on the app; if either is
//      prevented the promise **rejects** and the window stays open.
//      Otherwise native sets the closed flag immediately, tears the window
//      down on the next pump, then dispatches `closed` on the window plus
//      `window:closed` on the app.
//
// All of these lifecycle events are dispatched from the Rust side
// (`JsShellEventHandler`) — JS holds no parallel dispatch model.
//
// `BlitzApp` extends `EventTarget` so JS code can observe lifecycle
// changes across all windows from a single place:
//
//   - `window:open`   (cancelable — app-only; fires before `openWindow`
//                     resolves, so JS does not hold a Window object yet.
//                     A listener that does not call `preventDefault()`
//                     confirms the open.)
//   - `window:opened` (non-cancelable — after the window is created and
//                     `openWindow` has resolved)
//   - `window:close`  (cancelable — the app-level echo of the window's
//                     `close`, same moment; may preventDefault() to veto)
//   - `window:closed` (non-cancelable — after teardown)
//
// Pump-loop lifecycle (`pumpLoop` / `pumpStart`):
//
//   - `pump:start` (non-cancelable)
//   - `pump`       (non-cancelable, `PumpEvent.result`)
//   - `pump:end`   (non-cancelable, `PumpEndEvent.end`)
//   - `pump:error` (non-cancelable, `PumpErrorEvent.error`) — loop threw
//
// JS Document objects are private to their Window: a single Document is
// only ever attached to one Window in this design. If you need multiple
// windows, call `openWindow` multiple times.

import {
  Event,
  EventTarget,
  NativeApp,
  NativeDoc,
  NativeWindow,
  type PumpResult,
  WindowOptions,
} from "../native";
import {HTMLDocument} from "../document/html-document";
import {TypedEventTarget} from "../helpers/events";
import type {UIEvent} from "../events/events";
import {pluckWindow, Window} from "./window";

/** `Document`'s package-private fields, viewed by `BlitzApp`. */
interface DocumentInternalsForApp {
  readonly _native: InstanceType<typeof NativeDoc>;
}

/**
 * Options for `BlitzApp.pumpLoop`. All durations are in milliseconds.
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
 * `_pumpLoop` can tell a stop request from an external abort.
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
 * How a `pumpLoop` loop ended:
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

/** Event map for `BlitzApp`, typing its `addEventListener`/`removeEventListener`. */
interface BlitzAppEventMap {
  "window:open": UIEvent;
  "window:opened": UIEvent;
  "window:close": UIEvent;
  "window:closed": UIEvent;
  "pump:start": PumpStartEvent;
  pump: PumpEvent;
  "pump:end": PumpEndEvent;
  "pump:error": PumpErrorEvent;
}

/** Handle to a running pump loop: the completion signal plus a stop request. */
export interface PumpHandle {
  /**
   * Completion signal for the loop. Resolves with a `PumpEnd` describing
   * how it ended; rejects with the thrown error if the loop crashed (also
   * broadcast as `pump:error`). The rejection is always consumed internally,
   * so ignoring `done` never surfaces an unhandled rejection.
   */
  readonly done: Promise<PumpEnd>;

  /**
   * Request the loop to stop, taking effect at the next iteration (like an
   * `AbortSignal` abort); the loop ends with `{ kind: "stop" }` (and `reason`
   * when provided) in both `done` and `pump:end`. Idempotent; no-op once
   * the loop has ended.
   */
  stop(reason?: unknown): void;
}

function pluckDoc(doc: HTMLDocument): DocumentInternalsForApp {
  return doc as unknown as DocumentInternalsForApp;
}

export class BlitzApp extends TypedEventTarget<BlitzAppEventMap>(EventTarget) {
  /** @internal Used by `Window.close()` to delegate back to us. */
  readonly _native: InstanceType<typeof NativeApp>;

  /** Live windows, keyed by their `windowId`. */
  private readonly _windows: Map<bigint, Window> = new Map();

  /** True while a `pumpLoop` loop is running (re-entrancy guard). */
  private _pumping = false;

  private constructor(native: InstanceType<typeof NativeApp>) {
    super();
    this._native = native;
    // Weak ref so Rust can dispatch app-level lifecycle events
    // (`window:open`, `window:close`, `window:closed`) directly to this
    // object from the native event loop.
    this._native.setAppRef(this);
  }

  /** Build the underlying winit event loop and blitz application. */
  static create(): BlitzApp {
    return new BlitzApp(NativeApp.create());
  }

  /**
   * Open a new window for an existing `HTMLDocument`.
   * Construct window attributes with `WindowOptions.builder()`.
   *
   * Async: the window is physically created by the next event-loop pump, so
   * this resolves once the OS window exists and `windowId` is valid. Safe to
   * call from inside an event handler (e.g. a click) — the native side never
   * recursively pumps the event loop.
   */
  async openWindow(document: HTMLDocument, options?: InstanceType<typeof WindowOptions>): Promise<Window> {
    // Rust dispatches the cancelable app-level `window:open` event while
    // creating the window, before this promise resolves. A listener's
    // `preventDefault()` rejects `openWindow` (the native side drops the
    // fresh view, so no Window is ever handed out).
    const nativeWindow: InstanceType<typeof NativeWindow> = await this._native.openWindow(
      pluckDoc(document)._native,
      options,
    );
    const window = new Window(this, nativeWindow, document);
    this._windows.set(nativeWindow.windowId, window);
    return window;
  }

  /**
   * Close a window asynchronously. The promise resolves once the native
   * `View` has actually been torn down (on the next pump) and rejects if a
   * `close` (window) or `window:close` (app) listener calls
   * `preventDefault()`.
   *
   * All lifecycle events (`close`/`window:close`, then
   * `closed`/`window:closed`) are dispatched from the Rust side.
   */
  async closeWindow(window: Window): Promise<void> {
    if (!this._windows.has(pluckWindow(window)._nativeWindow.windowId)) return;
    if (window.closed) {
      this._windows.delete(pluckWindow(window)._nativeWindow.windowId);
      return;
    }
    // Rust dispatches the cancelable `close` (window) + `window:close`
    // (app) events at the moment of the request; `preventDefault()` rejects
    // this promise and the window stays open (map kept — the delete below is
    // skipped). On success Rust already dispatched `closed` + `window:closed`,
    // so we only drop the map entry.
    await this._native.closeWindow(pluckWindow(window)._nativeWindow);
    this._windows.delete(pluckWindow(window)._nativeWindow.windowId);
  }

  /**
   * Pump pending winit events, blocking up to `millis` milliseconds.
   * Call this in a loop (e.g. once per animation frame) to drive
   * rendering and event handling. JS event listeners run synchronously
   * inside this call.
   */
  pumpAppEvents(millis: number): PumpResult {
    return this._native.pumpAppEvents(millis);
  }

  /**
   * Whether a `pumpLoop` loop is currently running.
   */
  get pumping(): boolean {
    return this._pumping;
  }

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
   * Events dispatched on this app:
   *   - `pump:start` (non-cancelable) — loop started.
   *   - `pump`       (non-cancelable, `PumpEvent.result`) — after each pump.
   *   - `pump:end`   (non-cancelable, `PumpEndEvent.end`) — loop finished.
   *   - `pump:error` (non-cancelable, `PumpErrorEvent.error`) — the loop threw.
   *
   * Returns a `PumpHandle`: `done` settles when the loop ends — it resolves
   * with a `PumpEnd` describing how it ended, or rejects with the thrown
   * error (also broadcast as `pump:error`). The rejection is consumed
   * internally, so awaiting it from top-level setup and ignoring it
   * fire-and-forget are both safe — no unhandled rejection either way.
   *
   * Only one pump loop may run per app; calling again while one is active
   * throws. Start the loop from top-level setup, not from inside an event
   * handler — pumping from within a pump re-enters the native loop.
   */
  pumpLoop(options: PumpOptions = {}): PumpHandle {
    if (this._pumping) {
      throw new Error("pumpLoop: a pump loop is already running");
    }
    const {
      targetPeriod = 16.67,
      timeout = targetPeriod,
      signal,
    } = options;

    // Single stop source: an external `signal` and `handle.stop()` both
    // converge on this controller, the only signal `_pumpLoop` checks.
    const controller = new AbortController();
    if (signal) {
      if (signal.aborted) controller.abort();
      else signal.addEventListener("abort", () => controller.abort(signal.reason), {once: true});
    }

    this._pumping = true;
    this.dispatchEvent(new PumpStartEvent());

    const done = this._pumpLoop(targetPeriod, timeout, controller.signal);

    // Consume the rejection so a fire-and-forget caller never hits an
    // unhandled rejection; the rejection itself is preserved for awaiters,
    // and the error is broadcast as `pump:error`.
    done.catch((error) => {
      this.dispatchEvent(new PumpErrorEvent(error));
    });

    return {
      done,
      stop: (reason?: unknown) => controller.abort(new PumpStopRequest(reason)),
    };
  }

  private async _pumpLoop(
    targetPeriod: number,
    timeout: number,
    signal: AbortSignal,
  ): Promise<PumpEnd> {
    const native = this._native;
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
        const result = native.pumpAppEvents(timeout);
        this.dispatchEvent(new PumpEvent(result));
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
      this._pumping = false;
      this.dispatchEvent(new PumpEndEvent(end));
    }
    return end;
  }

  /** Set the document zoom level for a window. `1.0` is unzoomed. */
  setZoom(window: Window, zoom: number): void {
    this._native.setZoom(pluckWindow(window)._nativeWindow, zoom);
  }

  /** Get the current document zoom level for a window. */
  getZoom(window: Window): number {
    return this._native.getZoom(pluckWindow(window)._nativeWindow);
  }
}
