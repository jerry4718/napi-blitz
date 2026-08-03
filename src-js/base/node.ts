// `Node` — abstract base class for every node in our DOM. Concrete
// subclasses are `Element` (with `HTMLElement` etc. on top), `Text`,
// `Comment`, and `Document`.
//
// In the new architecture:
//   - JS Node only holds a `NodeHandle` (native handle). No nodeId.
//   - nodeId lives inside the Rust `NodeHandle` struct, invisible to JS.
//   - JS constructor only stores handle + doc reference. No data creation.
//   - All getters forward to the native handle.
//   - Tree relationship getters return already-wrapped JS Node objects
//     (Rust does the wrapping via NodeCache).
//   - EventTarget is fully inherited from Node.js built-in — no override.

import type { NativeNodeHandle } from "../native";
import type { DocumentInternals, NodeInternals } from "../internal/internal";

/** DOM nodeType constants. Mirrors the web spec. */
export const NodeTypes = {
  ELEMENT_NODE: 1,
  TEXT_NODE: 3,
  COMMENT_NODE: 8,
  DOCUMENT_NODE: 9,
} as const;

export abstract class Node extends EventTarget {
  protected readonly _handle: NativeNodeHandle;
  protected readonly _doc: DocumentInternals;

  /**
   * @internal Constructed by Rust via `registerNodeConstructor`. JS
   * never calls `new Node(...)` directly.
   */
  constructor(handle: NativeNodeHandle, doc: DocumentInternals) {
    super();
    this._handle = handle;
    this._doc = doc;
  }

  /** DOM-style numeric nodeType. */
  get nodeType(): number {
    return this._handle.nodeType();
  }

  get textContent(): string | null {
    return this._handle.textContent();
  }
  set textContent(value: string) {
    this._handle.setTextContent(value);
  }

  // ---- Tree relationships -------------------------------------------------
  // All return already-wrapped JS Node objects from Rust (NodeCache).

  get parentNode(): Node | null {
    return this._handle.parentNode() as Node | null;
  }

  get parentElement(): Node | null {
    return this.parentNode;
  }

  get firstChild(): Node | null {
    return this._handle.firstChild() as Node | null;
  }

  get lastChild(): Node | null {
    return this._handle.lastChild() as Node | null;
  }

  get nextSibling(): Node | null {
    return this._handle.nextSibling() as Node | null;
  }

  get previousSibling(): Node | null {
    return this._handle.previousSibling() as Node | null;
  }

  get childNodes(): Node[] {
    return this._handle.childNodes() as Node[];
  }

  get hasChildNodes(): boolean {
    return this._handle.firstChild() !== null;
  }

  // ---- Tree mutation ------------------------------------------------------
  // Pass child._handle — Rust extracts nodeId internally.

  appendChild<T extends Node>(child: T): T {
    this._handle.appendChild(child._handle);
    return child;
  }

  insertBefore<T extends Node>(node: T, anchor: Node | null): T {
    this._handle.insertBefore(
      node._handle,
      anchor === null ? null : anchor._handle,
    );
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    child._handle.remove();
    return child;
  }

  replaceChild<T extends Node>(newChild: Node, oldChild: T): T {
    oldChild._handle.replaceWith(newChild._handle);
    return oldChild;
  }

  /** Remove this node from its parent. Mirrors `ChildNode.remove`. */
  remove(): void {
    this._handle.remove();
  }

  // ---- Cloning / containment ---------------------------------------------

  cloneNode(deep = false): Node {
    return this._handle.cloneNode(deep) as Node;
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

  get isConnected(): boolean {
    let cur: Node | null = this;
    while (cur !== null) {
      const parent: Node | null = cur.parentNode;
      if (parent === null) {
        return cur.nodeType === NodeTypes.DOCUMENT_NODE;
      }
      cur = parent;
    }
    return false;
  }
}

// Internals shape declaration: re-export so other modules in this package
// can import the canonical `NodeInternals` from one place.
export type { NodeInternals };
