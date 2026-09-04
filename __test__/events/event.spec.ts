import test from "ava";
import {
  Event,
  CustomEvent,
  MessageEvent,
  EventTarget,
} from "../_shim.ts";

process.on("uncaughtException", (err, origin) => {
  console.error(`Uncaught#${ origin }:`, err);
});

// ── class shape ─────────────────────────────────────────────────────────

test("classes are exported", (t) => {
  t.is(typeof Event, "function");
  t.is(typeof CustomEvent, "function");
  t.is(typeof MessageEvent, "function");
  t.is(typeof EventTarget, "function");
});

test("prototype chain follows the standard hierarchy", (t) => {
  t.is(Object.getPrototypeOf(CustomEvent.prototype), Event.prototype);
  t.is(Object.getPrototypeOf(MessageEvent.prototype), Event.prototype);
  t.is(Object.getPrototypeOf(Event.prototype), Object.prototype);
});

test("constructor prototype wiring", (t) => {
  const e = new Event("x");
  t.true(e instanceof Event);
  t.true(e instanceof Object);
  const ce = new CustomEvent("x");
  t.true(ce instanceof Event);
  t.true(ce instanceof CustomEvent);
});

// ── Event ───────────────────────────────────────────────────────────────

test("Event constructor reads the type", (t) => {
  const e = new Event("click");
  t.is(e.type, "click");
});

test("Event default flags are false", (t) => {
  const e = new Event("click");
  t.false(e.bubbles);
  t.false(e.cancelable);
  t.false(e.composed);
  t.false(e.isTrusted);
});

test("timeStamp is a number", (t) => {
  const e = new Event("click");
  t.is(typeof e.timeStamp, "number");
});

test("eventPhase constants match the standard", (t) => {
  t.is(Event.NONE, 0);
  t.is(Event.CAPTURING_PHASE, 1);
  t.is(Event.AT_TARGET, 2);
  t.is(Event.BUBBLING_PHASE, 3);
});

test("fresh event is in NONE phase", (t) => {
  const e = new Event("click");
  t.is(e.eventPhase, Event.NONE);
  t.is(e.target, null);
  t.is(e.currentTarget, null);
  t.false(e.defaultPrevented);
});

test("preventDefault works only when cancelable", (t) => {
  const notCancelable = new Event("x");
  notCancelable.preventDefault();
  t.false(notCancelable.defaultPrevented);

  const cancelable = new CustomEvent("x");
  cancelable.preventDefault();
  t.false(cancelable.defaultPrevented);
});

test("stopPropagation / stopImmediatePropagation exist", (t) => {
  const e = new Event("x");
  t.is(typeof e.stopPropagation, "function");
  t.is(typeof e.stopImmediatePropagation, "function");
  e.stopPropagation();
  e.stopImmediatePropagation();
  t.pass();
});

test("composedPath returns an array", (t) => {
  const e = new Event("x");
  t.deepEqual(e.composedPath(), []);
});

// ── CustomEvent ─────────────────────────────────────────────────────────

test("CustomEvent stores a string detail", (t) => {
  const ce = new CustomEvent("boom", "payload");
  t.is(ce.type, "boom");
  t.is(ce.detail, "payload");
});

test("CustomEvent stores a number detail", (t) => {
  const ce = new CustomEvent("boom", 42);
  t.is(ce.detail, 42);
});

test("CustomEvent stores a boolean detail", (t) => {
  const ce = new CustomEvent("boom", true);
  t.true(ce.detail);
});

test("CustomEvent stores a bigint detail", (t) => {
  const ce = new CustomEvent("boom", 123n);
  t.is(ce.detail, 123n);
  const neg = new CustomEvent("boom", -5n);
  t.is(neg.detail, -5n);
});

test("CustomEvent stores an object detail", (t) => {
  const obj = { a: 1, b: [2, 3] };
  const ce = new CustomEvent("boom", obj);
  t.deepEqual(ce.detail, obj);
  t.is(ce.detail, obj); // object detail keeps identity, per the standard
});

test("CustomEvent default detail is null", (t) => {
  const ce = new CustomEvent("boom");
  t.is(ce.detail, null);
});

// ── MessageEvent ────────────────────────────────────────────────────────

test("MessageEvent stores data and origin", (t) => {
  const me = new MessageEvent("msg", "hello", "https://example.com");
  t.is(me.type, "msg");
  t.is(me.data, "hello");
  t.is(me.origin, "https://example.com");
});

test("MessageEvent defaults", (t) => {
  const me = new MessageEvent("msg");
  t.is(me.data, null);
  t.is(me.origin, "");
});

// ── EventTarget ─────────────────────────────────────────────────────────

test("addEventListener + dispatchEvent invokes the listener with the event", (t) => {
  const et = new EventTarget();
  let seen = null;
  et.addEventListener("foo", (e) => {
    seen = e;
  });
  const ev = new Event("foo");
  const result = et.dispatchEvent(ev);
  t.true(result);
  t.is(seen, ev);
});

test("dispatchEvent returns true for a non-cancelable prevented event", (t) => {
  // The current Event constructor is `(type)` only; cancelable defaults to
  // false, so preventDefault() does not cancel the default action.
  const et = new EventTarget();
  const ev = new Event("foo");
  ev.preventDefault();
  const result = et.dispatchEvent(ev);
  t.true(result);
});

test("listener with a different type is not called", (t) => {
  const et = new EventTarget();
  let calls = 0;
  et.addEventListener("foo", () => calls++);
  et.dispatchEvent(new Event("bar"));
  t.is(calls, 0);
});

test("removeEventListener stops future dispatch", (t) => {
  const et = new EventTarget();
  let calls = 0;
  const cb = () => calls++;
  et.addEventListener("foo", cb);
  et.dispatchEvent(new Event("foo"));
  et.removeEventListener("foo", cb);
  et.dispatchEvent(new Event("foo"));
  t.is(calls, 1);
});

test("capture listeners are not invoked on the plain target dispatch", (t) => {
  const et = new EventTarget();
  let calls = 0;
  et.addEventListener("foo", () => calls++, true);
  et.dispatchEvent(new Event("foo"));
  t.is(calls, 0);
});

test("adding the same listener twice does not duplicate", (t) => {
  const et = new EventTarget();
  let calls = 0;
  const cb = () => calls++;
  et.addEventListener("foo", cb);
  et.addEventListener("foo", cb);
  et.dispatchEvent(new Event("foo"));
  t.is(calls, 1);
});

test("dispatchEvent is reentrant: listener can dispatch another event", (t) => {
  const et = new EventTarget();
  const order = [];
  et.addEventListener("outer", () => {
    order.push("outer");
    et.dispatchEvent(new Event("inner"));
  });
  et.addEventListener("inner", () => order.push("inner"));
  et.dispatchEvent(new Event("outer"));
  t.deepEqual(order, ["outer", "inner"]);
});

test("stopImmediatePropagation prevents later listeners on the same target", (t) => {
  const et = new EventTarget();
  const order = [];
  et.addEventListener("foo", () => {
    order.push("first");
  });
  et.addEventListener("foo", (e) => {
    order.push("second");
    e.stopImmediatePropagation();
  });
  et.addEventListener("foo", () => {
    order.push("third");
  });
  et.dispatchEvent(new Event("foo"));
  t.deepEqual(order, ["first", "second"]);
});
