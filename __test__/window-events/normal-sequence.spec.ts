// Case: a normal (unprevented) open/close fires the lifecycle events in
// order: app `window:open` (before openWindow resolves), then after the
// close teardown the window's `closed` and the app's `window:closed`.

import {closeWindow, createApp, openWindow, testFn} from "../_helpers.ts";

testFn("normal open/close dispatches lifecycle events in order", async (t) => {
  const app = createApp();
  const events: string[] = [];
  app.addEventListener("window:open", () => events.push("app:window:open"));
  app.addEventListener("window:closed", () => events.push("app:window:closed"));
  const w = await openWindow(app);
  w.addEventListener("closed", () => events.push("win:closed"));
  t.is(w.closed, false);
  await closeWindow(app, w);
  t.deepEqual(events, ["app:window:open", "win:closed", "app:window:closed"]);
  t.true(w.closed);
});
