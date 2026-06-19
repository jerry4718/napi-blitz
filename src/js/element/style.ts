// `el.style` — Proxy-backed `CSSStyleDeclaration` for inline styles.
//
// Mirrors the web `CSSStyleDeclaration` interface as it surfaces on
// `HTMLElement.style`: the inline-style block. Reads and writes go
// through the native side every time, so the value reflects whatever
// the engine currently has parsed (e.g. after a normalizing reparse).
//
// Supported access patterns:
//
//   el.style.color = "red"             // sets `color`
//   el.style.color                     // reads `color`
//   delete el.style.color              // removes `color`
//   "color" in el.style                // has-check
//   Object.keys(el.style)              // enumerated property names
//   for (const name in el.style) ...   // iteration
//   el.style.fontSize = "12px"         // camelCase auto-mapped to `font-size`
//   el.style["font-size"]              // kebab-case works directly
//   el.style.getPropertyValue("color") // standard method form
//   el.style.setProperty("color", "x") // standard method form
//   el.style.removeProperty("color")   // standard method form
//   el.style.cssText                   // serialized block
//   el.style.cssText = "color:red"     // re-parses entire block
//   el.style.length                    // count of declarations
//   el.style.item(0)                   // name at index 0
//
// Things deliberately not implemented yet:
//   - Per-property `.priority` (`!important`). The native setter
//     drops priority; the getter never reports it.
//   - The CSSOM 2.0 `parentRule` link.
//   - Indexed access via numeric property keys (`el.style[0]`). Use
//     `item(0)` instead, which is also standard.

import type { NativeDocHandle } from "../native";

/**
 * Public shape behind `el.style`. The string-indexed entries are CSS
 * property values; the named members below are the standard
 * `CSSStyleDeclaration` methods/properties we support.
 */
export interface StyleDeclaration {
  /** CSS property name -> value. camelCase and kebab-case both work. */
  [property: string]: string | number | undefined | unknown;

  /** Serialized inline style, e.g. `"color: red; margin: 0"`. */
  cssText: string;

  /** Number of declarations currently in the block. */
  readonly length: number;

  /** Standard CSSOM: get a property's serialized value, "" if absent. */
  getPropertyValue(name: string): string;

  /** Standard CSSOM: set a property. `priority` is currently ignored. */
  setProperty(name: string, value: string, priority?: string): void;

  /** Standard CSSOM: remove and return the previous value, "" if absent. */
  removeProperty(name: string): string;

  /** Standard CSSOM: name of the declaration at `index`, "" if out of range. */
  item(index: number): string;
}

/**
 * Convert camelCase JS identifiers into kebab-case CSS property names.
 * Leaves names that already contain a hyphen, start with `--`, or are
 * already lowercased untouched.
 */
function camelToKebab(name: string): string {
  if (name.startsWith("--")) return name; // CSS custom property
  if (name.includes("-")) return name; // already kebab
  // Replace each upper-case letter with `-` + its lower-case form.
  // This matches the CSSOM's idl-attribute-to-css-property rule for
  // the common cases (fontSize -> font-size, backgroundColor ->
  // background-color). The browser also has a special case for the
  // `webkit`/`Moz`/`ms`/`O` vendor prefixes (`Webkit` -> `-webkit`),
  // which we replicate.
  let out = "";
  for (let i = 0; i < name.length; i++) {
    const ch = name[i];
    if (ch >= "A" && ch <= "Z") {
      out += "-" + ch.toLowerCase();
    } else {
      out += ch;
    }
  }
  // `WebkitTransform` becomes `-webkit-transform` (leading hyphen).
  if (out.startsWith("webkit-") || out.startsWith("moz-") || out.startsWith("ms-") || out.startsWith("o-")) {
    return "-" + out;
  }
  return out;
}

/**
 * Names that are spec methods/properties on `CSSStyleDeclaration` and
 * therefore must not be intercepted as CSS property names by the
 * proxy. Anything else is treated as a CSS property.
 */
