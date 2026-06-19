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

test("document.title round-trips through <title> element", (t) => {
  const doc = new HTMLDocument();

  // Empty document: no <title>, getter is "".
  t.is(doc.title, "");

  // Setter creates a <title> in <head>.
  doc.title = "Hello";
  t.is(doc.title, "Hello");
  const titleEl = doc.head!.childNodes.find(
    (n): n is Element => n instanceof Element && n.tagName === "title",
  );
  t.truthy(titleEl);
  t.is(titleEl!.textContent, "Hello");

  // Setting again updates the existing element rather than creating a new one.
  doc.title = "World";
  t.is(doc.title, "World");
  const titles = doc.head!.childNodes.filter(
    (n) => n instanceof Element && n.tagName === "title",
  );
  t.is(titles.length, 1);
});

test("getElementsByTagName returns a snapshot in tree order", (t) => {
  const doc = new HTMLDocument();
  const body = doc.body!;
  body.innerHTML = "<div><span></span></div><span></span>";

  const spans = doc.getElementsByTagName("span");
  t.is(spans.length, 2);
  t.is(spans[0].tagName, "span");
  t.is(spans[1].tagName, "span");

  // Upper-case input is normalized because blitz stores tag names lowercased.
  const divs = doc.getElementsByTagName("DIV");
  t.is(divs.length, 1);
  t.is(divs[0].tagName, "div");

  // Unknown tag: empty snapshot.
  t.is(doc.getElementsByTagName("nope").length, 0);
});

test("Document.getElementsByClassName matches class tokens", (t) => {
  const doc = new HTMLDocument();
  doc.body!.innerHTML =
    '<div class="foo bar"></div><span class="foo"></span><p class="bar baz"></p>';

  const foo = doc.getElementsByClassName("foo");
  t.is(foo.length, 2);
  t.is(foo[0].tagName, "div");
  t.is(foo[1].tagName, "span");

  const bar = doc.getElementsByClassName("bar");
  t.is(bar.length, 2);
  t.is(bar[0].tagName, "div");
  t.is(bar[1].tagName, "p");

  // Substring of a token should not match.
  t.is(doc.getElementsByClassName("ba").length, 0);
  // Unknown class: empty.
  t.is(doc.getElementsByClassName("nope").length, 0);
});

test("Element.getElementsByTagName is scoped to descendants", (t) => {
  const doc = new HTMLDocument();
  doc.body!.innerHTML =
    "<section><div><span></span></div><span></span></section><span id=outer></span>";

  const section = doc.getElementsByTagName("section")[0];
  // Two spans inside <section>.
  const spans = section.getElementsByTagName("span");
  t.is(spans.length, 2);

  // The element itself is not included even if it matches.
  const sections = section.getElementsByTagName("section");
  t.is(sections.length, 0);

  // "*" matches all descendant elements.
  const all = section.getElementsByTagName("*");
  t.is(all.length, 3); // div + 2 spans

  // Case-folding.
  const divs = section.getElementsByTagName("DIV");
  t.is(divs.length, 1);
  t.is(divs[0].tagName, "div");
});

test("Element.getElementsByClassName is scoped to descendants", (t) => {
  const doc = new HTMLDocument();
  doc.body!.innerHTML =
    '<div class="root"><span class="foo"></span><p class="foo bar"></p></div>';

  const div = doc.getElementsByTagName("div")[0];
  // Two descendants carry "foo".
  const foo = div.getElementsByClassName("foo");
  t.is(foo.length, 2);
  t.is(foo[0].tagName, "span");
  t.is(foo[1].tagName, "p");

  // The root element itself is excluded even though it carries "root".
  t.is(div.getElementsByClassName("root").length, 0);

  // Unknown class: empty.
  t.is(div.getElementsByClassName("nope").length, 0);
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
  // Standard API: createComment accepts optional initial data. The
  // native side ignores it (blitz has no Comment payload yet); the JS
  // wrapper logs a one-shot warning. Silence it for the duration of
  // this test.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any, no-console
  const origWarn = console.warn;
  // eslint-disable-next-line no-console
  console.warn = () => {};
  try {
    const c = doc.createComment("note");
    t.true(c instanceof Comment);
    // blitz drops the content; data round-trips as the empty string.
    t.is(c.data, "");
    c.data = "again";
    t.is(c.data, "");
  } finally {
    // eslint-disable-next-line no-console
    console.warn = origWarn;
  }
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
