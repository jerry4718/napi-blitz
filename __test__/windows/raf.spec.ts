// `window.requestAnimationFrame` / `cancelAnimationFrame` on real OS
// windows (openWindow + pump -> winit RedrawRequested -> frame before the
// render). One app per process, because a process holds exactly one winit
// EventLoop — openWindow only adds a window to that loop (events fan out
// by window id), so every test shares the one app and opens its own
// window. CI containers lack GPU support, so these are CI-skipped.

import {closeWindow, createApp, openWindow, pump, testFn} from "../_helpers.ts";
import type {Window} from "../_shim.ts";

let app: ReturnType<typeof createApp> | undefined;

function sharedApp(): ReturnType<typeof createApp> {
  return (app ??= createApp());
}

function pumpUntil(app: ReturnType<typeof createApp>, cond: () => boolean): void {
  for (let i = 0; i < 40 && !cond(); i++) {
    pump(app);
  }
}

testFn("requestAnimationFrame runs once with a numeric timestamp on the next redraw", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  const calls: number[] = [];
  const handle = window.requestAnimationFrame((ts) => calls.push(ts));
  t.true(typeof handle === "number" && handle > 0, "handle is a positive number");

  pumpUntil(winApp, () => calls.length === 1);
  t.is(calls.length, 1);
  t.true(typeof calls[0] === "number" && calls[0] >= 0, "timestamp is a number");

  // A callback that does not re-register never fires again.
  pump(winApp);
  pump(winApp);
  t.is(calls.length, 1);

  await closeWindow(winApp, window);
});

testFn("cancelled animation frames never run", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  let ran = 0;
  const handle = window.requestAnimationFrame(() => ran++);
  window.cancelAnimationFrame(handle);
  pump(winApp);
  pump(winApp);
  t.is(ran, 0);

  await closeWindow(winApp, window);
});

testFn("multiple registrations run in order within one frame", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  const log: string[] = [];
  window.requestAnimationFrame(() => log.push("a"));
  window.requestAnimationFrame(() => log.push("b"));
  pumpUntil(winApp, () => log.length === 2);

  t.deepEqual(log, ["a", "b"]);

  await closeWindow(winApp, window);
});

testFn("re-registering inside a callback runs on the next frame", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);

  let frame = 0;
  window.requestAnimationFrame(() => {
    frame++;
    window.requestAnimationFrame(() => frame++);
  });
  pumpUntil(winApp, () => frame === 1);
  t.is(frame, 1, "the re-registered callback must wait for the next frame");
  pumpUntil(winApp, () => frame === 2);
  t.is(frame, 2);

  await closeWindow(winApp, window);
});

testFn("requestAnimationFrame on a closed window throws", async (t) => {
  const winApp = sharedApp();
  const window = await openWindow(winApp);
  await closeWindow(winApp, window);

  t.throws(() => window.requestAnimationFrame(() => {}), {
    message: /window is closed/,
  });
});