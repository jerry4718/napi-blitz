// Case: the window blocks its own close by preventing the cancelable
// window-level `close` event. The close() promise must reject and the
// window stays open.

import {closeWindow, createApp, openWindow, pump, testFn} from "../_helpers.ts";

testFn("window can block its own close via close preventDefault", async (t) => {
  const app = createApp();
  const w = await openWindow(app);
  const block = (e: Event): void => {
    e.preventDefault();
  };
  w.addEventListener("close", block);
  const p = w.close();
  pump(app);
  await t.throwsAsync(() => p, {message: /close prevented/});
  t.is(w.closed, false); // window stays open
  w.removeEventListener("close", block);
  await closeWindow(app, w); // not blocked anymore
  t.true(w.closed);
});
