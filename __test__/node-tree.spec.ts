// Standard `Node` tree mutation surface: append/insert/remove/contains.

import test from "ava";

import { HTMLDocument } from "../dist/index.js";

test("appendChild / parentNode / childNodes are on Node", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const a = doc.createElement("div");
  const b = doc.createElement("span");

  body.appendChild(a);
  a.appendChild(b);

  t.is(b.parentNode, a);
  t.is(a.parentNode, body);
  t.is(a.firstChild, b);
  t.is(a.childNodes.length, 1);
  t.is(a.childNodes[0], b);
});

test("insertBefore inserts at the right position", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const a = doc.createElement("a");
  const b = doc.createElement("b");
  const c = doc.createElement("c");
  body.appendChild(a);
  body.appendChild(c);
  body.insertBefore(b, c);
  const kids = body.childNodes;
  t.is(kids.length, 3);
  t.is(kids[0], a);
  t.is(kids[1], b);
  t.is(kids[2], c);
});

test("removeChild detaches the node", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const a = doc.createElement("div");
  body.appendChild(a);
  t.is(a.parentNode, body);
  body.removeChild(a);
  t.is(a.parentNode, null);
});

test("Node.remove() detaches self", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const a = doc.createElement("div");
  body.appendChild(a);
  a.remove();
  t.is(a.parentNode, null);
});

test("contains walks ancestors", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const outer = doc.createElement("div");
  const inner = doc.createElement("span");
  body.appendChild(outer);
  outer.appendChild(inner);
  t.true(body.contains(inner));
  t.true(outer.contains(inner));
  t.false(inner.contains(outer));
});

test("cloneNode(false) shallow-copies a node without children", (t) => {
  const doc = new HTMLDocument();
  const div = doc.createElement("div");
  div.id = "src";
  div.appendChild(doc.createElement("span"));
  div.appendChild(doc.createTextNode("hello"));

  const clone = div.cloneNode(false);
  // Same tag, same attributes.
  t.is((clone as typeof div).tagName, "div");
  t.is((clone as typeof div).id, "src");
  // Distinct identity from the source.
  t.not(clone, div);
  // Detached: no parent until appended.
  t.is(clone.parentNode, null);
  // No children copied.
  t.is(clone.childNodes.length, 0);
  // Original is untouched.
  t.is(div.childNodes.length, 2);
});

test("cloneNode(true) deep-copies the whole subtree", (t) => {
  const doc = new HTMLDocument();
  const div = doc.createElement("div");
  div.id = "src";
  div.innerHTML = "<span>a</span><p>b</p>";

  const clone = div.cloneNode(true);
  t.is((clone as typeof div).id, "src");
  // Children are duplicated, not shared.
  t.is(clone.childNodes.length, 2);
  t.not(clone.firstChild, div.firstChild);
});

test("cloneNode preserves inline style attribute", (t) => {
  const doc = new HTMLDocument();
  const div = doc.createElement("div");
  div.setAttribute("style", "color: red");

  const shallow = div.cloneNode(false);
  t.is(shallow.getAttribute?.("style"), "color: red");

  const deep = div.cloneNode(true);
  t.is(deep.getAttribute?.("style"), "color: red");
});
