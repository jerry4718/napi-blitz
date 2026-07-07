// Event dispatch: drives the Rust -> JS bridge directly. Each test
// uses `dispatchChain` to simulate the full capture/target/bubble walk
// that the native `handle_event` would perform.

import test from "ava";

import { BlitzPointerEvent, HTMLDocument } from "../packages/napi-blitz/dist/index.js";

import {
  dispatchChain,
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

  const result = dispatchChain(
    pluckDocument(doc),
    nodeIdOf(inner),
    [nodeIdOf(inner), nodeIdOf(outer), nodeIdOf(body)],
  );

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

  dispatchChain(
    pluckDocument(doc),
    nodeIdOf(inner),
    [nodeIdOf(inner), nodeIdOf(outer), nodeIdOf(body)],
  );

  t.deepEqual(calls, ["inner", "outer", "body"]);
});

test("event chain: preventDefault is reported", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const el = doc.createElement("button");
  body.appendChild(el);

  el.addEventListener("click", (e) => e.preventDefault());

  const result = dispatchChain(
    pluckDocument(doc),
    nodeIdOf(el),
    [nodeIdOf(el), nodeIdOf(body)],
  );
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

  dispatchChain(
    pluckDocument(doc),
    nodeIdOf(inner),
    [nodeIdOf(inner), nodeIdOf(body)],
  );

  t.is(observed, inner);
});
