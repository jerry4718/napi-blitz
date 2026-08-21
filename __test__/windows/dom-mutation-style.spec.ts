// JS-created element subtrees matching ancestor descendant selectors, on a
// real OS window (pump -> vello render). CI containers lack GPU support
// (vello shaders need float16 capabilities), so it is CI-skipped via the
// shared `testFn`.

import {HTMLDocument, WindowOptions} from "../_shim.ts";
import {closeWindow, createApp, pump, testFn} from "../_helpers.ts";

testFn('JS-created element subtrees can match ancestor descendant selectors without panicking', async (t) => {
  const app = createApp();
  const document = HTMLDocument.create({
    baseHtml:
      '<!doctype html><html><head><title>x</title><style>.page-header h1 { margin: 0 0 8px; }</style></head><body></body></html>',
  });
  const options = WindowOptions.builder();
  options.size(320, 240);
  const winPromise = app.openWindow(document, options);
  pump(app); // create the window, resolve the openWindow promise
  const window = await winPromise;

  const header = document.createElement('header');
  header.setAttribute('class', 'page-header');
  const h1 = document.createElement('h1');
  h1.textContent = 'Title';
  header.appendChild(h1);
  document.body!.appendChild(header);

  const result = pump(app);
  t.false(result.exit);
  await closeWindow(app, window);
});
