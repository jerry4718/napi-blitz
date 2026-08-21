// Shared helpers for the AVA test files. Not picked up by AVA's default
// glob because the filename does not end in `.spec.ts`.

import test from "ava";

import {BlitzApp, HTMLDocument, WindowOptions} from "./_shim.ts";
import type {Node, PumpResult, Window} from "./_shim.ts";

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
  readonly _native: InstanceType<typeof import('./_shim.ts').NativeDoc>;
}

/** Read package-private fields off a `Document` instance. */
export function pluckDocument<T extends object>(d: T): TestDocumentInternals {
  return d as unknown as TestDocumentInternals;
}

// ── Native-window helpers (real OS windows + vello render; CI-skipped) ──

// These cases open real OS windows and render via vello (no GPU in CI), so
// skip when running in CI.
export const testFn = process.env.CI ? test.skip : test;

export function createApp(): BlitzApp {
  return BlitzApp.create();
}

export function newDoc(): HTMLDocument {
  return HTMLDocument.create({
    baseHtml:
      "<!doctype html><html><head><title>t</title></head><body></body></html>",
  });
}

/** Drive one pump so pending window creation / close flushing runs. */
export function pump(app: BlitzApp): PumpResult {
  return app.pumpAppEvents(0);
}

/** Open a window and pump once so the OS window exists and the promise resolves. */
export async function openWindow(app: BlitzApp): Promise<Window> {
  const p = app.openWindow(newDoc(), WindowOptions.builder().size(200, 150));
  pump(app);
  return p;
}

/** Close a window and pump once so the teardown flushes and the promise resolves. */
export async function closeWindow(app: BlitzApp, w: Window): Promise<void> {
  const p = w.close();
  pump(app);
  await p;
}
