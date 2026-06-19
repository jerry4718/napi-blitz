// `Document` — abstract base class for any document type. Concrete
// subclasses are `HTMLDocument`, `XMLDocument`, `SVGDocument`.
//
// Document owns:
//   - the native `DocHandle`
//   - the JS-side wrapper registry (nodeId -> WeakRef<Node>)
//   - the FinalizationRegistry that informs Rust when wrappers are GC'd
//   - the dispatch hook called by Rust for every DomEvent
//
// Concrete subclasses customize element wrapping (`_wrapElement`) so an
// HTMLDocument hands out HTMLElement, an SVGDocument hands out
// SVGElement, etc.

import {
  NativeDocHandleCtor,
  type DispatchResult,
  type DocHandleConfig,
  type EventPayload,
  type NativeDocHandle,
} from "../native";
import { Node, NodeTypes } from "../base/node";
import { Element } from "../element/element";
import { Text } from "../base/text";
import { Comment } from "../base/comment";
import { buildEvent } from "../events/events";
import { FontFaceSet } from "../fonts/font-face-set";
import type { DocumentInternals } from "../internal/internal";

export interface DocumentInit {
  uaStylesheets?: string[];
  baseHtml?: string;
}

/**
 * Top-level base. Most users instantiate one of the concrete subclasses
 * (e.g. `HTMLDocument`); this base class is exported mainly for
 * `instanceof` checks and shared code.
 */
export abstract class Document extends Node implements DocumentInternals {
  readonly _native: NativeDocHandle;

  /** nodeId -> WeakRef<Node>. Allows GC to reclaim unused wrappers. */
  private readonly _nodes: Map<number, WeakRef<Node>> = new Map();

  /** Tells the native side when a wrapper has been GC'd. */
  private readonly _finalizer: FinalizationRegistry<number>;

  /** Lazily-built `FontFaceSet` exposed via `document.fonts`. */
  private _fontsSet: FontFaceSet | null = null;

  protected constructor(init: DocumentInit = {}) {
    // Bootstrap: native handle wants the dispatch callback up front, but
    // we can't reference `this` before `super(...)`. Capture a forward
    // reference cell instead and back-fill after `super()`.
    const self: { ref: Document | null } = { ref: null };
    const native = NativeDocHandleCtor.create({
      uaStylesheets: init.uaStylesheets,
      baseHtml: init.baseHtml,
      onDispatch: (payload: EventPayload): DispatchResult => {
        const doc = self.ref;
        if (doc === null) {
          // An event fired during construction; not expected in
          // practice (we haven't pumped the loop yet) but be safe.
          return {
            defaultPrevented: false,
            propagationStopped: false,
            requestRedraw: false,
          };
        }
        return doc._dispatchFromNative(payload);
      },
    });

    super(native, native.rootNodeId(), undefined as unknown as DocumentInternals);

    self.ref = this;
    this._native = native;
    // Document is its own ownerDocument.
    (this as unknown as { _ownerDocument: DocumentInternals })._ownerDocument = this;

    this._finalizer = new FinalizationRegistry<number>((nodeId) => {
      const ref = this._nodes.get(nodeId);
      if (ref && ref.deref() === undefined) {
        this._nodes.delete(nodeId);
      }
      try {
        this._native.removeListenedNode(nodeId);
      } catch {
        // intentionally ignored
      }
    });
  }

  // ----- Standard DOM root accessors --------------------------------------

  /**
   * The `<html>` element. Uses a fast-path lookup that bypasses the CSS
   * selector engine — blitz keeps an internal TreeTraverser equivalent
   * and we replicate it on the napi side.
   */
  get documentElement(): Element {
    const id = this._native.htmlElementId();
    if (id !== null) return this._wrap(id) as Element;
    // Fallback to the napi rootElementId (always 0 in blitz, the
    // document node) when the tree has no <html> wrapper yet.
    return this._wrap(this._native.rootElementId()) as Element;
  }

