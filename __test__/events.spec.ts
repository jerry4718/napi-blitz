// Event dispatch: exercises the JS-side EventTarget chain directly.
//
// In the new architecture, Rust drives the three-phase dispatch via
// `wrap_node` + `node.dispatchEvent(event)`. These tests verify the
// JS-side EventTarget behavior that Rust relies on: bubble,
// stopPropagation, preventDefault, and target identity.
//
// Since Node.js `EventTarget.dispatchEvent` does not bubble on its own
// (Rust walks the chain manually), we simulate the Rust dispatch walk
// here: target -> ancestors, stopping when `cancelBubble` is set.

import test from "ava";

import { UIEvent, PointerEvent, HTMLDocument } from "../dist/index.js";
import type { Node } from "../dist/index.js";

/**
 * Simulate the Rust-side bubble walk: dispatch `event` on `target`,
 * then walk up parentNode chain calling dispatchEvent on each ancestor.
 * Stops early when `event.cancelBubble` (stopPropagation) is set.
 */
function bubbleDispatch(target: Node, event: Event): void {
  let cur: Node | null = target;
  while (cur !== null) {
    cur.dispatchEvent(event);
    if (event.cancelBubble) return;
    cur = cur.parentNode;
  }
}

test("event subclasses are exported", (t) => {
  t.true(typeof PointerEvent === "function");
});

test("event chain: bubble + stopPropagation", (t) => {
  const doc = HTMLDocument.create();
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

  const event = new UIEvent(
    { type: "click", bubbles: true, cancelable: true },
  );
  bubbleDispatch(inner, event);

  t.deepEqual(calls, ["inner"]);
  t.true(event.cancelBubble);
});

test("event chain: full bubble when no stop", (t) => {
  const doc = HTMLDocument.create();
  const body = doc.body!;
  const outer = doc.createElement("div");
  const inner = doc.createElement("span");
  body.appendChild(outer);
  outer.appendChild(inner);

  const calls: string[] = [];
  body.addEventListener("click", () => calls.push("body"));
  outer.addEventListener("click", () => calls.push("outer"));
  inner.addEventListener("click", () => calls.push("inner"));

  const event = new UIEvent(
    { type: "click", bubbles: true, cancelable: true },
  );
  bubbleDispatch(inner, event);

  t.deepEqual(calls, ["inner", "outer", "body"]);
});

test("event chain: preventDefault is reported", (t) => {
  const doc = HTMLDocument.create();
  const body = doc.body!;
  const el = doc.createElement("button");
  body.appendChild(el);

  el.addEventListener("click", (e) => e.preventDefault());

  const event = new UIEvent(
    { type: "click", bubbles: true, cancelable: true },
  );
  el.dispatchEvent(event);
  t.true(event.defaultPrevented);
});

test("event.target stays pinned to the originating node", (t) => {
  const doc = HTMLDocument.create();
  const body = doc.body!;
  const inner = doc.createElement("span");
  body.appendChild(inner);

  let observed: EventTarget | null = null;
  body.addEventListener("click", (e) => {
    observed = e.target;
  });

  const event = new UIEvent(
    { type: "click", bubbles: true, cancelable: true },
  );
  // Set target before dispatching, mirroring what Rust does via
  // __setLazyTarget / Object.defineProperty.
  Object.defineProperty(event, "target", {
    value: inner,
    configurable: true,
  });
  bubbleDispatch(inner, event);

  t.is(observed, inner);
});
