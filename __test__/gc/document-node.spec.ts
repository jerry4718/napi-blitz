// JS-wrapper lifetime of the Document/Node reference graph, observed via
// FinalizationRegistry. Each factory drops every strong JS reference
// before returning, so a fired finalization callback means the wrapper is
// reachable only through native references.
//
// Current expectations:
// - Document root wrapper: pinned by the NodeCache strong napi_ref (the
//   SharedDocument <-> JS Document cross-heap cycle) -> must FAIL here.
// - In-document Node wrapper: pinned the same way -> must FAIL here.
// - Detached Node wrapper: cache entry was switched to weak, wrapper is
//   collectible -> should PASS.
// - style/attributes Proxy: pinned by the strong `Anything` proxy caches
//   on SharedDocument -> must FAIL here.

import test from "ava";

import {HTMLDocument, HTMLElement} from "../_shim.ts";
import {track, waitForFinalization} from "./_gc-helpers.ts";

function orphanDocument(): string {
  const document = HTMLDocument.create();
  return track(document).id;
}

function inDocumentNode(): string {
  const document = HTMLDocument.create();
  const element = document.createElement("div") as HTMLElement;
  document.body!.appendChild(element);
  return track(element).id;
}

function detachedNode(): string {
  const document = HTMLDocument.create();
  const element = document.createElement("div") as HTMLElement;
  document.body!.appendChild(element);
  element.remove();
  return track(element).id;
}

function styleProxy(): string {
  const document = HTMLDocument.create();
  const element = document.createElement("div") as HTMLElement;
  return track(element.style as object).id;
}

function attributesProxy(): string {
  const document = HTMLDocument.create();
  const element = document.createElement("div") as HTMLElement;
  return track(element.attributes as object).id;
}

test.serial("Document wrapper is finalized after the last JS reference is dropped", async (t) => {
  const id = orphanDocument();
  t.true(await waitForFinalization(id), "Document wrapper was never finalized: still pinned by a native strong reference");
});

test.serial("in-document Node wrapper is finalized after its Document is dropped", async (t) => {
  const id = inDocumentNode();
  t.true(await waitForFinalization(id), "in-document Node wrapper was never finalized: still pinned by a native strong reference");
});

test.serial("detached Node wrapper is finalized after the last JS reference is dropped", async (t) => {
  const id = detachedNode();
  t.true(await waitForFinalization(id), "detached Node wrapper was never finalized");
});

test.serial("style Proxy is finalized after its element and Document are dropped", async (t) => {
  const id = styleProxy();
  t.true(await waitForFinalization(id), "style Proxy was never finalized: pinned by the SharedDocument proxy cache");
});

test.serial("attributes Proxy is finalized after its element and Document are dropped", async (t) => {
  const id = attributesProxy();
  t.true(await waitForFinalization(id), "attributes Proxy was never finalized: pinned by the SharedDocument proxy cache");
});
