// Dispatch-time listener mutation semantics.
//
// These three cases were verified by hand in Chrome (Chromium):
// they pin the standard behavior when a dispatched callback mutates the
// listener registry of the same target mid-dispatch. Any change to the
// dispatch driver must keep them green.

import test from "ava";
import { Event, EventTarget } from "../_shim.ts";

test("removing a not-yet-called listener inside dispatch skips it for the current event", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  const h1 = () => {
    hits.push("h1");
    target.removeEventListener("x", h2);
  };
  const h2 = () => {
    hits.push("h2");
  };
  target.addEventListener("x", h1);
  target.addEventListener("x", h2);

  // h1 removes h2 while h2 has not run yet: h2 must not fire this round.
  target.dispatchEvent(new Event("x"));
  // The removal persists: only h1 fires again.
  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h1", "h1"]);
});

test("once listener re-registering the same callback inside dispatch keeps firing", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  const h = () => {
    hits.push("h");
    // Re-arm with the same callback; Chrome fires h again on the next event.
    target.addEventListener("x", h, { once: true });
  };
  target.addEventListener("x", h, { once: true });

  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h", "h", "h"]);
});

test("once listener removing then re-registering itself inside dispatch keeps firing", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  const h = () => {
    hits.push("h");
    target.removeEventListener("x", h);
    target.addEventListener("x", h, { once: true });
  };
  target.addEventListener("x", h, { once: true });

  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h", "h", "h"]);
});

// The cases below are derived from the three Chrome-verified ones and the
// standard dispatch semantics; verify them in a browser before trusting
// them as a spec pin.

test("adding a new listener mid-dispatch does not fire it for the current event", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  const h3 = () => {
    hits.push("h3");
  };
  const h1 = () => {
    hits.push("h1");
    target.addEventListener("x", h3);
  };
  target.addEventListener("x", h1);

  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h1", "h1", "h3"]);
});

test("removing the running listener itself only affects the next event", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  const h1 = () => {
    hits.push("h1");
    target.removeEventListener("x", h1);
  };
  target.addEventListener("x", h1);

  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h1"]);
});

test("removing then re-adding a not-yet-called listener skips it this and every round", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  const h2 = () => {
    hits.push("h2");
  };
  const h1 = () => {
    hits.push("h1");
    target.removeEventListener("x", h2);
    target.addEventListener("x", h2);
  };
  target.addEventListener("x", h1);
  target.addEventListener("x", h2);

  // Chrome-verified: h1 re-runs the remove+re-add every round, so the
  // re-added h2 is always removed again before it can fire.
  target.dispatchEvent(new Event("x"));
  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h1", "h1"]);
});

test("nested dispatch on the same target runs an independent round", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  let nested = false;
  const h1 = () => {
    hits.push("h1");
    if (!nested) {
      nested = true;
      target.dispatchEvent(new Event("x"));
    }
  };
  const h2 = () => {
    hits.push("h2");
  };
  target.addEventListener("x", h1);
  target.addEventListener("x", h2);

  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h1", "h1", "h2", "h2"]);
});

test("removing a not-yet-called listener before a nested dispatch keeps the outer walk intact", (t) => {
  const target = new EventTarget();
  const hits: string[] = [];
  let nested = false;
  const h1 = () => {
    hits.push("h1");
    if (!nested) {
      nested = true;
      target.removeEventListener("x", h2);
      target.dispatchEvent(new Event("x"));
    }
  };
  const h2 = () => {
    hits.push("h2");
  };
  target.addEventListener("x", h1);
  target.addEventListener("x", h2);

  target.dispatchEvent(new Event("x"));

  t.deepEqual(hits, ["h1", "h1"]);
});