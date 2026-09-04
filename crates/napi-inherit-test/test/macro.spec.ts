// The shared case suite (see `chain-cases.ts`) run against the `#[layer]`
// macro-generated chain. Own file so ava runs it in parallel with the
// manual chain's entry.
import test from "ava";
import {createRequire} from "node:module";

import {InheritBase, InheritMid, InheritLeaf, makeProcLeafFromChain} from "../out/index.cjs";

import {registerChainCases} from "./chain-cases.ts";

const require = createRequire(import.meta.url);
const binding = require("../out/index.cjs");

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

test("rust layer struct names are not exported", (t) => {
    for (const name of ["InheritBase", "InheritMid", "InheritLeaf"]) {
        t.true(name in binding, `${name} should be exported`);
    }
    for (const name of ["BaseLayer", "MidLayer", "LeafLayer"]) {
        t.false(name in binding, `${name} should not leak as a JS export`);
    }
});
