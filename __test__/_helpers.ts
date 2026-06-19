// Shared helpers for the AVA test files. Not picked up by AVA's default
// glob because the filename does not end in `.spec.ts`.
//
// Mirrors the `pluck*` pattern from `src/js/internal/internal.ts`: tests
// occasionally need to read the package-private `_nodeId` off a Node so
// they can fabricate an `EventPayload` that targets a specific element
// — exactly what the native bridge would emit. Centralizing the
// `as unknown as Internals` cast here keeps the spec files readable.

import type { Node, EventPayload } from "../dist/index.js";

/** Shape of a `Node`'s package-private fields, as seen from test code. */
interface TestNodeInternals {
  readonly _nodeId: bigint;
}

/** Read package-private fields off a `Node` instance. */
export function pluckNode(n: Node): TestNodeInternals {
  return n as unknown as TestNodeInternals;
}

/** Convenience: just the nodeId. */
export function nodeIdOf(n: Node): bigint {
  return pluckNode(n)._nodeId;
}

/**
 * Build an `EventPayload` that mimics what `Rust -> JS` would send for
 * a `click`. `chain` is the bubble path from the deepest target up to
 * (but not including) the document.
 */
export function makeClickPayload(
  targetId: bigint,
  chain: bigint[],
): EventPayload {
  return {
    eventType: "click",
    target: targetId,
    chain,
    bubbles: true,
    cancelable: true,
    pointer: undefined,
    wheel: undefined,
    key: undefined,
    input: undefined,
    ime: undefined,
  };
}

/**
 * Shape of a `Document`'s package-private fields used by tests that
 * need to drive the dispatch path manually.
 */
interface TestDocumentInternals {
  readonly _native: import("../dist/index.js").NativeDocHandle;
  _dispatchFromNative(payload: EventPayload): {
    defaultPrevented: boolean;
    propagationStopped: boolean;
    requestRedraw: boolean;
  };
}

/** Read package-private fields off a `Document` instance. */
export function pluckDocument<T extends object>(d: T): TestDocumentInternals {
  return d as unknown as TestDocumentInternals;
}
