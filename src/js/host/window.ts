// `Window` — JS-side handle for one open OS window. Mirrors the web
// `Window` interface in the parts that make sense for a non-browser
// embedding: it exposes the document, runtime size/resizable controls,
// and a `close()` action.
//
// `Window` extends `EventTarget`, so JS code can listen for lifecycle
// events:
//
//   - `close`   (cancelable): fires before the window is torn down.
//                Dispatched from two places:
//                  * The OS window manager's "close request" (user
//                    clicked the X button or hit Cmd-W / Alt-F4),
//                    routed via the native `setAppEventHandler` hook
//                    on `BlitzApp`.
//                  * `Window.close()` / `BlitzApp.closeWindow(w)`,
//                    which dispatch the same event before delegating
//                    to the native side.
//                Calling `event.preventDefault()` cancels the close.
//
//   - `closed`  (non-cancelable): fires after the window has been
//                removed from the application. This is the place to
//                drop references and let the GC reclaim the
//                associated document tree.
//
// We deliberately do NOT close the OS window in a `FinalizationRegistry`
// callback. GC timing is unpredictable, and a user calling `close()`
// expects the window to disappear immediately. The Rust side mirrors
// this: `BlitzApp.close_window` runs synchronously.

import type { BlitzApp } from "./app";
import type { HTMLDocument } from "../document/html-document";
import type { Window as NativeWindow } from "../native";

export class Window extends EventTarget {
  /**
   * @internal Constructed by `BlitzApp.openWindow`. Direct construction
   * outside the package is unsupported.
   */
  constructor(
    private readonly _app: BlitzApp,
    private readonly _nativeWindow: NativeWindow,
    private readonly _document: HTMLDocument,
  ) {
    super();
  }

  /** The HTMLDocument painted in this window. */
  get document(): HTMLDocument {
    return this._document;
  }

  /**
   * Stable numeric id of the document attached to this window. Mirrors
   * the native `doc_id` and is used by `BlitzApp` to look up windows
   * routed from the OS event handler.
   */
  get docId(): bigint {
    return this._nativeWindow.docId;
  }

  /** Whether the window has been closed. */
  get closed(): boolean {
    return this._nativeWindow.closed;
  }

  /**
   * Close the OS window synchronously. Equivalent to
   * `app.closeWindow(window)`. Dispatches a cancelable `close` event
   * first; if the listener calls `event.preventDefault()` the close
   * is aborted and `closed` will not fire. Subsequent calls on an
   * already-closed window are no-ops.
   */
  close(): void {
    this._app.closeWindow(this);
  }

  /**
   * @internal Dispatch a cancelable `close` event on this window.
   * Returns `true` if the close should proceed (default not
   * prevented). Used by `BlitzApp` for both JS-initiated closes and
   * OS-initiated closes routed through the native bridge.
   */
  _dispatchClose(): boolean {
    const event = new Event("close", { cancelable: true });
    this.dispatchEvent(event);
    return !event.defaultPrevented;
  }

  /** @internal Dispatch the non-cancelable `closed` event. */
  _dispatchClosed(): void {
    this.dispatchEvent(new Event("closed"));
  }

  /**
   * Current surface size in physical pixels, as `[width, height]`.
   * Returns `null` if the window has not been initialised yet (no
   * `pumpAppEvents` has run since open) or has been closed.
   */
  get innerSize(): [number, number] | null {
    const dims = this._app._native.getWindowInnerSize(this._nativeWindow);
    if (dims === null) return null;
    return [dims[0], dims[1]];
  }

  /**
   * Request a new surface size. winit may settle on a different size
   * depending on the platform's window manager; observe `resize`
   * events on the document for the actual outcome.
   */
  resize(width: number, height: number): void {
    this._app._native.setWindowInnerSize(this._nativeWindow, width, height);
  }

  /**
   * Whether the window can currently be resized by the user. Returns
   * `null` while the window is uninitialised (e.g. before the first
   * `pumpAppEvents`).
   */
  get resizable(): boolean | null {
    return this._app._native.getWindowResizable(this._nativeWindow);
  }

  set resizable(value: boolean) {
    this._app._native.setWindowResizable(this._nativeWindow, value);
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
