// `Window` — JS-side handle for one open OS window. Mirrors the web
// `Window` interface in the parts that make sense for a non-browser
// embedding: it exposes the document and a `close()` action.
//
// We deliberately do NOT close the OS window in a `FinalizationRegistry`
// callback. GC timing is unpredictable, and a user calling `close()`
// expects the window to disappear immediately. The Rust side mirrors
// this: `BlitzApp.close_window` runs synchronously.

import type { BlitzApp } from "./app";
import type { HTMLDocument } from "../document/html-document";
import type { Window as NativeWindow } from "../native";

export class Window {
  /**
   * @internal Constructed by `BlitzApp.openWindow`. Direct construction
   * outside the package is unsupported.
   */
  constructor(
    private readonly _app: BlitzApp,
    private readonly _nativeWindow: NativeWindow,
    private readonly _document: HTMLDocument,
  ) {}

  /** The HTMLDocument painted in this window. */
  get document(): HTMLDocument {
    return this._document;
  }

  /** Whether the window has been closed. */
  get closed(): boolean {
    return this._nativeWindow.closed;
  }

  /**
   * Close the OS window synchronously. Equivalent to
   * `app.closeWindow(window)`. Subsequent calls are no-ops.
   */
  close(): void {
    this._app.closeWindow(this);
  }
}

/** Internals viewed by the package's friend modules. */
export interface WindowInternals {
  readonly _nativeWindow: NativeWindow;
  readonly _document: HTMLDocument;
}

/** Read the package-private fields off a `Window` instance. */
export function pluckWindow(w: Window): WindowInternals {
  return w as unknown as WindowInternals;
}
