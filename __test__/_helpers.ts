// Shared helpers for the AVA test files. Not picked up by AVA's default
// glob because the filename does not end in `.spec.ts`.

import type { Node } from "../dist/index.js";

/**
 * Shape of a `Node`'s package-private fields, as seen from test code.
 * In the new architecture, nodes no longer expose `_nodeId` - the nodeId
 * lives inside the Rust `NodeHandle` struct. Tests that need the nodeId
 * can call `handle.nodeId()` on the native handle if exposed, or use
 * identity-based comparisons instead.
 */
interface TestNodeInternals {
  readonly _handle: unknown;
  readonly _doc: unknown;
}

/** Read package-private fields off a `Node` instance. */
export function pluckNode(n: Node): TestNodeInternals {
  return n as unknown as TestNodeInternals;
}

/**
 * Shape of a `Document`'s package-private fields used by tests.
 */
interface TestDocumentInternals {
  readonly _native: import("../dist/index.js").NativeDocHandle;
}

/** Read package-private fields off a `Document` instance. */
export function pluckDocument<T extends object>(d: T): TestDocumentInternals {
  return d as unknown as TestDocumentInternals;
}
