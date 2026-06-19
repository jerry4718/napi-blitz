// Window size validation at the JS -> N-API boundary.

import test from "ava";

import { BlitzApp } from "../packages/napi-blitz/dist/index.js";

test("window surface dimensions are validated before reaching winit", (t) => {
  const app = BlitzApp.create();

  t.throws(() => app.openWindow({ width: -1, height: 100 }), {
    message: /width must be >= 1/,
  });
  t.throws(() => app.openWindow({ width: 100.5, height: 100 }), {
    message: /width must be an integer/,
  });
  t.throws(() => app.openWindow({ width: Number.POSITIVE_INFINITY, height: 100 }), {
    message: /width must be finite/,
  });
  t.throws(() => app.openWindow({ width: 100 }), {
    message: /width and height must be provided together/,
  });

  const window = app.openWindow({ width: 100, height: 100 });

  t.throws(() => window.resize(-1, 100), {
    message: /width must be >= 1/,
  });
  t.throws(() => window.resize(100, 50.25), {
    message: /height must be an integer/,
  });

  window.close();
});
