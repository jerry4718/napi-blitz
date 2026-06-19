// `FontFace` — JS-facing wrapper over a single registered font face.
// Mirrors the web `FontFace` interface
// (https://developer.mozilla.org/en-US/docs/Web/API/FontFace).
//
// Construction does NOT register the face with the engine — that
// happens when a `FontFace` is added to `document.fonts` (a
// `FontFaceSet`). This matches the browser model: `new FontFace(...)`
// creates an unloaded handle, and `set.add(face)` (or an explicit
// `face.load()` followed by `add`) materializes it.
//
// Source kinds:
//   - `BufferSource` (ArrayBuffer / TypedArray / DataView):
//     synchronously loadable; bytes are kept in JS land until the face
//     is registered.
//   - `string` (URL-style src descriptor): not yet supported. The
//     browser resolves this via the network and decodes the font; we
//     do not do that here. Users wanting URL-loaded fonts should put
//     `@font-face { src: url(...) }` in a stylesheet — blitz's
//     existing CSS path handles those.

/** Standard `FontFaceDescriptors` subset that we actually honor. */
export interface FontFaceDescriptors {
  /** CSS `font-style`, e.g. `"normal"`, `"italic"`, `"oblique 14deg"`. */
  style?: string;
  /** CSS `font-weight`, e.g. `"400"`, `"bold"`, `"100 900"`. */
  weight?: string;
  /** CSS `font-stretch`, e.g. `"normal"`, `"condensed"`, `"75%"`. */
  stretch?: string;
  // Reserved for future extension: unicodeRange, variant,
  // featureSettings, display. They round-trip on the FontFace but are
  // not yet wired into the underlying font cache.
  unicodeRange?: string;
  variant?: string;
  featureSettings?: string;
  display?: string;
}

/** Standard `FontFace.status` values. */
export type FontFaceLoadStatus = "unloaded" | "loading" | "loaded" | "error";

/** Source argument accepted by `new FontFace(...)`. */
export type FontFaceSource = BufferSource | string;

/**
 * A loadable font face. Construct one and pass it to
 * `document.fonts.add(face)` to make it available to layout/paint.
 */
export class FontFace {
  /** Mutable per spec: assigning `family` updates this handle's identity. */
  family: string;
  style: string;
  weight: string;
  stretch: string;
  unicodeRange: string;
  variant: string;
  featureSettings: string;
  display: string;

  private _status: FontFaceLoadStatus;
  private _bytes: Uint8Array | null;
  private _urlSource: string | null;
  private _loaded: Promise<FontFace>;
  private _resolveLoaded!: (face: FontFace) => void;
  private _rejectLoaded!: (err: unknown) => void;

  constructor(
    family: string,
    source: FontFaceSource,
    descriptors: FontFaceDescriptors = {},
  ) {
    if (typeof family !== "string" || family.length === 0) {
      throw new TypeError("FontFace: family must be a non-empty string");
    }
    this.family = family;
    this.style = descriptors.style ?? "normal";
    this.weight = descriptors.weight ?? "normal";
    this.stretch = descriptors.stretch ?? "normal";
    this.unicodeRange = descriptors.unicodeRange ?? "U+0-10FFFF";
    this.variant = descriptors.variant ?? "normal";
    this.featureSettings = descriptors.featureSettings ?? "normal";
    this.display = descriptors.display ?? "auto";

    this._loaded = new Promise<FontFace>((resolve, reject) => {
      this._resolveLoaded = resolve;
      this._rejectLoaded = reject;
    });
    // Catch handler kept silent so the unhandled-rejection guard never
    // fires before the user attaches their own handler. The `loaded`
    // getter still rejects to consumers that await it.
    this._loaded.catch(() => {});

    if (typeof source === "string") {
      // Per spec the string form is a CSS `src` descriptor list. We
      // don't resolve URLs here; record it so `load()` can throw a
      // clear error when called.
      this._urlSource = source;
      this._bytes = null;
      this._status = "unloaded";
      return;
    }

    // BufferSource: copy out a Uint8Array view we own. The caller's
    // ArrayBuffer may be detached/reused later; we want a stable view.
    this._bytes = bufferSourceToUint8Array(source);
    this._urlSource = null;
    this._status = "unloaded";
  }

  /** `"unloaded" | "loading" | "loaded" | "error"`. */
  get status(): FontFaceLoadStatus {
    return this._status;
  }

  /** Promise that resolves when the face has loaded. */
  get loaded(): Promise<FontFace> {
    return this._loaded;
  }

  /**
   * Trigger loading. For buffer-backed faces this completes
   * synchronously (the bytes are already in memory) and the returned
   * Promise is already resolved. For URL-backed faces it currently
   * rejects: use a `@font-face` stylesheet rule instead.
   */
  load(): Promise<FontFace> {
    if (this._status === "loaded" || this._status === "loading") {
      return this._loaded;
    }
    if (this._urlSource !== null) {
      this._status = "error";
      const err = new TypeError(
        "FontFace.load: URL-string sources are not supported. " +
          "Pass an ArrayBuffer/TypedArray, or use a `@font-face` CSS rule.",
      );
      this._rejectLoaded(err);
      return this._loaded;
    }
    if (this._bytes === null) {
      this._status = "error";
      const err = new TypeError("FontFace.load: no source data");
      this._rejectLoaded(err);
      return this._loaded;
    }
    this._status = "loaded";
    this._resolveLoaded(this);
    return this._loaded;
  }

  /** @internal Bytes stashed for the `FontFaceSet.add` registration path. */
  _takeBytes(): Uint8Array | null {
    return this._bytes;
  }

  /** @internal True if the face came from a URL we can't resolve. */
  _hasUrlSource(): boolean {
    return this._urlSource !== null;
  }

  /** @internal Force the status flag (used by FontFaceSet on add success). */
  _markLoaded(): void {
    if (this._status !== "loaded") {
      this._status = "loaded";
      this._resolveLoaded(this);
    }
  }

  /** @internal Force an error state with the given reason. */
  _markError(err: unknown): void {
    this._status = "error";
    this._rejectLoaded(err);
  }
}

/**
 * Produce a fresh `Uint8Array` view over the bytes of any
 * `BufferSource`. Always returns a tight slice — no lingering view of
 * the wider underlying buffer.
 */
function bufferSourceToUint8Array(source: BufferSource): Uint8Array {
  if (source instanceof ArrayBuffer) {
    return new Uint8Array(source.slice(0));
  }
  if (ArrayBuffer.isView(source)) {
    const view = source as ArrayBufferView;
    // Slice to detach from the original buffer's offset/length.
    const out = new Uint8Array(view.byteLength);
    out.set(
      new Uint8Array(view.buffer, view.byteOffset, view.byteLength),
    );
    return out;
  }
  throw new TypeError(
    "FontFace: source must be a BufferSource (ArrayBuffer/TypedArray/DataView) or a string",
  );
}
