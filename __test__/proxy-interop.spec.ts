// Layer instances keep their `OwnDataRegistry` in a `napi_wrap` private
// slot attached to the object itself. A transparent Proxy that forwards
// property access the way Vue reactivity does —
// `Reflect.get(target, key, receiver)` / `Reflect.set(target, key, v, receiver)`
// — passes the *proxy* as the receiver, so a native layer accessor
// receives the proxy as `this`, and its `napi_unwrap` would fail.
//
// Regression guard: every layer instance carries a self-reference data
// property, so accessors re-resolve the raw instance through a proxied
// receiver before unwrapping (`crates/napi-inherit/src/own.rs`).

import test from "ava";

import {HTMLDocument} from "./_shim.ts";

// The exact receiver-passing shape of Vue's `MutableReactiveHandler`:
// accessor properties run with `this` = the proxy.
function vueLikeProxy<T extends object>(target: T): T {
  return new Proxy(target, {
    get(t, key, receiver) {
      return Reflect.get(t, key, receiver);
    },
    set(t, key, value, receiver) {
      return Reflect.set(t, key, value, receiver);
    },
  });
}

test("layer accessors survive a receiver-passing proxy", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");
  const proxied = vueLikeProxy(el);

  t.notThrows(() => {
    void proxied.scrollTop;
  }, "the registry is resolved through the proxy to the raw instance");
});

test("the same accessor works on the raw instance", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");

  t.notThrows(() => {
    void el.scrollTop;
  });
});