// The shared case suite (see `chain-cases.ts`) run against the hand-written
// chain. Own file so ava runs it in parallel with the macro chain's entry.
import test from "ava";

import {buildInheritTestClasses, makeInheritLeafFromChain} from "../out/index.cjs";

import {registerChainCases} from "./chain-cases.ts";

const manual = buildInheritTestClasses() as any;

registerChainCases({
    Base: manual.Base,
    Mid: manual.Mid,
    Leaf: manual.Leaf,
    makeLeafFromChain: makeInheritLeafFromChain,
});

// ── chain-specific extras ─────────────────────────────────────────────────

test("manual: buildInheritTestClasses returns three constructor functions", (t) => {
    t.is(typeof manual.Base, "function");
    t.is(typeof manual.Mid, "function");
    t.is(typeof manual.Leaf, "function");
});

test("manual: buildInheritTestClasses is idempotent", (t) => {
    const again = buildInheritTestClasses() as any;
    t.is(again.Base, manual.Base);
    t.is(again.Mid, manual.Mid);
    t.is(again.Leaf, manual.Leaf);
});
