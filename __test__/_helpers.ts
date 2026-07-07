// Shared helpers for the AVA test files. Not picked up by AVA's default
// glob because the filename does not end in `.spec.ts`.
//
// Mirrors the `pluck*` pattern from package `src/internal/internal.ts`: tests
// occasionally need to read the package-private `_nodeId` off a Node so
// they can fabricate an `EventPayload` that targets a specific element
// — exactly what the native bridge would emit. Centralizing the
// `as unknown as Internals` cast here keeps the spec files readable.

import type { Node, EventPayload } from "../packages/napi-blitz/dist/index.js";

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
 * a single dispatch step of a `click`. `receiver` is the node currently
 * receiving the event, `phase` is 1=capture, 2=target, 3=bubble.
 */
export function makeClickPayload(
  targetId: bigint,
  receiverId: bigint,
  phase: number,
): EventPayload {
  return {
    eventType: "click",
    target: targetId,
    receiver: receiverId,
    phase,
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
 * Drive a full capture → target → bubble dispatch by calling
 * `_dispatchFromNative` for each step. `chain` is the propagation path
 * from the target up to (but not including) the document.
 *
 * Returns the merged `DispatchResult` (flags OR-ed across all steps).
 */
export function dispatchChain(
  doc: TestDocumentInternals,
  targetId: bigint,
  chain: bigint[],
): { defaultPrevented: boolean; propagationStopped: boolean; requestRedraw: boolean } {
  let result = { defaultPrevented: false, propagationStopped: false, requestRedraw: false };

  const merge = (r: { defaultPrevented: boolean; propagationStopped: boolean; requestRedraw: boolean }) => {
    result.defaultPrevented = result.defaultPrevented || r.defaultPrevented;
    result.propagationStopped = result.propagationStopped || r.propagationStopped;
    result.requestRedraw = result.requestRedraw || r.requestRedraw;
  };

  // Capture phase (root → target's parent), reversed
  for (let i = chain.length - 1; i >= 1; i--) {
    merge(doc._dispatchFromNative(makeClickPayload(targetId, chain[i], 1)));
    if (result.propagationStopped) return result;
  }

  // Target phase
  merge(doc._dispatchFromNative(makeClickPayload(targetId, targetId, 2)));
  if (result.propagationStopped) return result;

  // Bubble phase (target's parent → root)
  for (let i = 1; i < chain.length; i++) {
    merge(doc._dispatchFromNative(makeClickPayload(targetId, chain[i], 3)));
    if (result.propagationStopped) return result;
  }

  return result;
}

/**
 * Shape of a `Document`'s package-private fields used by tests that
 * need to drive the dispatch path manually.
 */
interface TestDocumentInternals {
  readonly _native: import("../packages/napi-blitz/dist/index.js").NativeDocHandle;
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
