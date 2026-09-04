// One real-window scenario per file: `BlitzApp.create()` builds a winit
// event loop, and blitz's shell refuses a second loop in the same process
// (`RecreationAttempt`), so lifecycle cases cannot share an AVA worker.
//
// This file: after a real openWindow + pump + close + pump teardown, the
// Document wrapper must be finalizable. It currently passes: the teardown
// path does release the native pin on the document.

import test from "ava";

import {BlitzApp, HTMLDocument, WindowOptions} from "../_shim.ts";
import {pump, testFn} from "../_helpers.ts";
import {track, waitForFinalization} from "./_gc-helpers.ts";

// Open, close, and tear down a window; drop every strong JS reference
// (document, window, promise handles) before returning, so the async test
// frame holds nothing while the GC loop runs.
async function openedAndClosedDocument(): Promise<string> {
  const app = BlitzApp.create();
  const document = HTMLDocument.create({
    baseHtml: "<!doctype html><html><head></head><body></body></html>",
  });
  const id = track(document).id;

  const opening = app.openWindow(document, WindowOptions.builder().size(200, 150));
  pump(app);
  const window = await opening;
  const closing = window.close();
  pump(app);
  await closing;
  return id;
}

testFn("Document is finalized after a real window open/close teardown", async (t) => {
  const id = await openedAndClosedDocument();
  t.true(await waitForFinalization(id), "Document wrapper was never finalized after window teardown");
});
