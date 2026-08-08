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

import type {BlitzApp} from "./app";
import type {HTMLDocument} from "../document/html-document";
import type {MonitorInfo, VideoModeInfo} from "../native";
import {NativeWindow} from "../native";

export class Window extends EventTarget {
  /**
   * @internal Constructed by `BlitzApp.openWindow`. Direct construction
   * outside the package is unsupported.
   */
  constructor(
    private readonly _app: BlitzApp,
    private readonly _nativeWindow: InstanceType<typeof NativeWindow>,
    private readonly _document: HTMLDocument,
  ) {
    super();
    // Register our dispatchEvent so Rust can forward pointer events
    // (pointermove/pointerup) to window-level listeners during
    // `handle_event`. The DOM chain walk only reaches DOM nodes; this
    // lets `window.addEventListener('pointermove', ...)` work.
    this._document._native.setWindowDispatch(
      (event: unknown) => this.dispatchEvent(event as Event),
    );
  }

  /** The HTMLDocument painted in this window. */
  get document(): HTMLDocument {
    return this._document;
  }

  /**
   * Opaque window identifier. Mirrors the native `window_id`
   * and is used by `BlitzApp` to look up windows routed from the
   * OS event handler.
   */
  get windowId(): bigint {
    return this._nativeWindow.windowId;
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
    const event = new Event("close", {cancelable: true});
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
    try {
      const dims = this._nativeWindow.getSize();
      return [dims[0], dims[1]];
    } catch {
      return null;
    }
  }

  /**
   * Request a new surface size. winit may settle on a different size
   * depending on the platform's window manager; observe `resize`
   * events on the document for the actual outcome.
   */
  resize(width: number, height: number): void {
    this._nativeWindow.setSize(width, height);
  }

  /**
   * Whether the window can currently be resized by the user. Returns
   * `null` while the window is uninitialised (e.g. before the first
   * `pumpAppEvents`).
   */
  get resizable(): boolean | null {
    try {
      return this._nativeWindow.getResizable();
    } catch {
      return null;
    }
  }

  get currentMonitor(): MonitorInfo | null {
    return this._nativeWindow.currentMonitor();
  }

  set resizable(value: boolean) {
    this._nativeWindow.setResizable(value);
  }

  setTitle(title: string): void {
    this._nativeWindow.setTitle(title);
  }

  setSize(width: number, height: number): void {
    this._nativeWindow.setSize(width, height);
  }

  setMinSize(width: number, height: number): void {
    this._nativeWindow.setMinSize(width, height);
  }

  setMaxSize(width: number, height: number): void {
    this._nativeWindow.setMaxSize(width, height);
  }

  setResizable(value: boolean): void {
    this._nativeWindow.setResizable(value);
  }

  setMaximized(value: boolean): void {
    this._nativeWindow.setMaximized(value);
  }

  setVisible(value: boolean): void {
    this._nativeWindow.setVisible(value);
  }

  setTransparent(value: boolean): void {
    this._nativeWindow.setTransparent(value);
  }

  setBlur(value: boolean): void {
    this._nativeWindow.setBlur(value);
  }

  setDecorations(value: boolean): void {
    this._nativeWindow.setDecorations(value);
  }

  setFullscreenBorderless(monitor: MonitorInfo): void {
    this._nativeWindow.setFullscreenBorderless(monitor);
  }

  setFullscreenExclusive(
    monitor: MonitorInfo,
    videoMode: VideoModeInfo,
  ): void {
    this._nativeWindow.setFullscreenExclusive(monitor, videoMode);
  }

  setFullscreenNone(): void {
    this._nativeWindow.setFullscreenNone();
  }

  setEnabledButtons(buttons: string[]): void {
    this._nativeWindow.setEnabledButtons(buttons);
  }

  setWindowIcon(data: Uint8Array): void {
    this._nativeWindow.setWindowIcon(data);
  }

  /**
   * Set the document zoom level. `1.0` is unzoomed. The total viewport
   * scale is `hidpi_scale * zoom`, which scales layout and CSS transforms.
   */
  setZoom(zoom: number): void {
    this._app.setZoom(this, zoom);
  }

  /** Get the current document zoom level. */
  getZoom(): number {
    return this._app.getZoom(this);
  }
}

/** Internals viewed by the package's friend modules. */
export interface WindowInternals {
  readonly _nativeWindow: InstanceType<typeof NativeWindow>;
  readonly _document: HTMLDocument;
}

/** Read the package-private fields off a `Window` instance. */
export function pluckWindow(w: Window): WindowInternals {
  return w as unknown as WindowInternals;
}
