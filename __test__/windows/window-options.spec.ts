// Window resize validation at the JS -> N-API boundary, on a real OS window
// (openWindow + pump -> vello render). CI containers lack GPU support (same
// as dom-mutation-style.spec.ts), so it is CI-skipped via the shared `testFn`.

import {closeWindow, createApp, newDoc, pump, testFn} from "../_helpers.ts";
import {WindowOptions} from "../_shim.ts";

testFn("window resize dimensions are validated at the napi boundary", async (t) => {
  const app = createApp();
  const valid = WindowOptions.builder();
  valid.size(100, 100);
  const winPromise = app.openWindow(newDoc(), valid);
  pump(app); // create the window, resolve the openWindow promise
  const window = await winPromise;

  t.throws(() => window.resize(-1, 100), {
    message: /width must be >= 1/,
  });
  t.throws(() => window.resize(100, 50.25), {
    message: /height must be an integer/,
  });

  await closeWindow(app, window);
});
