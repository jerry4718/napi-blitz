// EventTarget / Event behavior cases ported from an eval_report-driven
// suite: listener dispatch order, capture, stopImmediatePropagation,
// handleEvent indirection, `this` binding, and on<event> attribute slots.

import test from "ava";
import { Event, EventTarget, HTMLDocument } from "../_shim.ts";

test("function listener runs with the target as this; object listener uses handleEvent", (t) => {
  const hits: string[] = [];
  const push = (s: string) => hits.push(s);

  class ET extends EventTarget {
    name = "ET";
  }

  const et: any = new ET();

  function handleEvent() {
    push(this.name);
  }

  et.addEventListener("notify", handleEvent);
  et.addEventListener("notify", { name: "EL", handleEvent });

  et.dispatchEvent(new Event("notify"));

  t.deepEqual(hits, ["ET", "EL"]);
});

test("capture listeners fire before bubble listeners", (t) => {
  const et = new EventTarget();
  const log: string[] = [];
  et.addEventListener("ping", () => log.push("bubble-first"));
  et.addEventListener("ping", () => log.push("capture"), true);
  et.addEventListener("ping", () => log.push("bubble-second"));
  et.dispatchEvent(new Event("ping"));

  t.is(log.join(","), "capture,bubble-first,bubble-second");
});

test("stopImmediatePropagation in a capture listener halts the dispatch", (t) => {
  const et = new EventTarget();
  const log: string[] = [];
  et.addEventListener(
    "ping",
    (e) => {
      log.push("capture");
      e.stopImmediatePropagation();
    },
    true,
  );
  et.addEventListener("ping", () => log.push("bubble"));
  et.dispatchEvent(new Event("ping"));

  t.is(log.join(","), "capture");
});

test("handleEvent is looked up per dispatch, so mutating it takes effect", (t) => {
  const et = new EventTarget();
  const hits: string[] = [];
  const push = (s: string) => hits.push(s);
  const listener = {
    handleEvent() {
      push("first");
    },
  };
  et.addEventListener("ping", listener);
  et.dispatchEvent(new Event("ping"));
  listener.handleEvent = () => push("second");
  et.dispatchEvent(new Event("ping"));

  t.deepEqual(hits, ["first", "second"]);
});

test("on<event> attribute slot fires in registration order and overwrites in place", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");
  const log: string[] = [];
  const tag = (name: string) => () => log.push(name);
  el.addEventListener("click", tag("a"));
  (el as any).onclick = tag("f1");
  el.addEventListener("click", tag("b"));
  (el as any).onclick = tag("f2");
  el.dispatchEvent(new Event("click"));

  t.is(log.join(","), "a,f2,b");
});

test("clearing and re-setting on<event> keeps the attribute slot position", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");
  const log: string[] = [];
  const tag = (name: string) => () => log.push(name);
  el.addEventListener("click", tag("a"));
  (el as any).onclick = tag("f1");
  el.addEventListener("click", tag("b"));
  (el as any).onclick = null;
  (el as any).onclick = tag("f3");
  el.dispatchEvent(new Event("click"));

  t.is(log.join(","), "a,b,f3");
});

test("clearing on<event> removes the attribute listener entirely", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");
  const log: string[] = [];
  const tag = (name: string) => () => log.push(name);
  el.addEventListener("click", tag("a"));
  (el as any).onclick = tag("f1");
  el.addEventListener("click", tag("b"));
  (el as any).onclick = null;
  el.dispatchEvent(new Event("click"));

  t.is(log.join(","), "a,b");
});