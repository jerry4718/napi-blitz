// rAF retention diagnostics on real OS windows: the queue holds each
// callback as a strong napi reference until it runs, is cancelled, or the
// window closes, so a payload captured by the callback must be collectible
// through every one of those paths (the callback itself would be a GC root
// while queued). One app per process (one winit EventLoop): the app is
// created once, and each test opens its own window on it. CI-skipped.

import type {BlitzApp, Window} from "../_shim.ts";
import {closeWindow, createApp, openWindow, pump, testFn} from "../_helpers.ts";
import {isFinalized, track} from "./_gc-helpers.ts";

let app: BlitzApp | undefined;

function sharedApp(): BlitzApp {
  return (app ??= createApp());
}

async function gcSweep(): Promise<void> {
  for (let i = 0; i < 10; i++) {
    globalThis.gc?.();
    await new Promise((resolve) => setImmediate(resolve));
  }
}

testFn("payload captured by an executed rAF callback is collected after the frame", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  let payload: {mark: number} | undefined = {mark: 1};
  const {id} = track(payload);
  let fired = false;
  window.requestAnimationFrame(() => {
    void payload;
    fired = true;
  });
  for (let i = 0; i < 40 && !fired; i++) {
    pump(winApp);
  }
  t.true(fired, "frame callback ran");
  // The callback has run and its strong reference is gone; drop the local
  // strong reference so only the (dead) closure could still hold it.
  payload = undefined;

  await gcSweep();
  t.true(isFinalized(id), "payload captured by an executed rAF callback must be collected");

  await closeWindow(winApp, window);
});

testFn("payload captured by a cancelled rAF callback is collected", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  let payload: {mark: number} | undefined = {mark: 1};
  const {id} = track(payload);
  const handle = window.requestAnimationFrame(() => {
    void payload;
  });
  window.cancelAnimationFrame(handle);
  payload = undefined;

  // A few pumps to make sure cancellation silently holds against any
  // scheduled redraw; the callback must never fire.
  for (let i = 0; i < 4; i++) {
    pump(winApp);
  }
  await gcSweep();
  t.true(isFinalized(id), "payload captured by a cancelled rAF callback must be collected");

  await closeWindow(winApp, window);
});

testFn("payload captured by a queued rAF callback is collected when the window closes", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  let payload: {mark: number} | undefined = {mark: 1};
  const {id} = track(payload);
  // Register and never run it: the window close must release the queue.
  window.requestAnimationFrame(() => {
    void payload;
  });
  payload = undefined;

  await closeWindow(winApp, window);
  await gcSweep();
  t.true(isFinalized(id), "payload captured by a queued rAF callback must be collected on window close");
});