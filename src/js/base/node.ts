// `Node` — abstract base class for every node in our DOM. Concrete
// subclasses are `Element` (with `HTMLElement` etc. on top), `Text`,
// `Comment`, and `Document`.
//
// Closely mirrors the web `Node` interface. Each Node holds:
//   - `_handle`: the native DocHandle, used for every DOM op
//   - `_nodeId`: blitz's internal id
//   - `_ownerDocument`: the JS Document this node belongs to. Used for
//     `_wrap`-based reverse lookups when returning related nodes.
//
// We keep the underscore + `protected` style instead of TS `#` so the
// internal hatch in `internal.ts` keeps working.

import type { NativeDocHandle } from "../native";
import { pluckNode, type DocumentInternals, type NodeInternals } from "../internal/internal";

/** DOM nodeType constants. Mirrors the web spec. */
export const NodeTypes = {
  ELEMENT_NODE: 1,
  TEXT_NODE: 3,
  COMMENT_NODE: 8,
  DOCUMENT_NODE: 9,
} as const;

export abstract class Node extends EventTarget {
  protected readonly _handle: NativeDocHandle;
  protected readonly _nodeId: number;
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
    handle: NativeDocHandle,
    nodeId: number,
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

  /** DOM-style numeric nodeType. */
  get nodeType(): number {
    return this._handle.nodeType(this._nodeId);
  }

  /**
   * Concatenated text content of this node and its descendants. Setter
   * resets to a single text-node child for elements; for Text/Comment
   * it updates `data` directly.
   */
  get textContent(): string | null {
    return this._handle.textContent(this._nodeId);
  }
  set textContent(value: string) {
    this._handle.setTextContent(this._nodeId, value);
  }

  // ---- Tree relationships -------------------------------------------------

  get parentNode(): Node | null {
    const id = this._handle.parentId(this._nodeId);
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  /** Same as `parentNode` for now; differs from spec only for non-Element parents. */
  get parentElement(): Node | null {
    return this.parentNode;
  }

  get firstChild(): Node | null {
    const id = this._handle.firstChildId(this._nodeId);
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  get lastChild(): Node | null {
    const id = this._handle.lastChildId(this._nodeId);
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  get nextSibling(): Node | null {
    const id = this._handle.nextSiblingId(this._nodeId);
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  get previousSibling(): Node | null {
    const id = this._handle.previousSiblingId(this._nodeId);
    return id === null ? null : (this._ownerDocument._wrap(id) as Node);
  }

  /** Live-ish snapshot of children. We materialize the whole array each call. */
  get childNodes(): Node[] {
    return this._handle
      .childIds(this._nodeId)
      .map((id) => this._ownerDocument._wrap(id) as Node);
  }

  get hasChildNodes(): boolean {
    return this._handle.firstChildId(this._nodeId) !== null;
  }

  // ---- Tree mutation ------------------------------------------------------

  appendChild<T extends Node>(child: T): T {
    this._handle.appendChild(this._nodeId, pluckNode(child)._nodeId);
    return child;
  }

  insertBefore<T extends Node>(node: T, anchor: Node | null): T {
    this._handle.insertBefore(
      this._nodeId,
      pluckNode(node)._nodeId,
      anchor === null ? null : pluckNode(anchor)._nodeId,
    );
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    // The spec requires `child.parentNode === this`; we trust callers
    // and let blitz error if invariants are violated.
    this._handle.remove(pluckNode(child)._nodeId);
    return child;
  }

  replaceChild<T extends Node>(newChild: Node, oldChild: T): T {
    this._handle.replaceWith(pluckNode(oldChild)._nodeId, pluckNode(newChild)._nodeId);
    return oldChild;
  }

  /** Remove this node from its parent. Mirrors `ChildNode.remove`. */
  remove(): void {
    this._handle.remove(this._nodeId);
  }

  // ---- Cloning / containment ---------------------------------------------

  cloneNode(deep = false): Node {
    if (!deep) {
      // We don't yet expose a shallow-clone primitive on the native side.
      // Until then, deep-clone is the only safe option.
      // TODO: add `cloneNodeShallow` in Rust and wire here.
    }
    const id = this._handle.deepCloneNode(this._nodeId);
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
      const parentId = cur._handle.parentId(cur._nodeId);
      if (parentId === null) {
        return cur._handle.nodeType(cur._nodeId) === NodeTypes.DOCUMENT_NODE;
      }
      cur = cur._ownerDocument._wrap(parentId) as Node;
    }
    return false;
  }
}

// Internals shape declaration: re-export so other modules in this package
// can import the canonical `NodeInternals` from one place.
export type { NodeInternals };
