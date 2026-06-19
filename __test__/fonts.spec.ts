// CSS Font Loading API: `FontFace` and `document.fonts` (FontFaceSet).
//
// We don't ship a real font file in tests, so registration with valid
// font data is exercised manually via the example apps. These tests
// cover: constructor validation, descriptor defaults, status
// transitions, the URL-source error path, and the JS-visible behavior
// of `FontFaceSet` (membership / iteration / size / has / delete).
//
// One regression guard does push real bytes through the native path:
// passing garbage bytes must not crash the engine — it should report
// "0 faces registered" via the Rust return value (fontique ignores
// data it cannot parse).

import test from "ava";

import { FontFace, FontFaceSet, HTMLDocument } from "../dist/index.js";

test("FontFace constructor validates family name", (t) => {
  const buf = new Uint8Array([0, 0, 0, 0]);
  t.throws(() => new FontFace("", buf), { instanceOf: TypeError });
  // Non-string also rejected.
  t.throws(
    () => new FontFace(undefined as unknown as string, buf),
    { instanceOf: TypeError },
  );
});

test("FontFace exposes spec descriptors with defaults", (t) => {
  const buf = new Uint8Array(4);
  const face = new FontFace("X", buf);
  t.is(face.family, "X");
  t.is(face.style, "normal");
  t.is(face.weight, "normal");
  t.is(face.stretch, "normal");
  t.is(face.unicodeRange, "U+0-10FFFF");
  t.is(face.variant, "normal");
  t.is(face.featureSettings, "normal");
  t.is(face.display, "auto");
  t.is(face.status, "unloaded");
});

test("FontFace honors descriptor overrides", (t) => {
  const face = new FontFace("X", new Uint8Array(4), {
    weight: "700",
    style: "italic",
    stretch: "condensed",
  });
  t.is(face.weight, "700");
  t.is(face.style, "italic");
  t.is(face.stretch, "condensed");
});

test("FontFace.load() resolves synchronously for buffer sources", async (t) => {
  const face = new FontFace("X", new Uint8Array(4));
  const result = await face.load();
  t.is(result, face);
  t.is(face.status, "loaded");
  // `loaded` mirrors `load()`'s outcome.
  t.is(await face.loaded, face);
});

test("FontFace.load() rejects URL-string sources", async (t) => {
  const face = new FontFace("X", "url(./missing.ttf)");
  await t.throwsAsync(() => face.load(), { instanceOf: TypeError });
  t.is(face.status, "error");
});

test("document.fonts is a stable FontFaceSet singleton", (t) => {
  const doc = new HTMLDocument();
  const set = doc.fonts;
  t.true(set instanceof FontFaceSet);
  t.is(doc.fonts, set);
  t.is(set.size, 0);
  t.is(set.status, "loaded");
});

test("FontFaceSet.add registers, has/size/iterate, delete/clear work", async (t) => {
  // Build a tiny non-font buffer. fontique will refuse to parse it, so
  // `register_fonts` returns "0 faces registered" — but the JS side
  // still considers the FontFace "added" once the native call completes
  // without throwing. (This matches the browser: a face whose source
  // turns out to be undecodable does not throw on `add`.)
  const buf = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);

  const doc = new HTMLDocument();
  const face = new FontFace("Stub", buf);
  await face.load();

  t.is(doc.fonts.size, 0);
  doc.fonts.add(face);
  t.true(doc.fonts.has(face));
  t.is(doc.fonts.size, 1);
  t.is(face.status, "loaded");

  // Iteration yields the face.
  const collected: typeof face[] = [];
  for (const f of doc.fonts) collected.push(f);
  t.deepEqual(collected, [face]);

  // forEach mirrors iteration.
  const seen: typeof face[] = [];
  doc.fonts.forEach((f) => seen.push(f));
  t.deepEqual(seen, [face]);

  // Re-adding the same face is a no-op (per Set semantics).
  doc.fonts.add(face);
  t.is(doc.fonts.size, 1);

  t.true(doc.fonts.delete(face));
  t.false(doc.fonts.has(face));
  t.is(doc.fonts.size, 0);
  // Re-deleting reports false.
  t.false(doc.fonts.delete(face));

  // clear is fine on an empty set.
  doc.fonts.clear();
});

test("FontFaceSet.add rejects non-FontFace arguments", (t) => {
  const doc = new HTMLDocument();
  t.throws(
    () => doc.fonts.add({ family: "X" } as unknown as FontFace),
    { instanceOf: TypeError },
  );
});

test("FontFaceSet.add rejects URL-source faces with a clear error", (t) => {
  const doc = new HTMLDocument();
  const face = new FontFace("X", "url(./missing.ttf)");
  t.throws(() => doc.fonts.add(face), { instanceOf: TypeError });
  t.false(doc.fonts.has(face));
});

test("FontFaceSet surfaces invalid CSS descriptors from the engine", (t) => {
  // The native side parses weight/style/stretch as CSS and throws
  // InvalidArg on garbage. The error must propagate, not be swallowed,
  // and the face must not be added.
  const doc = new HTMLDocument();
  const face = new FontFace("X", new Uint8Array(4), {
    weight: "not-a-real-weight",
  });
  t.throws(() => doc.fonts.add(face));
  t.false(doc.fonts.has(face));
});
