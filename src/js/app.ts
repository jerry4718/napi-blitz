// `BlitzApp` is the JS-side wrapper for the underlying winit-driven
// blitz application. JS users instantiate it once, attach as many
// Documents (windows) as they like, and pump the loop.
//
// The native handle is stored as a private field. Document's native
// handle is similarly private; we reach into it via a contained cast,
// the package's only "friend" channel between BlitzApp and Document.

import { NativeBlitzAppCtor, type NativeBlitzApp, type NativeDocHandle, type PumpResult } from "./native";
import type { Document } from "./document";

/** Document's package-private fields, viewed by BlitzApp. */
interface DocumentInternals {
  readonly _native: NativeDocHandle;
}

function pluckDoc(doc: Document): DocumentInternals {
  return doc as unknown as DocumentInternals;
}

export class BlitzApp {
  private readonly _native: NativeBlitzApp;

  private constructor(native: NativeBlitzApp) {
    this._native = native;
  }

  /** Build the underlying winit event loop and blitz application. */
  static create(): BlitzApp {
    return new BlitzApp(NativeBlitzAppCtor.create());
  }

  /** Attach a fresh window driven by the given document. */
  openWindow(doc: Document): void {
    this._native.openWindow(pluckDoc(doc)._native);
  }

  /**
   * Pump pending winit events, blocking up to `millis` milliseconds. Call
   * this in a loop (e.g. once per animation frame) to drive rendering and
   * event handling. JS event listeners run synchronously inside this call.
   */
  pumpAppEvents(millis: number): PumpResult {
    return this._native.pumpAppEvents(millis);
  }
}
