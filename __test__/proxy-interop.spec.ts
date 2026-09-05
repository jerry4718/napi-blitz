// Layer instances keep their `OwnDataRegistry` in a `napi_wrap` private
// slot attached to the object itself. A transparent Proxy that forwards
// property access the way Vue reactivity does —
// `Reflect.get(target, key, receiver)` / `Reflect.set(target, key, v, receiver)`
// — passes the *proxy* as the receiver, so a native layer accessor
// receives the proxy as `this`, and its `napi_unwrap` would fail.
//
// The global proxy-compat mode (`setProxyCompat`, written once) decides
// how accessors resolve the registry: `off` keeps the pre-proxy behavior,
// `on`/`auto` resolve through the instance's self-reference key
// (`crates/napi-inherit/src/own.rs`). Tests below run in file order, so
// the "unset" case executes before the mode is written.

import test from "ava";

import {HTMLDocument, setProxyCompat} from "./_shim.ts";

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

test("unset mode: a receiver-passing proxy still throws", (t) => {
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");
  const proxied = vueLikeProxy(el);

  t.throws(
    () => {
      void proxied.scrollTop;
    },
    {message: /OwnDataRegistry/},
    "default is `off`: the receiver stays a proxy and the unwrap fails",
  );
});

test("setProxyCompat(\"on\"): proxy resolves through the self-reference key", (t) => {
  t.notThrows(() => {
    setProxyCompat("on");
  }, "first write succeeds");

  // The self-reference key is attached at construction time, so the mode
  // must be in effect before the instances are created.
  const doc = HTMLDocument.create();
  const el = doc.createElement("div");
  const proxied = vueLikeProxy(el);

  t.notThrows(() => {
    void proxied.scrollTop;
  }, "the registry is resolved through the proxy to the raw instance");

  t.throws(
    () => {
      setProxyCompat("auto");
    },
    {message: /already set to On/},
    "the mode is written once",
  );
});