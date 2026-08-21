// `BlitzApp` — JS-side wrapper for the underlying winit-driven blitz
// application. Each app owns one event loop and any number of windows.
//
// Lifecycle:
//   1. `BlitzApp.create()` builds the loop and installs the
//      app-event bridge with the native side.
//   2. `app.openWindow(document, options?)` attaches an existing
//      `HTMLDocument` to a new native window. Returns a `Window`.
//   3. `app.pumpAppEvents(ms)` drives the loop. Call once per frame.
//   4. `app.closeWindow(window)` (or `window.close()`) closes the
//      window synchronously. Both paths dispatch a cancelable `close`
//      on the window first; if not prevented, native closes the
//      window and we dispatch `closed` on the window plus a
//      `window:close` / `window:closed` pair on the app.
//
// `BlitzApp` extends `EventTarget` so JS code can observe lifecycle
// changes across all windows from a single place:
//
//   - `window:open`   (non-cancelable, `detail: { window }`)
//   - `window:close`  (non-cancelable; the window-level `close` already
//                     gave anyone a chance to cancel)
//   - `window:closed` (non-cancelable)
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
  type AppDispatchResult,
  type AppEventPayload,
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
    // Wire the native -> JS bridge so winit `CloseRequested` reaches
    // us as a `close` event on the right window. The handler runs
    // synchronously inside `pumpAppEvents`.
    this._native.setAppEventHandler((payload: AppEventPayload) =>
      this._dispatchFromNative(payload),
    );
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
    const nativeWindow: InstanceType<typeof NativeWindow> = await this._native.openWindow(
      pluckDoc(document)._native,
      options,
    );
    const window = new Window(this, nativeWindow, document);
    this._windows.set(nativeWindow.windowId, window);

    this.dispatchEvent(
      new CustomEvent("window:open", {detail: {window}}),
    );
    return window;
  }

  /**
   * Close a window synchronously. After this call returns the window
   * stops painting and receiving events; subsequent `closeWindow` calls
   * for the same window are no-ops.
   *
   * Dispatches `close` (cancelable) on the window first. If the
   * default is prevented, this call returns without closing. On a
   * successful close, dispatches `closed` on the window plus
   * `window:close` and `window:closed` on this app.
   */
  closeWindow(window: Window): void {
    if (!this._windows.has(pluckWindow(window)._nativeWindow.windowId)) return;
    if (window.closed) {
      this._windows.delete(pluckWindow(window)._nativeWindow.windowId);
      return;
    }
    if (!window._dispatchClose()) {
      // Listener cancelled the close.
      return;
    }
    // The native `closeWindow` will fire its own `closed` notification
    // through the bridge — but only for windows the bridge knows
    // about. We forward, then dispatch the JS-visible side-effects.
    // To avoid a duplicate `closed` from the bridge, drop the window
    // from our map *before* calling native: when the bridge fires we
    // will not find a wrapper and skip the JS dispatch.
    const windowId = pluckWindow(window)._nativeWindow.windowId;
    this._windows.delete(windowId);

    this._native.closeWindow(pluckWindow(window)._nativeWindow);

    window._dispatchClosed();
    this.dispatchEvent(
      new CustomEvent("window:close", {detail: {window}}),
    );
    this.dispatchEvent(
      new CustomEvent("window:closed", {detail: {window}}),
    );
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

  /**
   * @internal Receive an app event the native side serialized while
   * inside `pumpAppEvents`. Returns the dispatch result so native can
   * decide whether to respect `preventDefault()`.
   */
  private _dispatchFromNative(payload: AppEventPayload): AppDispatchResult {
    const window = this._windows.get(payload.windowId);
    if (window === undefined) {
      // Window already gone from our map — nothing to dispatch.
      return {defaultPrevented: false};
    }

    if (payload.type === "close") {
      const proceed = window._dispatchClose();
      return {defaultPrevented: !proceed};
    }
    if (payload.type === "closed") {
      // The window is gone on the native side. Mirror that on the JS
      // side and dispatch the matching events.
      this._windows.delete(payload.windowId);
      window._dispatchClosed();
      this.dispatchEvent(
        new CustomEvent("window:close", {detail: {window}}),
      );
      this.dispatchEvent(
        new CustomEvent("window:closed", {detail: {window}}),
      );
      return {defaultPrevented: false};
    }
    return {defaultPrevented: false};
  }
}
