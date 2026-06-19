// `attributes` proxy — gives elements a NamedNodeMap-ish object that
// reads/writes attributes lazily through the native side.
//
// Standard `NamedNodeMap` exposes `Attr` objects with `name`/`value`/
// `namespaceURI`. We don't model `Attr` yet; instead this version
// supports the common ergonomic patterns:
//
//   el.attributes.id              -> string | undefined
//   el.attributes.id = "x"        -> sets the attribute
//   "id" in el.attributes         -> hasAttribute
//   delete el.attributes.id       -> removeAttribute
//   Object.keys(el.attributes)    -> attribute names
//   for (const k in el.attributes) ...
//   JSON.stringify(el.attributes) -> { name: value, ... }
//
// `length`, indexed access, and `getNamedItem` will land when we add a
// proper `Attr` wrapper.

import type { NativeDocHandle } from "../native";

/** The shape user code sees behind `el.attributes`. */
export type AttributesMap = Record<string, string>;

export function makeAttributesProxy(
  handle: NativeDocHandle,
  nodeId: bigint,
): AttributesMap {
  // The proxy target is just a placeholder object; we route every
  // operation through `handle` so reads always reflect the latest
  // native state.
  const target: Record<string, unknown> = Object.create(null);

  return new Proxy(target, {
    get(_, prop): unknown {
      if (typeof prop !== "string") return undefined;
      const value = handle.getAttribute(nodeId, prop);
      return value === null ? undefined : value;
    },

    set(_, prop, value): boolean {
      if (typeof prop !== "string") return false;
      handle.setAttribute(nodeId, prop, String(value), null);
      return true;
    },

    has(_, prop): boolean {
      if (typeof prop !== "string") return false;
      return handle.getAttribute(nodeId, prop) !== null;
    },

    deleteProperty(_, prop): boolean {
      if (typeof prop !== "string") return false;
      handle.removeAttribute(nodeId, prop, null);
      return true;
    },

    ownKeys(): string[] {
      return handle.getAttributes(nodeId).map((a) => a.name);
    },

    getOwnPropertyDescriptor(_, prop): PropertyDescriptor | undefined {
      if (typeof prop !== "string") return undefined;
      const value = handle.getAttribute(nodeId, prop);
      if (value === null) return undefined;
      return {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
      };
    },
  }) as AttributesMap;
}
