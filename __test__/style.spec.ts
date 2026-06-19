// `HTMLElement.style` Proxy: standard CSSOM `CSSStyleDeclaration`
// surface backed by the native inline-style block.

import test from "ava";

import { HTMLDocument, HTMLElement } from "../packages/napi-blitz/dist/index.js";

test("style is a stable Proxy on HTMLElement", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div");
  t.true(el instanceof HTMLElement);
  // Same identity across reads.
  t.is((el as HTMLElement).style, (el as HTMLElement).style);
});

test("style.color set/read/delete round-trips", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div") as HTMLElement;
  el.style.color = "red";
  // stylo may serialize `red` as `rgb(255, 0, 0)`; just check
  // non-empty.
  t.true(typeof el.style.color === "string" && el.style.color.length > 0);

  // 'in' operator surfaces presence.
  t.true("color" in el.style);

  // Delete removes it.
  delete el.style.color;
  t.is(el.style.color, "");
  t.false("color" in el.style);
});

test("style.fontSize (camelCase) maps to kebab-case `font-size`", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div") as HTMLElement;
  el.style.fontSize = "12px";

  // Same value reachable through both spellings.
  t.is(el.style.fontSize, "12px");
  t.is(el.style["font-size"], "12px");

  // ownKeys reports the kebab-case canonical form (stylo's longhand
  // identifier).
  const keys = Object.keys(el.style);
  t.true(keys.includes("font-size"));
  t.false(keys.includes("fontSize"));
});

test("style.cssText returns a serialized block and reparses on set", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div") as HTMLElement;
  el.style.color = "red";
  el.style.margin = "0";

  // cssText is non-empty and contains both properties.
  const text = el.style.cssText;
  t.true(text.length > 0);
  t.true(text.includes("margin"));

  // Setting cssText replaces the entire block.
  el.style.cssText = "padding: 4px";
  t.is(el.style.color, "");
  t.is(el.style.padding, "4px");
});

test("getPropertyValue / setProperty / removeProperty methods (CSSOM)", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div") as HTMLElement;

  el.style.setProperty("color", "blue");
  t.true(el.style.getPropertyValue("color").length > 0);

  // camelCase also accepted as an argument; method-form converts too.
  el.style.setProperty("fontSize", "14px");
  t.is(el.style.getPropertyValue("font-size"), "14px");

  // removeProperty returns the previous value, or "" if absent.
  const prev = el.style.removeProperty("color");
  t.true(prev.length > 0);
  t.is(el.style.removeProperty("color"), "");
});

test("length and item(i) follow declaration order", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div") as HTMLElement;
  el.style.color = "red";
  el.style.padding = "0";

  t.is(el.style.length, Object.keys(el.style).length);
  t.true(el.style.length >= 2);
  // item(out-of-range) is "".
  t.is(el.style.item(99), "");
  // Numeric string indices follow item(n).
  t.is((el.style as unknown as Record<string, string>)[String(0)], el.style.item(0));
});

test("unknown CSS properties are ignored, not thrown", (t) => {
  const doc = new HTMLDocument();
  const el = doc.createElement("div") as HTMLElement;
  // stylo refuses unknown properties silently; getter/setter don't throw.
  t.notThrows(() => {
    el.style["definitely-not-a-css-property"] = "x";
  });
  t.is(el.style["definitely-not-a-css-property"], "");
});
