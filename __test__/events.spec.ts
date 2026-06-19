// Event dispatch: drives the Rust -> JS bridge directly. Each test
// hand-builds an `EventPayload` (via `makeClickPayload`) that mirrors
// what the native side would emit, then calls the package-private
// `_dispatchFromNative` to walk the chain.

import test from "ava";

import { BlitzPointerEvent, HTMLDocument } from "../dist/index.js";

import {
  makeClickPayload,
  nodeIdOf,
  pluckDocument,
} from "./_helpers.js";

test("event subclasses are exported", (t) => {
  t.true(typeof BlitzPointerEvent === "function");
});

test("event chain: bubble + stopPropagation", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const outer = doc.createElement("div");
  const inner = doc.createElement("span");
  body.appendChild(outer);
  outer.appendChild(inner);

  const calls: string[] = [];
  body.addEventListener("click", () => calls.push("body"));
  outer.addEventListener("click", () => calls.push("outer"));
  inner.addEventListener("click", (e) => {
    calls.push("inner");
    e.stopPropagation();
  });

  const payload = makeClickPayload(nodeIdOf(inner), [
    nodeIdOf(inner),
    nodeIdOf(outer),
    nodeIdOf(body),
  ]);
  const result = pluckDocument(doc)._dispatchFromNative(payload);

  t.deepEqual(calls, ["inner"]);
  t.true(result.propagationStopped);
});

test("event chain: full bubble when no stop", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const outer = doc.createElement("div");
  const inner = doc.createElement("span");
  body.appendChild(outer);
  outer.appendChild(inner);

  const calls: string[] = [];
  body.addEventListener("click", () => calls.push("body"));
  outer.addEventListener("click", () => calls.push("outer"));
  inner.addEventListener("click", () => calls.push("inner"));

  const payload = makeClickPayload(nodeIdOf(inner), [
    nodeIdOf(inner),
    nodeIdOf(outer),
    nodeIdOf(body),
  ]);
  pluckDocument(doc)._dispatchFromNative(payload);

  t.deepEqual(calls, ["inner", "outer", "body"]);
});

test("event chain: preventDefault is reported", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const el = doc.createElement("button");
  body.appendChild(el);

  el.addEventListener("click", (e) => e.preventDefault());

  const payload = makeClickPayload(nodeIdOf(el), [
    nodeIdOf(el),
    nodeIdOf(body),
  ]);
  const result = pluckDocument(doc)._dispatchFromNative(payload);
  t.true(result.defaultPrevented);
});

test("event.target stays pinned to the originating node", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const inner = doc.createElement("span");
  body.appendChild(inner);

  let observed: EventTarget | null = null;
  body.addEventListener("click", (e) => {
    observed = e.target;
  });

  const payload = makeClickPayload(nodeIdOf(inner), [
    nodeIdOf(inner),
    nodeIdOf(body),
  ]);
  pluckDocument(doc)._dispatchFromNative(payload);

  t.is(observed, inner);
});
