// `BlitzApp` — JS-side wrapper for the underlying winit-driven blitz
// application. Each app owns one event loop and any number of windows.
//
// Lifecycle:
//   1. `BlitzApp.create()` builds the loop.
//   2. `app.openWindow({ html, ... })` creates a fresh HTMLDocument and
//      attaches it to a brand-new window. Returns a `Window`.
//   3. `app.pumpAppEvents(ms)` drives the loop. Call once per frame.
//   4. `app.closeWindow(window)` (or `window.close()`) closes the
//      window synchronously. The native side does not rely on JS GC.
//
// JS Document objects are private to their Window: a single Document is
// only ever attached to one Window in this design. If you need multiple
// windows, call `openWindow` multiple times.

import {
  NativeBlitzAppCtor,
  type NativeBlitzApp,
  type PumpResult,
  type Window as NativeWindow,
} from "../native";
import { HTMLDocument } from "../document/html-document";
import { Window, pluckWindow } from "./window";
import type { DocumentInit } from "../document/document";

/** Options when opening a new window. Future winit options land here. */
export interface OpenWindowInit extends DocumentInit {
  // Reserved for: title?: string, size?: { width: number; height: number },
  //               resizable?: boolean, ...
}

/** `Document`'s package-private fields, viewed by `BlitzApp`. */
interface DocumentInternalsForApp {
  readonly _native: import("../native").NativeDocHandle;
}

function pluckDoc(doc: HTMLDocument): DocumentInternalsForApp {
  return doc as unknown as DocumentInternalsForApp;
}

export class BlitzApp {
  /** @internal Used by `Window.close()` to delegate back to us. */
  readonly _native: NativeBlitzApp;

  /** Live windows, keyed by their native handle. */
  private readonly _windows: Set<Window> = new Set();

  private constructor(native: NativeBlitzApp) {
    this._native = native;
  }

  /** Build the underlying winit event loop and blitz application. */
  static create(): BlitzApp {
    return new BlitzApp(NativeBlitzAppCtor.create());
  }

  /**
   * Open a new window driven by a fresh `HTMLDocument`. Use the returned
   * `Window`'s `document` to mutate the DOM and `window.close()` to tear
   * the window down.
   */
  openWindow(init: OpenWindowInit = {}): Window {
    const document = new HTMLDocument(init);
    const nativeWindow: NativeWindow = this._native.openWindow(pluckDoc(document)._native);
    const window = new Window(this, nativeWindow, document);
    this._windows.add(window);
    return window;
  }

  /**
   * Close a window synchronously. After this call returns the window
   * stops painting and receiving events; subsequent `closeWindow` calls
   * for the same window are no-ops.
   */
  closeWindow(window: Window): void {
    if (!this._windows.has(window)) return;
    this._native.closeWindow(pluckWindow(window)._nativeWindow);
    this._windows.delete(window);
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
}
