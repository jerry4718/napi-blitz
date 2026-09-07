// Layer instances keep their `OwnDataRegistry` in a `napi_wrap` private
// slot attached to the object itself. A transparent Proxy that forwards
// property access the way Vue reactivity does —
// `Reflect.get(target, key, receiver)` / `Reflect.set(target, key, v, receiver)`
// — passes the *proxy* as the receiver, so a native layer accessor
// receives the proxy as `this` and its `napi_unwrap` fails.
//
// Every instance also stores the registry's heap address under a scalar
// key (`__napi_blitz_registry`, a BigInt). Proxy traps pass scalar values
// through unchanged (reactive wrappers only touch objects), so accessors
// fall back to reading the address through the proxy and dereference it
// directly (`crates/napi-inherit/src/own.rs`).

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
  }, "the registry address is read through the proxy and dereferenced");

  t.notThrows(() => {
    void el.scrollTop;
  }, "the raw instance unwraps directly");
});

test("the registry pointer key is a non-enumerable BigInt", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");

  const desc = Object.getOwnPropertyDescriptor(el, "__napi_blitz_registry");
  t.truthy(desc, "the key is an own property of the instance");
  t.is(desc!.enumerable, false, "hidden from enumeration");
  t.is(typeof desc!.value, "bigint", "the address is a scalar, untouched by proxy traps");
  t.is(Object.keys(el).includes("__napi_blitz_registry"), false);
});