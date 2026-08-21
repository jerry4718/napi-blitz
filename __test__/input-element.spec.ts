// `HTMLInputElement` and `HTMLTextAreaElement`: construction, property
// accessors, and InputDataHandle integration.

import test from "ava";

import {Element, HTMLDocument, HTMLElement, HTMLInputElement, HTMLTextAreaElement,} from './_shim.ts';

// ---- Construction & inheritance --------------------------------------------

test("createElement('input') returns an HTMLInputElement", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.true(input instanceof HTMLInputElement);
  t.true(input instanceof HTMLElement);
  t.true(input instanceof Element);
  t.is(input.tagName, "input");
});

test("createElement('textarea') returns an HTMLTextAreaElement", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.true(textarea instanceof HTMLTextAreaElement);
  t.true(textarea instanceof HTMLElement);
  t.true(textarea instanceof Element);
  t.is(textarea.tagName, "textarea");
});

test("HTMLInputElement parsed from HTML is the correct subclass", (t) => {
  const doc = HTMLDocument.create();
  doc.body!.innerHTML = '<input id="i1" type="text">';
  const input = doc.getElementById("i1");
  t.true(input instanceof HTMLInputElement);
});

test("HTMLTextAreaElement parsed from HTML is the correct subclass", (t) => {
  const doc = HTMLDocument.create();
  doc.body!.innerHTML = '<textarea id="t1"></textarea>';
  const textarea = doc.getElementById("t1");
  t.true(textarea instanceof HTMLTextAreaElement);
});

test("non-input elements remain plain HTMLElement", (t) => {
  const doc = HTMLDocument.create();
  doc.body!.innerHTML = '<div id="d1"></div>';
  const div = doc.getElementById("d1");
  t.true(div instanceof HTMLElement);
  t.false(div instanceof HTMLInputElement);
  t.false(div instanceof HTMLTextAreaElement);
});

// ---- HTMLInputElement: attribute-backed properties -------------------------

test("HTMLInputElement.type defaults to 'text' and round-trips", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.is(input.type, "text");

  input.type = "checkbox";
  t.is(input.type, "checkbox");
  t.is(input.getAttribute("type"), "checkbox");
});

test("HTMLInputElement.disabled toggles the attribute", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.false(input.disabled);

  input.disabled = true;
  t.true(input.disabled);
  t.true(input.hasAttribute("disabled"));

  input.disabled = false;
  t.false(input.disabled);
  t.false(input.hasAttribute("disabled"));
});

test("HTMLInputElement.placeholder round-trips", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.is(input.placeholder, "");

  input.placeholder = "Enter name";
  t.is(input.placeholder, "Enter name");
  t.is(input.getAttribute("placeholder"), "Enter name");
});

test("HTMLInputElement.readOnly toggles the attribute", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.false(input.readOnly);

  input.readOnly = true;
  t.true(input.readOnly);

  input.readOnly = false;
  t.false(input.readOnly);
});

test("HTMLInputElement.required toggles the attribute", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.false(input.required);

  input.required = true;
  t.true(input.required);

  input.required = false;
  t.false(input.required);
});

test("HTMLInputElement.name round-trips", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.is(input.name, "");

  input.name = "username";
  t.is(input.name, "username");
  t.is(input.getAttribute("name"), "username");
});

test("HTMLInputElement.defaultValue reads/writes the value attribute", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.is(input.defaultValue, "");

  input.defaultValue = "default text";
  t.is(input.defaultValue, "default text");
  t.is(input.getAttribute("value"), "default text");
});

// ---- HTMLInputElement: value & checked (InputDataHandle) -------------------

test("HTMLInputElement.value reads from attribute when no editor exists", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.is(input.value, "");

  input.setAttribute("value", "hello");
  t.is(input.value, "hello");
});

test("HTMLInputElement.value setter updates the attribute", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  input.value = "test value";
  t.is(input.getAttribute("value"), "test value");
  t.is(input.value, "test value");
});

test("HTMLInputElement.value on parsed input reads the attribute", (t) => {
  const doc = HTMLDocument.create();
  doc.body!.innerHTML = '<input id="i1" value="prefilled">';
  const input = doc.getElementById("i1") as HTMLInputElement;
  t.is(input.value, "prefilled");
});

test("HTMLInputElement.checked toggles via attribute fallback", (t) => {
  const doc = HTMLDocument.create();
  const input = doc.createElement("input");
  t.false(input.checked);

  input.checked = true;
  t.true(input.checked);
  t.true(input.hasAttribute("checked"));

  input.checked = false;
  t.false(input.checked);
  t.false(input.hasAttribute("checked"));
});

test("HTMLInputElement.checked on parsed checkbox reads the attribute", (t) => {
  const doc = HTMLDocument.create();
  doc.body!.innerHTML = '<input id="cb" type="checkbox" checked>';
  const input = doc.getElementById("cb") as HTMLInputElement;
  t.true(input.checked);
});

// ---- HTMLTextAreaElement: attribute-backed properties ----------------------

test("HTMLTextAreaElement.rows defaults to 2 and round-trips", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.is(textarea.rows, 2);

  textarea.rows = 5;
  t.is(textarea.rows, 5);
  t.is(textarea.getAttribute("rows"), "5");
});

test("HTMLTextAreaElement.cols defaults to 20 and round-trips", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.is(textarea.cols, 20);

  textarea.cols = 40;
  t.is(textarea.cols, 40);
  t.is(textarea.getAttribute("cols"), "40");
});

test("HTMLTextAreaElement.placeholder round-trips", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.is(textarea.placeholder, "");

  textarea.placeholder = "Enter text";
  t.is(textarea.placeholder, "Enter text");
});

test("HTMLTextAreaElement.readOnly toggles the attribute", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.false(textarea.readOnly);

  textarea.readOnly = true;
  t.true(textarea.readOnly);

  textarea.readOnly = false;
  t.false(textarea.readOnly);
});

test("HTMLTextAreaElement.required toggles the attribute", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.false(textarea.required);

  textarea.required = true;
  t.true(textarea.required);

  textarea.required = false;
  t.false(textarea.required);
});

test("HTMLTextAreaElement.name round-trips", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.is(textarea.name, "");

  textarea.name = "comment";
  t.is(textarea.name, "comment");
});

test("HTMLTextAreaElement.disabled toggles the attribute", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.false(textarea.disabled);

  textarea.disabled = true;
  t.true(textarea.disabled);

  textarea.disabled = false;
  t.false(textarea.disabled);
});

// ---- HTMLTextAreaElement: value (InputDataHandle) --------------------------

test("HTMLTextAreaElement.value reads from attribute when no editor exists", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  t.is(textarea.value, "");

  textarea.setAttribute("value", "initial");
  t.is(textarea.value, "initial");
});

test("HTMLTextAreaElement.value setter updates the attribute", (t) => {
  const doc = HTMLDocument.create();
  const textarea = doc.createElement("textarea");
  textarea.value = "typed text";
  t.is(textarea.getAttribute("value"), "typed text");
  t.is(textarea.value, "typed text");
});

// ---- Identity / wrapper stability ------------------------------------------

test("HTMLInputElement wrapper identity is stable across queries", (t) => {
  const doc = HTMLDocument.create();
  doc.body!.innerHTML = '<input id="i1">';
  const a = doc.getElementById("i1");
  const b = doc.querySelector("#i1");
  t.is(a, b);
});
