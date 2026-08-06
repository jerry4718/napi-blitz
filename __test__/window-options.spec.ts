// Window size validation at the JS -> N-API boundary.

import test from "ava";

import { BlitzApp, HTMLDocument, WindowOptions } from "../dist/index.js";

test("window surface dimensions are validated before reaching winit", (t) => {
  const app = BlitzApp.create();
  const document = () => HTMLDocument.create();

  const negative = WindowOptions.builder();
  negative.size(-1, 100);
  t.throws(() => app.openWindow(document(), negative), {
    message: /width must be >= 1/,
  });

  const fractional = WindowOptions.builder();
  fractional.size(100.5, 100);
  t.throws(() => app.openWindow(document(), fractional), {
    message: /width must be an integer/,
  });

  const infinite = WindowOptions.builder();
  infinite.size(Number.POSITIVE_INFINITY, 100);
  t.throws(() => app.openWindow(document(), infinite), {
    message: /width must be finite/,
  });

  const valid = WindowOptions.builder();
  valid.size(100, 100);
  const window = app.openWindow(document(), valid);

  t.throws(() => window.resize(-1, 100), {
    message: /width must be >= 1/,
  });
  t.throws(() => window.resize(100, 50.25), {
    message: /height must be an integer/,
  });

  window.close();
});
