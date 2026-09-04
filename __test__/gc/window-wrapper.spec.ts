// One real-window scenario per file (see window-teardown.spec.ts).
//
// This file: the Window JS wrapper itself after its native window is torn
// down. The open/close flow runs in a helper that drops every strong JS
// reference before returning, so the async test frame holds nothing while
// the GC loop runs. Nothing on the native side is expected to hold the
// Window wrapper strongly after teardown (`js_window_ref` is weak), so it
// should be finalizable.

import test from "ava";

import {BlitzApp, HTMLDocument, WindowOptions} from "../_shim.ts";
import type {Window} from "../_shim.ts";
import {pump, testFn} from "../_helpers.ts";
import {track, waitForFinalization} from "./_gc-helpers.ts";

async function openedAndClosedWindow(): Promise<string> {
  const app = BlitzApp.create();
  const document = HTMLDocument.create({
    baseHtml: "<!doctype html><html><head></head><body></body></html>",
  });

  const opening = app.openWindow(document, WindowOptions.builder().size(200, 150));
  pump(app);
  const window: Window = await opening;
  const id = track(window).id;

  const closing = window.close();
  pump(app);
  await closing;
  return id;
}

testFn("Window wrapper is finalized after its native window is torn down", async (t) => {
  const id = await openedAndClosedWindow();
  t.true(await waitForFinalization(id), "Window wrapper was never finalized after teardown");
});
