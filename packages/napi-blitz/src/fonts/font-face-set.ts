// `FontFaceSet` — JS-facing collection of registered `FontFace`s,
// mirroring the web `FontFaceSet` interface
// (https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSet).
//
// `document.fonts` returns an instance of this. Adding a `FontFace` to
// the set ships its bytes into the underlying `FontContext` via the
// native `DocHandle.registerFont` call, so subsequent layout/paint can
// shape with it.
//
// The set is iterable in insertion order. We intentionally implement
// the most-used surface (`add` / `delete` / `clear` / `has` / `size`
// / iteration / `forEach` / `loaded` / `ready` / `status`) and skip
// the parts that need a font-loading event loop:
//
//   - `load(font, text?)` and `check(font, text?)` query the set with
//     a CSS shorthand and an optional sample string. Implementing them
//     well requires a CSS font-shorthand parser plus shaping queries.
//     We expose stubs that throw so the surface is honest.
//   - `loading` / `loadingdone` / `loadingerror` events are not emitted
//     because every add resolves synchronously. `EventTarget` methods
//     are still inherited so `addEventListener` is a safe no-op.

import type { NativeDocHandle, RegisterFontOptions } from "../native";
import { FontFace } from "./font-face";

/** Standard `FontFaceSet.status` values. */
export type FontFaceSetLoadStatus = "loading" | "loaded";

/**
 * The collection backing `document.fonts`.
 *
 * Iteration yields registered faces in insertion order. The set
 * extends `EventTarget` so consumers can attach `loadingdone` listeners
 * even though we do not currently dispatch them.
 */
export class FontFaceSet extends EventTarget {
  private readonly _native: NativeDocHandle;
  private readonly _faces: Set<FontFace> = new Set();
  private readonly _ready: Promise<FontFaceSet>;

  /** @internal Constructed by `Document`. */
  constructor(native: NativeDocHandle) {
    super();
    this._native = native;
    // We register synchronously, so `ready` is always already resolved.
    // If a future iteration introduces in-flight loading (e.g. URL
    // sources), this should become a fresh Promise each time the set
    // transitions out of `"loaded"`, per spec.
    this._ready = Promise.resolve(this);
  }

  /** `"loading"` while an add is in-flight, `"loaded"` otherwise. */
  get status(): FontFaceSetLoadStatus {
    // We register synchronously, so once `add` returns the engine has
    // the bytes. There is no in-flight state to report.
    return "loaded";
  }

  /**
   * Resolves once all currently-pending font loads complete. With our
   * synchronous registration this is always already-resolved.
   */
  get ready(): Promise<FontFaceSet> {
    return this._ready;
  }

  /** Number of faces currently in the set. */
  get size(): number {
    return this._faces.size;
  }

  /**
   * Add `face` to the set, registering its bytes with the underlying
   * font cache. Returns `this` (per spec).
   *
   * If `face.load()` has not yet been called, it is invoked
   * automatically. URL-source faces (which we don't fetch) cause this
   * to throw — use a `@font-face` CSS rule instead.
   */
  add(face: FontFace): this {
    if (!(face instanceof FontFace)) {
      throw new TypeError("FontFaceSet.add: argument must be a FontFace");
    }
    if (this._faces.has(face)) return this;

    if (face._hasUrlSource()) {
      throw new TypeError(
        "FontFaceSet.add: URL-string sources are not supported. " +
          "Pass an ArrayBuffer/TypedArray, or use a `@font-face` CSS rule.",
      );
    }

    const bytes = face._takeBytes();
    if (bytes === null || bytes.byteLength === 0) {
      throw new TypeError("FontFaceSet.add: face has no source data to register");
    }

    const opts: RegisterFontOptions = {
      familyName: face.family,
    };
    if (face.weight !== "normal") opts.weight = face.weight;
    if (face.style !== "normal") opts.style = face.style;
    if (face.stretch !== "normal") opts.stretch = face.stretch;

    try {
      this._native.registerFont(bytes, opts);
    } catch (err) {
      face._markError(err);
      throw err;
    }

    face._markLoaded();
    this._faces.add(face);
    return this;
  }

  /**
   * Remove `face` from the set. Returns whether the face was present.
   *
   * NOTE: blitz's font cache currently has no public unregister path
   * for individual faces, so the bytes remain resolvable inside the
   * engine. The face is removed from this set (so iteration stops
   * yielding it), but layout that already references the family will
   * continue to find it. This matches the browser at the JS-visible
   * layer for current frames; future re-styling will still see it.
   */
  delete(face: FontFace): boolean {
    return this._faces.delete(face);
  }

  /** Whether `face` is currently in the set. */
  has(face: FontFace): boolean {
    return this._faces.has(face);
  }

  /** Drop every face. Same caveat as `delete` re: engine-side caching. */
  clear(): void {
    this._faces.clear();
  }

  /** Iterate over registered faces in insertion order. */
  forEach(
    callback: (value: FontFace, key: FontFace, set: FontFaceSet) => void,
    thisArg?: unknown,
  ): void {
    for (const face of this._faces) {
      callback.call(thisArg, face, face, this);
    }
  }

  [Symbol.iterator](): IterableIterator<FontFace> {
    return this._faces.values();
  }

  values(): IterableIterator<FontFace> {
    return this._faces.values();
  }

  keys(): IterableIterator<FontFace> {
    return this._faces.values();
  }

  entries(): IterableIterator<[FontFace, FontFace]> {
    const inner = this._faces.values();
    return {
      next(): IteratorResult<[FontFace, FontFace]> {
        const r = inner.next();
        if (r.done) return { value: undefined, done: true };
        return { value: [r.value, r.value], done: false };
      },
      [Symbol.iterator](): IterableIterator<[FontFace, FontFace]> {
        return this;
      },
    };
  }

  /**
   * Spec: returns a Promise resolving to the matching faces for the
   * given CSS font shorthand. We do not yet have a font-shorthand
   * parser nor a shaping-aware query path; throwing makes the gap
   * obvious instead of silently returning [].
   */
  load(_font: string, _text?: string): Promise<FontFace[]> {
    return Promise.reject(
      new Error("FontFaceSet.load(font, text?) is not yet implemented"),
    );
  }

  /**
   * Spec: returns whether the given CSS font shorthand can be rendered
   * using only currently-loaded fonts. Same gap as `load`.
   */
  check(_font: string, _text?: string): boolean {
    throw new Error("FontFaceSet.check(font, text?) is not yet implemented");
  }
}
