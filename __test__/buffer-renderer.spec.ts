import test from "ava";

import { BufferBlitzApp } from "../packages/wasm-blitz/dist/index.js";

test("BufferBlitzApp renders a document into an RGBA frame", (t) => {
  const app = BufferBlitzApp.create({
    width: 64,
    height: 32,
    scale: 1,
    baseHtml: `<!doctype html><html><body><p>Hello buffer</p></body></html>`,
  });

  const frame = app.render();

  t.is(frame.width, 64);
  t.is(frame.height, 32);
  t.is(frame.scale, 1);
  t.true(frame.data instanceof Uint8Array);
  t.is(frame.data.byteLength, 64 * 32 * 4);
});
