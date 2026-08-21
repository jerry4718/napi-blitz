// Case: the app blocks openWindow by preventing the cancelable app-level
// `window:open` event. The openWindow promise must reject and no Window is
// handed out.

import {createApp, newDoc, pump, testFn} from "../_helpers.ts";
import {WindowOptions} from "../_shim.ts";

testFn("app can block openWindow via window:open preventDefault", async (t) => {
  const app = createApp();
  const block = (e: Event): void => {
    e.preventDefault();
  };
  app.addEventListener("window:open", block);
  const p = app.openWindow(newDoc(), WindowOptions.builder().size(200, 150));
  pump(app); // native creates the window, dispatches window:open -> rejected
  await t.throwsAsync(() => p, {message: /window open prevented/});
  app.removeEventListener("window:open", block);
});
