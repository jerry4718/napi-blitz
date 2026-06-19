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