const RESERVED = new Set<string | symbol>([
  "cssText",
  "length",
  "getPropertyValue",
  "setProperty",
  "removeProperty",
  "item",
  // Standard internal symbols / well-known JS hooks. We don't return
  // anything for these so the proxy reports `undefined`.
  Symbol.toPrimitive,
  Symbol.toStringTag,
  Symbol.iterator,
  // Avoid colliding with debugger / util.inspect.
  "toString",
  "valueOf",
  "constructor",
  // Promise-like detection.
  "then",
]);

export function makeStyleProxy(
  handle: NativeDocHandle,
  nodeId: number,
): StyleDeclaration {
  // The target carries the spec methods so calls like
  // `el.style.setProperty("color", "x")` resolve via the normal
  // property lookup (the `get` trap returns these untouched).
  const target = Object.create(null) as Record<string | symbol, unknown>;

  target.cssText = "";
  target.length = 0;

  target.getPropertyValue = (name: string): string => {
    const v = handle.getStyleProperty(nodeId, camelToKebab(name));
    return v ?? "";
  };

  target.setProperty = (name: string, value: string): void => {
    handle.setStyleProperty(nodeId, camelToKebab(name), value);
  };

  target.removeProperty = (name: string): string => {
    const css = camelToKebab(name);
    const previous = handle.getStyleProperty(nodeId, css) ?? "";
    handle.removeStyleProperty(nodeId, css);
    return previous;
  };

  target.item = (index: number): string => {
    const names = handle.getStylePropertyNames(nodeId);
    return names[index] ?? "";
  };

  return new Proxy(target, {
    get(_, prop): unknown {
      // Spec accessors served from the target object.
      if (prop === "cssText") {
        return handle.getStyleAttribute(nodeId);
      }
      if (prop === "length") {
        return handle.getStylePropertyNames(nodeId).length;
      }
      if (RESERVED.has(prop)) {
        return target[prop];
      }
      if (typeof prop !== "string") return undefined;
      // Numeric string indices (`style["0"]`) act like `item(n)` per
      // CSSOM, returning the n-th property name.
      if (/^\d+$/.test(prop)) {
        const i = Number(prop);
        const names = handle.getStylePropertyNames(nodeId);
        return names[i] ?? "";
      }
      const v = handle.getStyleProperty(nodeId, camelToKebab(prop));
      return v ?? "";
    },

    set(_, prop, value): boolean {
      if (prop === "cssText") {
        // Setting cssText reparses the whole block. We delegate by
        // setting the `style` attribute, which blitz reparses for us.
        handle.setAttribute(nodeId, "style", String(value), null);
        return true;
      }
      if (RESERVED.has(prop)) {
        // Spec methods are not assignable from user code.
        return false;
      }
      if (typeof prop !== "string") return false;
      handle.setStyleProperty(nodeId, camelToKebab(prop), String(value));
      return true;
    },

    has(_, prop): boolean {
      if (RESERVED.has(prop)) return true;
      if (typeof prop !== "string") return false;
      return handle.getStyleProperty(nodeId, camelToKebab(prop)) !== null;
    },

    deleteProperty(_, prop): boolean {
      if (RESERVED.has(prop)) return false;
      if (typeof prop !== "string") return false;
      handle.removeStyleProperty(nodeId, camelToKebab(prop));
      return true;
    },

    ownKeys(): string[] {
      return handle.getStylePropertyNames(nodeId);
    },

    getOwnPropertyDescriptor(_, prop): PropertyDescriptor | undefined {
      if (RESERVED.has(prop)) return undefined;
      if (typeof prop !== "string") return undefined;
      const v = handle.getStyleProperty(nodeId, camelToKebab(prop));
      if (v === null) return undefined;
      return {
        value: v,
        writable: true,
        enumerable: true,
        configurable: true,
      };
    },
  }) as unknown as StyleDeclaration;
}
