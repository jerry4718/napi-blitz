// Window size validation at the JS -> N-API boundary. Only the pure
// parameter-validation cases live here (no pump / no real window), so they
// run everywhere, including CI. The resize case that needs a real OS window
// + vello rendering lives in `windows/window-options.spec.ts` (CI-skipped).

import test from "ava";

import {createApp, newDoc} from "./_helpers.ts";
import {WindowOptions} from "./_shim.ts";

const app = createApp();

test("window surface dimensions are validated before reaching winit", async (t) => {
  const negative = WindowOptions.builder();
  negative.size(-1, 100);
  await t.throwsAsync(() => app.openWindow(newDoc(), negative), {
    message: /width must be >= 1/,
  });

  const fractional = WindowOptions.builder();
  fractional.size(100.5, 100);
  await t.throwsAsync(() => app.openWindow(newDoc(), fractional), {
    message: /width must be an integer/,
  });

  const infinite = WindowOptions.builder();
  infinite.size(Number.POSITIVE_INFINITY, 100);
  await t.throwsAsync(() => app.openWindow(newDoc(), infinite), {
    message: /width must be finite/,
  });
});
