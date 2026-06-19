// `Document` surface: construction, root accessors, queries, factories.

import test from "ava";

import {
  Comment,
  Element,
  HTMLDocument,
  HTMLElement,
  Node,
  Text,
} from "../packages/napi-blitz/dist/index.js";

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

test("Element.querySelector / querySelectorAll are scoped to descendants", (t) => {
  const doc = new HTMLDocument();
  doc.body!.innerHTML =
    '<div id="root"><p class="a">1</p><span class="a">2</span><p class="b">3</p></div>';

  const root = doc.getElementsByTagName("div")[0];

  // Simple type selector.
  const firstP = root.querySelector("p");
  t.truthy(firstP);
  t.is(firstP!.textContent, "1");

  // Class selector returns both descendants.
  const aEls = root.querySelectorAll(".a");
  t.is(aEls.length, 2);
  t.is(aEls[0].tagName, "p");
  t.is(aEls[1].tagName, "span");

  // Descendant combinator.
  const spansInP = root.querySelectorAll("p span");
  // No <span> inside any <p> here.
  t.is(spansInP.length, 0);

  // The root element itself must not be a match even if it satisfies
  // the selector (#root matches "#root" but is excluded).
  t.is(root.querySelector("#root"), null);

  // No match.
  t.is(root.querySelector("section"), null);
  t.is(root.querySelectorAll("section").length, 0);
});

test("Element.querySelector supports id and compound selectors", (t) => {
  const doc = new HTMLDocument();
  doc.body!.innerHTML =
    '<ul><li id="first" class="item">a</li><li class="item">b</li></ul>';

  const ul = doc.getElementsByTagName("ul")[0];

  const byId = ul.querySelector("#first");
  t.truthy(byId);
  t.is(byId!.textContent, "a");

  const items = ul.querySelectorAll("li.item");
  t.is(items.length, 2);

  // :first-child pseudo (stylo supports structural pseudos).
  const firstChild = ul.querySelector("li:first-child");
  t.truthy(firstChild);
  t.is(firstChild!.id, "first");
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
  // eslint-disable-next-line no-console
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
