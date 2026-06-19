import test from "ava";

import {
  HTMLDocument,
  BlitzPointerEvent,
  Node,
  Element,
  HTMLElement,
  Text,
  Comment,
  type EventPayload,
} from "../dist/index.js";

// nodeId is package-private. The Rust dispatch path emits it for us, so a
// few tests below fabricate a payload identical to what the native bridge
// would send. This is a test-only hatch — user code never sees nodeIds.
function nodeIdOf(n: Node): number {
  return (n as unknown as { _nodeId: number })._nodeId;
}

function makeClickPayload(targetId: number, chain: number[]): EventPayload {
  return {
    eventType: "click",
    target: targetId,
    chain,
    bubbles: true,
    cancelable: true,
    pointer: undefined,
    wheel: undefined,
    key: undefined,
    input: undefined,
    ime: undefined,
  };
}

// ---------------------------------------------------------------------------
// Construction & root accessors
// ---------------------------------------------------------------------------

test("HTMLDocument has documentElement / head / body", (t) => {
  const doc = new HTMLDocument();
  t.is(doc.documentElement.tagName, "html");
  t.is(doc.head?.tagName, "head");
  t.is(doc.body?.tagName, "body");
});

test("createElement returns an HTMLElement; identity is stable", (t) => {
  const doc = new HTMLDocument();
  const div = doc.createElement("div");
  t.true(div instanceof HTMLElement);
  t.true(div instanceof Element);
  t.true(div instanceof Node);
  t.is(div.tagName, "div");

  doc.body!.appendChild(div);
  // Tree query through Node returns the same wrapper object.
  t.is(doc.body!.firstChild, div);
});

test("createTextNode returns a Text wrapper", (t) => {
  const doc = new HTMLDocument();
  const t1 = doc.createTextNode("hi");
  t.true(t1 instanceof Text);
  t.is(t1.data, "hi");
  t1.appendData(" there");
  t.is(t1.data, "hi there");
});

test("createComment returns a Comment wrapper", (t) => {
  const doc = new HTMLDocument();
  const c = doc.createComment();
  t.true(c instanceof Comment);
  // NOTE: blitz's `NodeData::Comment` is currently a unit variant and
  // does not store text. Setting `data` is a no-op until blitz grows a
  // Comment payload. We still exercise the API surface.
  c.data = "note";
  t.is(typeof c.data, "string");
});

// ---------------------------------------------------------------------------
// Node tree operations (the standard DOM surface)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Element attributes
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// HTML serialization
// ---------------------------------------------------------------------------

test("innerHTML / outerHTML reflect the tree", (t) => {
  const doc = new HTMLDocument();
  const div = doc.createElement("div");
  doc.body!.appendChild(div);
  div.innerHTML = "<span>hi</span>";

  t.true(div.innerHTML.includes("<span"));
  t.true(div.innerHTML.includes("hi"));
  t.true(div.outerHTML.startsWith("<div"));
  t.true(div.outerHTML.endsWith("</div>"));
});

// ---------------------------------------------------------------------------
// Event dispatch (drives the Rust -> JS bridge directly)
// ---------------------------------------------------------------------------

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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const result = (doc as any)._dispatchFromNative(payload);

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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (doc as any)._dispatchFromNative(payload);

  t.deepEqual(calls, ["inner", "outer", "body"]);
});

test("event chain: preventDefault is reported", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  const el = doc.createElement("button");
  body.appendChild(el);

  el.addEventListener("click", (e) => e.preventDefault());

  const payload = makeClickPayload(nodeIdOf(el), [nodeIdOf(el), nodeIdOf(body)]);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const result = (doc as any)._dispatchFromNative(payload);
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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (doc as any)._dispatchFromNative(payload);

  t.is(observed, inner);
});
