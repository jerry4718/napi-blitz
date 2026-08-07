// Window size validation at the JS -> N-API boundary.

import test from "ava";

import { BlitzApp, HTMLDocument, WindowOptions } from "../dist/index.js";

// winit only allows one event loop per process, so share a single
// BlitzApp across all tests in this file.
const app = BlitzApp.create();
const newDoc = () => HTMLDocument.create();

test("window surface dimensions are validated before reaching winit", (t) => {
  const negative = WindowOptions.builder();
  negative.size(-1, 100);
  t.throws(() => app.openWindow(newDoc(), negative), {
    message: /width must be >= 1/,
  });

  const fractional = WindowOptions.builder();
  fractional.size(100.5, 100);
  t.throws(() => app.openWindow(newDoc(), fractional), {
    message: /width must be an integer/,
  });

  const infinite = WindowOptions.builder();
  infinite.size(Number.POSITIVE_INFINITY, 100);
  t.throws(() => app.openWindow(newDoc(), infinite), {
    message: /width must be finite/,
  });
});

// openWindow with valid dimensions triggers pump_app_events -> vello
// rendering. CI containers lack GPU support (same as
// dom-mutation-style.spec.ts), so skip in CI.
const testFn = process.env.CI ? test.skip : test;

testFn("window resize dimensions are validated at the napi boundary", (t) => {
  const valid = WindowOptions.builder();
  valid.size(100, 100);
  const window = app.openWindow(newDoc(), valid);

  t.throws(() => window.resize(-1, 100), {
    message: /width must be >= 1/,
  });
  t.throws(() => window.resize(100, 50.25), {
    message: /height must be an integer/,
  });

  window.close();
});
