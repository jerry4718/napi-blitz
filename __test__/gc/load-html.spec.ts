// One real-window scenario per file (see window-teardown.spec.ts).
//
// `window.loadHtml` swaps in a fresh document the way assigning
// `location.href` would: the retired document's wrappers must be
// collectible, and the window must carry a new document object whose
// tree reflects the parsed HTML.

import test from "ava";

import {BlitzApp, HTMLDocument, WindowOptions} from "../_shim.ts";
import type {HTMLDocument, HTMLElement} from "../_shim.ts";
import {pump, testFn} from "../_helpers.ts";
import {track, waitForFinalization} from "./_gc-helpers.ts";

interface NavigateProbe {
  oldDocId: string;
  oldDivId: string;
  freshIsNewObject: boolean;
  freshIsReturnedObject: boolean;
  freshText: string;
}

async function navigateAndProbe(): Promise<NavigateProbe> {
  const app = BlitzApp.create();
  const document = HTMLDocument.create({
    baseHtml: '<!doctype html><html><head></head><body><div id="a">old</div></body></html>',
  });
  const opening = app.openWindow(document, WindowOptions.builder().size(200, 150));
  pump(app);
  const window = await opening;

  const oldDocId = track(document).id;
  const oldDivId = track(document.querySelector("#a") as HTMLElement).id;

  const returnedDoc = window.loadHtml(
    '<!doctype html><html><head></head><body><div id="a">new</div></body></html>',
  ) as HTMLDocument;
  pump(app);

  const freshDoc = window.document as HTMLDocument;
  const freshIsNewObject = freshDoc !== document;
  const freshIsReturnedObject = returnedDoc === freshDoc;
  const freshText = (freshDoc.querySelector("#a") as HTMLElement | undefined)?.textContent ?? null;

  const closing = window.close();
  pump(app);
  await closing;
  return {oldDocId, oldDivId, freshIsNewObject, freshIsReturnedObject, freshText};
}

testFn("loadHtml swaps in a fresh document and retires the old one", async (t) => {
  const {oldDocId, oldDivId, freshIsNewObject, freshIsReturnedObject, freshText} =
    await navigateAndProbe();
  t.true(freshIsNewObject, "window.document must be a fresh document object after loadHtml");
  t.true(freshIsReturnedObject, "loadHtml must return the same wrapper as window.document");
  t.is(freshText, "new");
  t.true(await waitForFinalization(oldDivId), "old document's node wrapper was never finalized");
  t.true(await waitForFinalization(oldDocId), "retired document wrapper was never finalized");
});