  /**
   * The `<head>` element, or null if missing. Fast-path lookup; does
   * not go through querySelector.
   */
  get head(): Element | null {
    const id = this._native.headElementId();
    return id === null ? null : (this._wrap(id) as Element);
  }

  /**
   * The `<body>` element, or null if missing. Fast-path lookup; does
   * not go through querySelector.
   */
  get body(): Element | null {
    const id = this._native.bodyElementId();
    return id === null ? null : (this._wrap(id) as Element);
  }

  /**
   * Document title. Reads the textContent of the `<title>` element if
   * one exists; returns `""` otherwise. Setting the title finds the
   * existing `<title>` and updates it, or creates a fresh one inside
   * `<head>`. Mirrors the standard DOM `document.title`.
   *
   * Lookup uses blitz's `find_title_node_id`, which is a single
   * short-circuiting tree traversal — cheaper than a CSS-selector
   * dispatch.
   *
   * blitz observes the `<title>` element on each mutator-flush and
   * synchronously calls `winit_window.set_title(...)`, so the OS window
   * title updates within the same frame as the property write.
   */
  get title(): string {
    const id = this._native.findTitleNodeId();
    if (id === null) return "";
    return (this._wrap(id) as Element).textContent ?? "";
  }
  set title(value: string) {
    const existingId = this._native.findTitleNodeId();
    if (existingId !== null) {
      (this._wrap(existingId) as Element).textContent = value;
      return;
    }
    // No <title> yet: create one and append to <head>. If <head> is
    // also missing (unusual for HTML documents) fall back to
    // documentElement so the element at least lives in the tree.
    const titleEl = this.createElement("title");
    titleEl.textContent = value;
    const head = this.head ?? this.documentElement;
    head.appendChild(titleEl);
  }

  // ----- Node registry ----------------------------------------------------

  /**
   * Get-or-create the JS-side wrapper for a known-existing nodeId.
   * Concrete subclasses override `_makeWrapper` to choose Element vs.
   * HTMLElement vs. SVGElement etc.
   */
  _wrap(nodeId: number): Node {
    const cached = this._nodes.get(nodeId)?.deref();
    if (cached) return cached;

    const node = this._makeWrapper(nodeId);
    this._nodes.set(nodeId, new WeakRef(node));
    this._finalizer.register(node, nodeId);
    this._native.addListenedNode(nodeId);
    return node;
  }

  /**
   * Build a fresh wrapper for `nodeId`. Default implementation handles
   * Text/Comment and falls back to `Element` for everything else.
   * Subclasses override to specialize element wrapping.
   */
  protected _makeWrapper(nodeId: number): Node {
    const type = this._native.nodeType(nodeId);
    if (type === NodeTypes.TEXT_NODE) return new Text(this._native, nodeId, this);
    if (type === NodeTypes.COMMENT_NODE) return new Comment(this._native, nodeId, this);
    return this._makeElementWrapper(nodeId);
  }

  /** Build an Element-class wrapper. Overridden per document type. */
  protected abstract _makeElementWrapper(nodeId: number): Element;

  // ----- Factories --------------------------------------------------------

  createElement(localName: string): Element {
    const id = this._native.createElement(localName, null, null);
    return this._wrap(id) as Element;
  }

  createElementNS(namespace: string | null, qualifiedName: string): Element {
    const id = this._native.createElement(qualifiedName, namespace, null);
    return this._wrap(id) as Element;
  }

  createTextNode(text: string): Text {
    const id = this._native.createTextNode(text);
    return this._wrap(id) as Text;
  }

  createComment(data?: string): Comment {
    const id = this._native.createCommentNode();
    const comment = this._wrap(id) as Comment;
    if (data !== undefined && data !== "") {
      // Standard signature is `createComment(data)` with `data` non-optional;
      // we accept undefined for ergonomics. Setter triggers the one-shot
      // warning if blitz's Comment payload is still absent.
      comment.data = data;
    }
    return comment;
  }

  // ----- Queries ----------------------------------------------------------

