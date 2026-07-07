// `Node` — abstract base class for every node in our DOM. Concrete
// subclasses are `Element` (with `HTMLElement` etc. on top), `Text`,
// `Comment`, and `Document`.
//
// Closely mirrors the web `Node` interface. Each Node holds:
//   - `_handle`: the native NodeHandle, used for node-scoped DOM ops
//   - `_nodeId`: blitz's internal id
//   - `_ownerDocument`: the JS Document this node belongs to. Used for
//     `_wrap`-based reverse lookups when returning related nodes.
//
// We keep the underscore + `protected` style instead of TS `#` so the
// internal hatch in `internal.ts` keeps working.

import type { NativeNodeHandle, AddEventListenerOptions as NativeAddEventListenerOptions } from "../native";
import { pluckNode, type DocumentInternals, type NodeInternals } from "../internal/internal";

/** DOM nodeType constants. Mirrors the web spec. */
export const NodeTypes = {
  ELEMENT_NODE: 1,
  TEXT_NODE: 3,
  COMMENT_NODE: 8,
  DOCUMENT_NODE: 9,
} as const;

export abstract class Node extends EventTarget {
  protected readonly _handle: NativeNodeHandle;
  protected readonly _nodeId: bigint;
  // Not `readonly`: `Document` patches it to `this` immediately after
  // calling `super()` (a Document is its own owner, but `this` is not
  // available before `super(...)` returns). No other code should
  // reassign this — `_setOwnerDocument` below is the only allowed
  // mutation path.
  protected _ownerDocument: DocumentInternals;

  /**
   * @internal
   * Constructed only by Document. Calling this directly outside
   * Document's `_wrap` registry will produce a Node that is not tracked
   * for caching or finalization.
   */
  constructor(
    handle: NativeNodeHandle,
    nodeId: bigint,
    ownerDocument: DocumentInternals,
  ) {
    super();
    this._handle = handle;
    this._nodeId = nodeId;
    this._ownerDocument = ownerDocument;
  }

  /**
   * @internal Patch the owner-document reference. Used exclusively by
   * the `Document` constructor to point its own `_ownerDocument` at
   * `this` after `super()` runs (there is no way to forward a
   * not-yet-constructed `this` through `super()`'s arguments).
   *
   * Non-Document call sites are a bug; we keep this `protected` so
   * the type system blocks accidental external use.
   */
  protected _setOwnerDocument(doc: DocumentInternals): void {
    this._ownerDocument = doc;
  }

  override addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ): void {
    if (listener === null) return;
    // Register the listener in the Rust-side ListenerStore.
    // This replaces the JS EventTarget's internal listener map.
    const opts: NativeAddEventListenerOptions | null =
      typeof options === "boolean"
        ? { capture: options, once: false, passive: false }
        : options
          ? {
              capture: options.capture ?? false,
              once: options.once ?? false,
              passive: options.passive ?? false,
            }
          : null;
    this._ownerDocument._native.addListener(
      this._nodeId,
      type,
      listener as (...args: unknown[]) => unknown,
      opts,
    );
  }

  override removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | EventListenerOptions,
  ): void {
    if (listener === null) return;
    const capture =
      typeof options === "boolean" ? options : (options?.capture ?? false);
    this._ownerDocument._native.removeListener(
      this._nodeId,
      type,
      listener as (...args: unknown[]) => unknown,
      capture,
    );
  }

  /** DOM-style numeric nodeType. */
  get nodeType(): number {
    return this._handle.nodeType();
  }

  /**
   * Concatenated text content of this node and its descendants. Setter
   * resets to a single text-node child for elements; for Text/Comment
   * it updates `data` directly.
   */
  get textContent(): string | null {
    return this._handle.textContent();
  }
  set textContent(value: string) {
    this._handle.setTextContent(value);
  }

  // ---- Tree relationships -------------------------------------------------

  get parentNode(): Node | null {
    const id = this._handle.parentId();
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  /** Same as `parentNode` for now; differs from spec only for non-Element parents. */
  get parentElement(): Node | null {
    return this.parentNode;
  }

  get firstChild(): Node | null {
    const id = this._handle.firstChildId();
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  get lastChild(): Node | null {
    const id = this._handle.lastChildId();
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  get nextSibling(): Node | null {
    const id = this._handle.nextSiblingId();
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  get previousSibling(): Node | null {
    const id = this._handle.previousSiblingId();
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  /** Live-ish snapshot of children. We materialize the whole array each call. */
  get childNodes(): Node[] {
    return this._handle
      .childIds()
      .map((id) => this._ownerDocument._wrap(id) as Node);
  }

  get hasChildNodes(): boolean {
    return this._handle.firstChildId() !== null;
  }

  // ---- Tree mutation ------------------------------------------------------

  appendChild<T extends Node>(child: T): T {
    this._handle.appendChild(pluckNode(child)._nodeId);
    return child;
  }

  insertBefore<T extends Node>(node: T, anchor: Node | null): T {
    this._handle.insertBefore(
      pluckNode(node)._nodeId,
      anchor === null ? null : pluckNode(anchor)._nodeId,
    );
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    // The spec requires `child.parentNode === this`; we trust callers
    // and let blitz error if invariants are violated.
    pluckNode(child)._handle.remove();
    return child;
  }

  replaceChild<T extends Node>(newChild: Node, oldChild: T): T {
    pluckNode(oldChild)._handle.replaceWith(pluckNode(newChild)._nodeId);
    return oldChild;
  }

  /** Remove this node from its parent. Mirrors `ChildNode.remove`. */
  remove(): void {
    this._handle.remove();
  }

  // ---- Cloning / containment ---------------------------------------------

  /**
   * Standard `Node.cloneNode(deep)`.
   *
   * - `deep=false` (default): copies this node only — same tag, same
   *   attributes, same text/comment payload, but no children. The
   *   clone starts detached (no parent, no owner-list membership).
   * - `deep=true`: copies this node and the entire subtree.
   *
   * The clone shares no mutable state with the original beyond what
   * `Clone` on the underlying `NodeData` shares (Arc-pointered things
   * like the parsed inline-style block — those are immutable from
   * the engine's side and copy-on-write at the property level).
   */
  cloneNode(deep = false): Node {
    const id = deep
      ? this._handle.deepCloneNode()
      : this._handle.shallowCloneNode();
    return this._ownerDocument._wrap(id) as Node;
  }

  contains(other: Node | null): boolean {
    if (other === null) return false;
    let cur: Node | null = other;
    while (cur !== null) {
      if (cur === this) return true;
      cur = cur.parentNode;
    }
    return false;
  }

  /** True iff this node has no JS-visible parent (yet). */
  get isConnected(): boolean {
    // We treat the document root as "connected". The native side currently
    // doesn't expose a quick `is_connected`, so walk up to the root.
    let cur: Node | null = this;
    while (cur !== null) {
      const parentId = cur._handle.parentId();
      if (parentId === null) {
        return cur._handle.nodeType() === NodeTypes.DOCUMENT_NODE;
      }
      cur = cur._ownerDocument._wrap(parentId) as Node;
    }
    return false;
  }
}

// Internals shape declaration: re-export so other modules in this package
// can import the canonical `NodeInternals` from one place.
export type { NodeInternals };
