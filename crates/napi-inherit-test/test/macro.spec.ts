// The shared case suite (see `chain-cases.ts`) run against the `#[layer]`
// macro-generated chain. Own file so ava runs it in parallel with the
// manual chain's entry.
import test from "ava";
import {createRequire} from "node:module";

import {InheritBase, InheritMid, InheritLeaf, makeProcLeafFromChain} from "../lib/index.cjs";

import {registerChainCases} from "./chain-cases.ts";

const require = createRequire(import.meta.url);
const binding = require("../lib/index.cjs");

registerChainCases({
    Base: InheritBase,
    Mid: InheritMid,
    Leaf: InheritLeaf,
    makeLeafFromChain: makeProcLeafFromChain,
});

// ── chain-specific extras ─────────────────────────────────────────────────

test("macro: static method on the leaf constructor", (t) => {
    t.is(InheritLeaf.leafConst(), 99);
});

test("macro: field js_name overrides the default property name", (t) => {
    const leaf = new InheritLeaf(1, 2, 3);
    t.is(leaf.renamedProp, 42);
    leaf.renamedProp = 7;
    t.is(leaf.renamedProp, 7);
});

test("macro: #[layer(this)] injects the instance into a receiver-less method", (t) => {
    const leaf = new InheritLeaf(7, 8, 9);
    t.is(leaf.thisInjected(), "injected:7");
    const desc = Object.getOwnPropertyDescriptor(InheritBase.prototype, "thisInjected")!;
    t.is(typeof desc.value, "function");
    t.is(typeof InheritBase.thisInjected, "undefined");
});

test("rust layer struct names are not exported", (t) => {
    for (const name of ["InheritBase", "InheritMid", "InheritLeaf"]) {
        t.true(name in binding, `${name} should be exported`);
    }
    for (const name of ["BaseLayer", "MidLayer", "LeafLayer"]) {
        t.false(name in binding, `${name} should not leak as a JS export`);
    }
});
