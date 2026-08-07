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
//      `windowclose` / `windowclosed` pair on the app.
//
// `BlitzApp` extends `EventTarget` so JS code can observe lifecycle
// changes across all windows from a single place:
//
//   - `windowopen`   (non-cancelable, `detail: { window }`)
//   - `windowclose`  (non-cancelable; the window-level `close` already
//                     gave anyone a chance to cancel)
//   - `windowclosed` (non-cancelable)
//
// JS Document objects are private to their Window: a single Document is
// only ever attached to one Window in this design. If you need multiple
// windows, call `openWindow` multiple times.

import {
  type AppDispatchResult,
  type AppEventPayload,
  type NativeBlitzApp,
  NativeBlitzAppCtor,
  type PumpResult,
  type Window as NativeWindow,
  type WindowOptions,
} from "../native";
import {HTMLDocument} from "../document/html-document";
import {pluckWindow, Window} from "./window";

/** `Document`'s package-private fields, viewed by `BlitzApp`. */
interface DocumentInternalsForApp {
  readonly _native: import("../native").NativeDocHandle;
}

function pluckDoc(doc: HTMLDocument): DocumentInternalsForApp {
  return doc as unknown as DocumentInternalsForApp;
}

export class BlitzApp extends EventTarget {
  /** @internal Used by `Window.close()` to delegate back to us. */
  readonly _native: NativeBlitzApp;

  /** Live windows, keyed by their `windowId`. */
  private readonly _windows: Map<bigint, Window> = new Map();

  private constructor(native: NativeBlitzApp) {
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
    return new BlitzApp(NativeBlitzAppCtor.create());
  }

  /**
   * Open a new window for an existing `HTMLDocument`.
   * Construct window attributes with `WindowOptions.builder()`.
   */
  openWindow(document: HTMLDocument, options?: WindowOptions): Window {
    const nativeWindow: NativeWindow = this._native.openWindow(
      pluckDoc(document)._native,
      options,
    );
    const window = new Window(this, nativeWindow, document);
    this._windows.set(nativeWindow.windowId, window);

    this.dispatchEvent(
      new CustomEvent("windowopen", {detail: {window}}),
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
   * `windowclose` and `windowclosed` on this app.
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
      new CustomEvent("windowclose", {detail: {window}}),
    );
    this.dispatchEvent(
      new CustomEvent("windowclosed", {detail: {window}}),
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

    if (payload.eventType === "close") {
      const proceed = window._dispatchClose();
      return {defaultPrevented: !proceed};
    }
    if (payload.eventType === "closed") {
      // The window is gone on the native side. Mirror that on the JS
      // side and dispatch the matching events.
      this._windows.delete(payload.windowId);
      window._dispatchClosed();
      this.dispatchEvent(
        new CustomEvent("windowclose", {detail: {window}}),
      );
      this.dispatchEvent(
        new CustomEvent("windowclosed", {detail: {window}}),
      );
      return {defaultPrevented: false};
    }
    return {defaultPrevented: false};
  }
}

