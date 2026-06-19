// `BlitzApp` — JS-side wrapper for the underlying winit-driven blitz
// application. Each app owns one event loop and any number of windows.
//
// Lifecycle:
//   1. `BlitzApp.create()` builds the loop and installs the
//      app-event bridge with the native side.
//   2. `app.openWindow({ html, ... })` creates a fresh HTMLDocument and
//      attaches it to a brand-new window. Returns a `Window`. Also
//      dispatches a `windowopen` event on the app.
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
  NativeBlitzAppCtor,
  type AppDispatchResult,
  type AppEventPayload,
  type NativeBlitzApp,
  type PumpResult,
  type Window as NativeWindow,
  type WindowOptions,
} from "../native";
import { HTMLDocument } from "../document/html-document";
import { Window, pluckWindow } from "./window";
import type { DocumentInit } from "../document/document";

/**
 * Options when opening a new window. Combines document-init fields
 * (e.g. `baseHtml`, `uaStylesheets`) with winit-style window attributes.
 *
 * Title behaviour: if the document carries a `<title>` element, blitz
 * will overwrite the window title shortly after open via its mutator-
 * flush plumbing. The `title` option here only takes effect while the
 * document has no `<title>`. To control the title programmatically over
 * time, set `document.title` (which manipulates the `<title>` element).
 */
export interface OpenWindowInit extends DocumentInit {
  /** Initial window title. May be overwritten by the document's `<title>`. */
  title?: string;
  /** Initial surface width, physical pixels. Pair with `height`. */
  width?: number;
  /** Initial surface height, physical pixels. Pair with `width`. */
  height?: number;
  /** Whether the window is initially resizable by the user. */
  resizable?: boolean;
}

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

  /** Live windows, keyed by their attached document's `docId`. */
  private readonly _windows: Map<bigint, Window> = new Map();

  private constructor(native: NativeBlitzApp) {
    super();
    this._native = native;
    // Wire the native -> JS bridge so winit `CloseRequested` reaches
    // us as a `close` event on the right window. The handler runs
    // synchronously inside `pumpAppEvents`.
    this._native.setAppEventHandler((payload) =>
      this._dispatchFromNative(payload),
    );
  }

  /** Build the underlying winit event loop and blitz application. */
  static create(): BlitzApp {
    return new BlitzApp(NativeBlitzAppCtor.create());
  }

  /**
   * Open a new window driven by a fresh `HTMLDocument`. Use the returned
   * `Window`'s `document` to mutate the DOM and `window.close()` to tear
   * the window down.
   *
   * After the native side is set up, we dispatch a `windowopen` event
   * on this app with `event.detail = { window }`.
   */
  openWindow(init: OpenWindowInit = {}): Window {
    const document = new HTMLDocument({
      uaStylesheets: init.uaStylesheets,
      baseHtml: init.baseHtml,
    });
    const options = buildWindowOptions(init);
    const nativeWindow: NativeWindow = this._native.openWindow(
      pluckDoc(document)._native,
      options,
    );
    const window = new Window(this, nativeWindow, document);
    this._windows.set(nativeWindow.docId, window);

    this.dispatchEvent(
      new CustomEvent("windowopen", { detail: { window } }),
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
    if (!this._windows.has(pluckWindow(window)._nativeWindow.docId)) return;
    if (window.closed) {
      this._windows.delete(pluckWindow(window)._nativeWindow.docId);
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
    const docId = pluckWindow(window)._nativeWindow.docId;
    this._windows.delete(docId);

    this._native.closeWindow(pluckWindow(window)._nativeWindow);

    window._dispatchClosed();
    this.dispatchEvent(
      new CustomEvent("windowclose", { detail: { window } }),
    );
    this.dispatchEvent(
      new CustomEvent("windowclosed", { detail: { window } }),
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
    const window = this._windows.get(payload.windowDocId);
    if (window === undefined) {
      // Window already gone from our map — nothing to dispatch.
      return { defaultPrevented: false };
    }

    if (payload.eventType === "close") {
      const proceed = window._dispatchClose();
      return { defaultPrevented: !proceed };
    }
    if (payload.eventType === "closed") {
      // The window is gone on the native side. Mirror that on the JS
      // side and dispatch the matching events.
      this._windows.delete(payload.windowDocId);
      window._dispatchClosed();
      this.dispatchEvent(
        new CustomEvent("windowclose", { detail: { window } }),
      );
      this.dispatchEvent(
        new CustomEvent("windowclosed", { detail: { window } }),
      );
      return { defaultPrevented: false };
    }
    return { defaultPrevented: false };
  }
}

/**
 * Pick the window-attribute fields out of an `OpenWindowInit` and shape
 * them as the `WindowOptions` napi object. Returns `undefined` if no
 * window-level options were specified, so the native side can fall back
 * to winit's defaults without us having to construct a placeholder.
 */
function buildWindowOptions(init: OpenWindowInit): WindowOptions | undefined {
  const { title, width, height, resizable } = init;
  if (
    title === undefined &&
    width === undefined &&
    height === undefined &&
    resizable === undefined
  ) {
    return undefined;
  }
  return { title, width, height, resizable };
}
