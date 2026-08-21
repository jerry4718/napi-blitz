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
//      asynchronously. Rust dispatches the cancelable `window:close` event
//      through the ancestor chain — the window receives it first, then this
//      app. A `preventDefault()` at either level rejects the promise and the
//      window stays open. Otherwise native sets the closed flag immediately,
//      tears the window down on the next pump, then dispatches the
//      non-cancelable `window:closed` (again propagated window → app).
//
// All of these lifecycle events are dispatched from the Rust side
// (`JsShellEventHandler`) — JS holds no parallel dispatch model.
//
// `BlitzApp` extends `EventTarget` so JS code can observe lifecycle
// changes across all windows from a single place. Lifecycle events share
// one `window:*` namespace and propagate window → app in bubble order —
// window and app see the SAME event type (`event.target` is the window):
//
//   - `window:open`   (cancelable — app-only; fires before `openWindow`
//                     resolves, so JS does not hold a Window object yet.
//                     A listener that does not call `preventDefault()`
//                     confirms the open.)
//   - `window:close`  (cancelable — bubbles from the window; either level
//                     may preventDefault() to veto the close)
//   - `window:closed` (non-cancelable — after teardown)
//
// Pump-loop lifecycle (`pumpLoop` / `pumpStart`):
//
//   - `pump:start` (non-cancelable)
//   - `pump`       (non-cancelable, `detail: { result }`)
//   - `pump:end`   (non-cancelable, `detail: { reason: 'exit' | 'aborted' }`)
//   - `pump:error` (non-cancelable, `detail: { error }`) — loop threw
//
// JS Document objects are private to their Window: a single Document is
// only ever attached to one Window in this design. If you need multiple
// windows, call `openWindow` multiple times.

import {
  NativeApp,
  NativeDoc,
  NativeWindow,
  type PumpResult,
  WindowOptions,
} from "../native";
import {HTMLDocument} from "../document/html-document";
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
   * pump returns and `pumpend` fires with `reason: 'aborted'`.
   */
  signal?: AbortSignal;
}

function pluckDoc(doc: HTMLDocument): DocumentInternalsForApp {
  return doc as unknown as DocumentInternalsForApp;
}

export class BlitzApp extends EventTarget {
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
   * `window:close` listener (window or app level) calls
   * `preventDefault()`.
   *
   * All lifecycle events (`window:close`, then `window:closed`) are
   * dispatched from the Rust side and propagate window → app.
   */
  async closeWindow(window: Window): Promise<void> {
    if (!this._windows.has(pluckWindow(window)._nativeWindow.windowId)) return;
    if (window.closed) {
      this._windows.delete(pluckWindow(window)._nativeWindow.windowId);
      return;
    }
    // Rust dispatches the cancelable `window:close` event (window then app)
    // at the moment of the request; `preventDefault()` rejects this promise
    // and the window stays open (map kept — the delete below is skipped).
    // On success Rust already dispatched `window:closed` through the chain,
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
   * the native side reports exit (all windows closed) or `signal` aborts.
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
   *   - `pump`       (non-cancelable, `detail.result`) — after each pump.
   *   - `pump:end`   (non-cancelable, `detail.reason: 'exit' | 'aborted'`)
   *                  — loop finished.
   *   - `pump:error` (non-cancelable, `detail.error`) — the loop threw.
   *
   * The `join` option controls how the loop's completion is exposed:
   *   - `join: true` — returns a `Promise<void>` that resolves when the
   *     loop ends.
   *   - omitted or `join: false` — returns `undefined`: the loop runs in
   *     the background, and a thrown error is surfaced as `pump:error`
   *     instead of an unhandled rejection.
   *
   * Only one pump loop may run per app; calling again while one is active
   * throws. Start the loop from top-level setup, not from inside an event
   * handler — pumping from within a pump re-enters the native loop.
   */
  pumpLoop(options?: PumpOptions & { join?: false }): undefined
  pumpLoop(options?: PumpOptions & { join: true }): Promise<void>
  pumpLoop(options: PumpOptions & { join?: boolean } = {}): Promise<void> | undefined {
    if (this._pumping) {
      throw new Error("pumpLoop: a pump loop is already running");
    }
    const {
      targetPeriod = 16.67,
      timeout = targetPeriod,
      signal,
      join,
    } = options;

    this._pumping = true;
    this.dispatchEvent(new CustomEvent("pump:start"));

    const looping = this._pumpLoop(targetPeriod, timeout, signal);

    if (join) return looping;

    looping.catch((error) => {
      this.dispatchEvent(
        new CustomEvent("pump:error", {detail: {error}}),
      );
    })
  }

  private async _pumpLoop(
    targetPeriod: number,
    timeout: number,
    signal?: AbortSignal,
  ): Promise<void> {
    const native = this._native;
    let reason: "exit" | "aborted" = "exit";
    try {
      // Absolute target tick on the `performance.now()` timeline. Each
      // iteration takes exactly one `now` sample, sleeps until the target
      // tick, then advances it. Anchoring to absolute timestamps (rather
      // than a fixed per-iteration sleep) means `setTimeout`'s imprecision
      // never accumulates into cadence drift.
      let next = performance.now() + targetPeriod;
      while (true) {
        if (signal?.aborted) {
          reason = "aborted";
          break;
        }
        const result = native.pumpAppEvents(timeout);
        this.dispatchEvent(
          new CustomEvent("pump", {detail: {result}}),
        );
        if (result.exit) {
          reason = "exit";
          break;
        }
        const now = performance.now();
        const diff = next - now;
        await new Promise<void>((resolve) => setTimeout(resolve, Math.max(diff, 0)));
        next = (diff <= 0 ? now : next) + targetPeriod;
      }
    } finally {
      this._pumping = false;
      this.dispatchEvent(
        new CustomEvent("pump:end", {detail: {reason}}),
      );
    }
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
