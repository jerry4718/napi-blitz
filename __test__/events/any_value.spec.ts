import test from "ava";
import {CustomEvent, Event, EventTarget} from "../_shim.ts";

declare const gc: (() => void) | undefined;

// A value stored in a `CustomEvent.detail` is held through an `AnyValue`
// reference: while the event object is alive the detail object stays alive
// (strong reference), and once the event object is collected the reference
// is released so the detail can be collected too.
test("CustomEvent detail releases its AnyValue reference when the event is collected", async (t) => {
  const finalized = new Set<string>();
  const reg = new FinalizationRegistry<string>((held) => {
    finalized.add(held);
  });

  function seed() {
    const payload = {tag: "payload"};
    const ev = new CustomEvent("boom", payload);

    // The stored AnyValue reads back the same object.
    t.is(ev.detail, payload);

    // Register both for collection notification. Returning from `seed`
    // drops the function-scoped references; the detail then only stays
    // alive through the event's AnyValue reference.
    reg.register(payload, "payload");
    reg.register(ev, "event");
  }

  seed();

  let waited = 0;
  while (waited < 100 && !finalized.has("payload")) {
    if (typeof gc === "function") {
      gc();
      gc();
    }
    await new Promise((r) => setTimeout(r, 20));
    waited += 1;
  }

  t.true(finalized.has("payload"), "detail should be collected once the event is gone");
});

// The event passed to `dispatchEvent` crosses into native code through an
// `AnyValue` and out to each listener. That reference is released when the
// call returns, so once nothing else references the event (and the listener
// no longer holds it) it can be collected.
test("dispatchEvent releases the event's AnyValue reference after the call", async (t) => {
  const finalized = new Set<string>();
  const reg = new FinalizationRegistry<string>((held) => {
    finalized.add(held);
  });

  function seed() {
    const et = new EventTarget();
    const ev = new Event("foo");
    let received: unknown = null;
    et.addEventListener("foo", (e: unknown) => {
      received = e;
    });

    // The listener receives the same event object.
    t.true(et.dispatchEvent(ev));
    t.true(et.dispatchEvent(ev));
    t.true(et.dispatchEvent(ev));
    t.true(et.dispatchEvent(ev));
    t.is(received, ev);

    // Register for collection notification. Returning from `seed` drops the
    // function-scoped references, including the listener's captured one.
    reg.register(ev, "event");
    reg.register(received as object, "received");
  }

  seed();

  let waited = 0;
  while (waited < 100 && !finalized.has("event")) {
    if (typeof gc === "function") {
      gc();
      gc();
    }
    await new Promise((r) => setTimeout(r, 20));
    waited += 1;
  }

  t.true(finalized.has("event"), "event should be collected after dispatch");
});
