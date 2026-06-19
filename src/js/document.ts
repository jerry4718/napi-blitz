// `Document` is the user-facing top-level handle. It owns:
//
// 1. A native `DocHandle` (the Rust-side document state). This handle is
//    private; consumers cannot reach it through the public surface.
// 2. A registry mapping nodeId -> WeakRef<Element>, so JS-side wrappers
//    are unique per node and the GC can reclaim them when no listener /
//    user reference remains.
// 3. A FinalizationRegistry that informs the native side when an Element
//    has been garbage collected, so Rust can drop the corresponding
//    listener bookkeeping.
// 4. The dispatch hook that Rust calls for every DomEvent.

import {
  NativeDocHandleCtor,
  type DispatchResult,
  type DocHandleConfig,
  type EventPayload,
  type NativeDocHandle,
  type AttrInit,
} from "./native";
import { Element } from "./element";
import { buildEvent, type BlitzDomEvent } from "./events";

export interface DocumentInit {
  uaStylesheets?: string[];
  baseHtml?: string;
}

/**
 * Element's private fields, viewed by their package-internal "friend" -
 * Document. TypeScript has no real friend visibility; we contain the
 * cast here, behind the {@link pluck} helper, instead of sprinkling it
 * across the codebase.
 */
interface ElementInternals {
  readonly _nodeId: number;
  readonly _handle: NativeDocHandle;
}

/**
 * Read the package-private (`_nodeId`, `_handle`) fields off an Element.
 * Used by Document to drive node-id-based native calls without leaking
 * those fields to user code.
 */
function pluck(el: Element): ElementInternals {
  return el as unknown as ElementInternals;
}

/**
 * Top-level document. Provides factory APIs for creating elements and
 * routes events from the native side back into the JS DOM.
 */
export class Document {
  private readonly _native: NativeDocHandle;

  /** nodeId -> WeakRef<Element>. Allows GC to reclaim unused wrappers. */
  private readonly _nodes: Map<number, WeakRef<Element>> = new Map();

  /** Tells the native side when a wrapper has been GC'd. */
  private readonly _finalizer: FinalizationRegistry<number>;

  constructor(init: DocumentInit = {}) {
    const onDispatch = (payload: EventPayload): DispatchResult =>
      this._dispatchFromNative(payload);

    const cfg: DocHandleConfig = {
      uaStylesheets: init.uaStylesheets,
      baseHtml: init.baseHtml,
      onDispatch,
    };

    this._native = NativeDocHandleCtor.create(cfg);

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

  // ----- Standard DOM-style root accessors --------------------------------

  /** The `<html>` element. Equivalent to `document.documentElement`. */
  get documentElement(): Element {
    return this._wrap(this._native.rootElementId());
  }

  /** Convenience for `<body>`. May be null while parsing partial documents. */
  get body(): Element | null {
    return this.querySelector("body");
  }

  /** Convenience for `<head>`. */
  get head(): Element | null {
    return this.querySelector("head");
  }

  // ----- Element registry (private) ---------------------------------------

  /**
   * Get-or-create the JS-side wrapper for a known-existing nodeId. Returns
   * the same Element instance as long as a previous wrapper has not been
   * garbage-collected. This is intentionally private: the public API only
   * ever surfaces Elements, never raw ids.
   */
  private _wrap(nodeId: number): Element {
    const cached = this._nodes.get(nodeId)?.deref();
    if (cached) return cached;

    const el = new Element(this._native, nodeId);
    this._nodes.set(nodeId, new WeakRef(el));
    this._finalizer.register(el, nodeId);
    this._native.addListenedNode(nodeId);
    return el;
  }

  // ----- Factories --------------------------------------------------------

  createElement(localName: string, namespace?: string, attrs?: AttrInit[]): Element {
    const id = this._native.createElement(localName, namespace ?? null, attrs ?? null);
    return this._wrap(id);
  }

  createTextNode(text: string): Element {
    const id = this._native.createTextNode(text);
    return this._wrap(id);
  }

  createComment(): Element {
    const id = this._native.createCommentNode();
    return this._wrap(id);
  }

  // ----- Tree mutation ----------------------------------------------------

  appendChild(parent: Element, child: Element): void {
    this._native.appendChild(pluck(parent)._nodeId, pluck(child)._nodeId);
  }

  insertBefore(parent: Element, node: Element, anchor: Element | null): void {
    this._native.insertBefore(
      pluck(parent)._nodeId,
      pluck(node)._nodeId,
      anchor ? pluck(anchor)._nodeId : null,
    );
  }

  remove(node: Element): void {
    this._native.remove(pluck(node)._nodeId);
  }

  // ----- Tree query -------------------------------------------------------

  parentOf(node: Element): Element | null {
    const id = this._native.parentId(pluck(node)._nodeId);
    return id === null ? null : this._wrap(id);
  }

  firstChildOf(node: Element): Element | null {
    const id = this._native.firstChildId(pluck(node)._nodeId);
    return id === null ? null : this._wrap(id);
  }

  childrenOf(node: Element): Element[] {
    return this._native
      .childIds(pluck(node)._nodeId)
      .map((id) => this._wrap(id));
  }

  querySelector(selector: string): Element | null {
    const id = this._native.querySelector(selector);
    return id === null ? null : this._wrap(id);
  }

  querySelectorAll(selector: string): Element[] {
    return this._native.querySelectorAll(selector).map((id) => this._wrap(id));
  }

  getElementById(id: string): Element | null {
    const nodeId = this._native.getElementById(id);
    return nodeId === null ? null : this._wrap(nodeId);
  }

  // ----- Layout / lifecycle ----------------------------------------------

  resolve(timeMs = 0): void { this._native.resolve(timeMs); }

  loadHtml(html: string): void { this._native.loadHtml(html); }

  // ----- Native event dispatch -------------------------------------------

  /**
   * Called by Rust for every DomEvent. We dispatch along the chain
   * (target -> root) honoring `event.cancelBubble` between steps so
   * `stopPropagation()` works as expected.
   *
   * We deliberately do NOT model a separate capture phase here.
   * `EventTarget.dispatchEvent` invokes every registered listener on a
   * target regardless of `useCapture`, so simulating capture by calling
   * `dispatchEvent` multiple times along the chain would double-fire
   * default-phase listeners. Capture-registered listeners still receive
   * the event during the bubble walk; this matches what Vue / React
   * renderers expect from a custom DOM.
   *
   * Nodes that have no live JS wrapper are skipped silently.
   */
  private _dispatchFromNative(payload: EventPayload): DispatchResult {
    const event = buildEvent(payload, this);

    // Resolve the originating target Element up front. The standard
    // `EventTarget.dispatchEvent` rewrites `event.target` to whatever
    // object it's invoked on, which would clobber the original target
    // as we walk the chain. We pin `event.target` to the originating
    // element with `Object.defineProperty`; the per-dispatch listener
    // machinery treats own-properties as authoritative and won't
    // overwrite it.
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
      this._dispatchTo(id, event);
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

  private _dispatchTo(nodeId: number, event: BlitzDomEvent): void {
    const ref = this._nodes.get(nodeId);
    const target = ref?.deref();
    if (!target) return;
    target.dispatchEvent(event);
  }
}
