// Case: the app blocks closeWindow by preventing the cancelable app-level
// `window:close` event (bubbled up from the window's `close`). The
// closeWindow promise must reject and the window stays open.

import {closeWindow, createApp, openWindow, pump, testFn} from "../_helpers.ts";

testFn("app can block closeWindow via window:close preventDefault", async (t) => {
  const app = createApp();
  const w = await openWindow(app);
  const block = (e: Event): void => {
    e.preventDefault();
  };
  app.addEventListener("window:close", block);
  const p = app.closeWindow(w);
  pump(app);
  await t.throwsAsync(() => p, {message: /close prevented/});
  t.is(w.closed, false); // window stays open
  app.removeEventListener("window:close", block);
  await closeWindow(app, w); // not blocked anymore
  t.true(w.closed);
});
