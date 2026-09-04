// One real-window scenario per file (see window-teardown.spec.ts).
//
// This file: an `openWindow` request that has not been pumped yet. The
// pending request owns the `SharedDocument`, so the Document wrapper must
// stay alive until the request is drained; after the window is closed and
// torn down every strong JS reference must be gone (the helper returns
// before the GC loop), and the wrapper must be finalizable.

import test from "ava";

import {BlitzApp, HTMLDocument, WindowOptions} from "../_shim.ts";
import {pump, testFn} from "../_helpers.ts";
import {requireGc, track, waitForFinalization} from "./_gc-helpers.ts";

interface PendingThenClosed {
  id: string;
  aliveWhilePending: boolean;
}

async function pendingThenClosed(): Promise<PendingThenClosed> {
  const app = BlitzApp.create();
  const document = HTMLDocument.create({
    baseHtml: "<!doctype html><html><head></head><body></body></html>",
  });
  const {id, weak} = track(document);

  const opening = app.openWindow(document, WindowOptions.builder().size(200, 150));

  const gc = requireGc();
  gc();
  await new Promise<void>((resolve) => setImmediate(resolve));
  const aliveWhilePending = weak.deref() !== undefined;

  pump(app);
  const window = await opening;
  const closing = window.close();
  pump(app);
  await closing;
  return {id, aliveWhilePending};
}

testFn("pending open keeps its Document alive until the request is pumped", async (t) => {
  const {id, aliveWhilePending} = await pendingThenClosed();
  t.true(aliveWhilePending, "pending-open Document must stay alive while the open request is queued");
  t.true(await waitForFinalization(id), "Document wrapper was never finalized after the window was closed");
});
