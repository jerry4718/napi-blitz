// Internal helpers shared across the JS-side DOM classes.
//
// TypeScript has no real package-private visibility, so we put one
// canonical "friend" hatch here. Every class that needs to peek at
// another class's underscore-prefixed fields uses `pluck()` rather
// than asserting `as unknown as Internals` inline.
//
// Keeping this in one module also makes it easy to grep for who
// depends on internals when we tighten encapsulation later.

import type { NativeDocHandle } from "../native";

/** Shape of a `Node`'s package-private fields. */
export interface NodeInternals {
  readonly _handle: NativeDocHandle;
  readonly _nodeId: number;
  readonly _ownerDocument: DocumentInternals;
}

/** Shape of a `Document`'s package-private fields, viewed by friends. */
export interface DocumentInternals {
  readonly _native: NativeDocHandle;
  /**
   * Get-or-create a wrapper for a known-existing nodeId. Returns the
   * concrete Node subclass (Element/Text/Comment/...) wrapped as a
   * generic object; callers cast to the type they need.
   */
  _wrap(nodeId: number): object;
}

/** Read the package-private fields off a `Node` instance. */
export function pluckNode<T extends object>(n: T): NodeInternals {
  return n as unknown as NodeInternals;
}

/** Read the package-private fields off a `Document` instance. */
export function pluckDocument<T extends object>(d: T): DocumentInternals {
  return d as unknown as DocumentInternals;
}
