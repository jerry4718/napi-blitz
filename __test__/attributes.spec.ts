// `Element` attribute surface: getAttribute/setAttribute/removeAttribute,
// the `attributes` proxy, and the `id` / `className` shortcuts.

import test from "ava";

import { HTMLDocument } from "../packages/napi-blitz/dist/index.js";

test("getAttribute / setAttribute / removeAttribute round trip", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div");
  el.setAttribute("data-x", "1");
  t.is(el.getAttribute("data-x"), "1");
  t.true(el.hasAttribute("data-x"));
  el.removeAttribute("data-x");
  t.is(el.getAttribute("data-x"), null);
  t.false(el.hasAttribute("data-x"));
});

test("attributes proxy: get / set / delete / in / ownKeys", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div");
  el.attributes.id = "foo";
  el.attributes["data-x"] = "1";
  t.is(el.attributes.id, "foo");
  t.is(el.attributes["data-x"], "1");
  t.true("id" in el.attributes);
  t.false("missing" in el.attributes);

  const keys = Object.keys(el.attributes).sort();
  t.deepEqual(keys, ["data-x", "id"]);

  delete el.attributes.id;
  t.is(el.attributes.id, undefined);
  t.is(el.getAttribute("id"), null);
});

test("element.id and className shortcuts", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div");
  el.id = "hi";
  el.className = "a b";
  t.is(el.getAttribute("id"), "hi");
  t.is(el.getAttribute("class"), "a b");
  t.is(el.id, "hi");
  t.is(el.className, "a b");
});
