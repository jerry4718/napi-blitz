// One real-window scenario per file (see window-teardown.spec.ts).
//
// Diagnostic for the multi-window example leak: a listener closure that
// captures its own target window (`child.addEventListener("closed", () =>
// children.filter(w => w !== child))`). The listener table lives in the
// window wrapper's own block as a strong `Anything`, so the closure is a
// GC root; if it captures the window itself the wrapper can never be
// collected.

import {BlitzApp, HTMLDocument, WindowOptions} from "../_shim.ts";
import type {HTMLElement} from "../_shim.ts";
import {pump, testFn} from "../_helpers.ts";
import {isFinalized, track} from "./_gc-helpers.ts";

interface CaptureProbe {
  windowId: string;
  docId: string;
  bodyId: string;
  mountElId: string;
  childDivId: string;
}

async function captureScenario(): Promise<CaptureProbe> {
  const app = BlitzApp.create();
  const document = HTMLDocument.create({
    baseHtml: '<!doctype html><html><head><title>t</title></head><body></body></html>',
  });
  const opening = app.openWindow(document, WindowOptions.builder().size(200, 150));
  pump(app);
  const window = await opening;

  const body = document.body as HTMLElement;
  const mountEl = document.createElement("div");
  body.appendChild(mountEl);
  const childDiv = document.createElement("section");
  mountEl.appendChild(childDiv);

  // The example's leak pattern: the closure captures the window itself.
  window.addEventListener("closed", () => {
    void window;
  });

  const {id: windowId} = track(window);
  const {id: docId} = track(document);
  const {id: bodyId} = track(body);
  const {id: mountElId} = track(mountEl);
  const {id: childDivId} = track(childDiv);

  const closing = window.close();
  pump(app);
  await closing;
  pump(app);
  return {windowId, docId, bodyId, mountElId, childDivId};
}

testFn("capturing listener keeps the window and its wrappers alive", async (t) => {
  const p = await captureScenario();
  // Give the GC a chance, then report who survived.
  for (let i = 0; i < 10; i++) {
    globalThis.gc?.();
    await new Promise((resolve) => setImmediate(resolve));
  }
  const survived = [
    ["window", isFinalized(p.windowId)],
    ["document", isFinalized(p.docId)],
    ["body", isFinalized(p.bodyId)],
    ["mountEl", isFinalized(p.mountElId)],
    ["childDiv", isFinalized(p.childDivId)],
  ];
  const alive = survived.filter(([, fin]) => !fin).map(([name]) => name);
  console.log(`[capture-leak] finalized: ${survived.filter(([, f]) => f).map(([n]) => n).join(", ") || "none"}`);
  console.log(`[capture-leak] still alive: ${alive.join(", ") || "none"}`);
  // The expectation this diagnostic asserts: everything must be collected.
  t.deepEqual(alive, [], "capturing listener must not pin the window graph");
});