  querySelector(selector: string): Element | null {
    const id = this._native.querySelector(selector);
    return id === null ? null : (this._wrap(id) as Element);
  }

  querySelectorAll(selector: string): Element[] {
    return this._native
      .querySelectorAll(selector)
      .map((id) => this._wrap(id) as Element);
  }

  getElementById(id: string): Element | null {
    const nodeId = this._native.getElementById(id);
    return nodeId === null ? null : (this._wrap(nodeId) as Element);
  }

  /**
   * All elements with the given tag name. Uses a fast-path pre-order
   * DFS rather than the CSS selector engine. Returns a snapshot array;
   * the web spec's "live HTMLCollection" is not modeled here.
   *
   * Pass `"*"` to match every element in the document. Tag names are
   * case-folded to lowercase to match blitz's storage convention.
   */
  getElementsByTagName(name: string): Element[] {
    if (name === "*") {
      return this.querySelectorAll("*");
    }
    return this._native
      .findAllByLocalName(name.toLowerCase())
      .map((id) => this._wrap(id) as Element);
  }

  /**
   * All elements carrying the given class name. Uses a fast-path
   * pre-order DFS: we walk the tree once and test each element's
   * `class` attribute for a whitespace-separated token match. Snapshot
   * array, not a live collection.
   */
  getElementsByClassName(className: string): Element[] {
    return this._native
      .findAllByClassName(className)
      .map((id) => this._wrap(id) as Element);
  }

  // ----- Layout / lifecycle ----------------------------------------------

  /** Recompute style and layout. Drives CSS animations via `timeMs`. */
  resolve(timeMs = 0): void {
    this._native.resolve(timeMs);
  }

  // ----- Fonts ------------------------------------------------------------

  /**
   * The CSS Font Loading API `FontFaceSet` for this document. Use it
   * to register `FontFace` instances whose source is a buffer:
   *
   * ```ts
   * const face = new FontFace("MyFamily", buffer, { weight: "400" });
   * await face.load();
   * document.fonts.add(face);
   * ```
   *
   * URL-source `@font-face` rules are still handled by the engine's
   * stylesheet path; this API is only for runtime registration of
   * fonts that already exist in memory.
   */
  get fonts(): FontFaceSet {
    if (this._fontsSet === null) {
      this._fontsSet = new FontFaceSet(this._native);
    }
    return this._fontsSet;
  }

  // ----- Native event dispatch -------------------------------------------

  /**
   * Called by Rust for every DomEvent. We dispatch along the chain
   * (target -> root) honoring `event.cancelBubble` between steps so
   * `stopPropagation()` works as expected.
   */
  protected _dispatchFromNative(payload: EventPayload): DispatchResult {
    const event = buildEvent(payload, this);

    // Pin event.target to the originating element. Standard
    // `EventTarget.dispatchEvent` rewrites `target` to the dispatch
    // receiver, which would clobber the intended target as we walk.
    const originalTarget = this._nodes.get(payload.target)?.deref() ?? null;
    if (originalTarget) {
      Object.defineProperty(event, "target", {
        configurable: true,
        enumerable: true,
        value: originalTarget,
        writable: false,
      });
    }

    let propagationStopped = false;

    for (const id of payload.chain) {
      const ref = this._nodes.get(id);
      const target = ref?.deref();
      if (target) target.dispatchEvent(event);

      if (event.cancelBubble) {
        propagationStopped = true;
        break;
      }
      if (!payload.bubbles) break;
    }

    return {
      defaultPrevented: event.defaultPrevented,
      propagationStopped,
      requestRedraw: false,
    };
  }
}

/**
 * Helper: build the napi config object for a `Document`'s native handle.
 * Concrete subclasses use this in their static `create` factories.
 */
export function buildDocConfig(
  init: DocumentInit,
  onDispatch: (payload: EventPayload) => DispatchResult,
): DocHandleConfig {
  return {
    uaStylesheets: init.uaStylesheets,
    baseHtml: init.baseHtml,
    onDispatch,
  };
}
