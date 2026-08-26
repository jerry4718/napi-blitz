// The shared behavioural suite for an inheritance chain. One set of cases is
// registered against both the `#[layer]` macro-generated chain and the
// hand-written chain, so any drift between the two implementations shows up
// as a failing case on that side.
import test from "ava";

export interface ChainApi {
    Base: any;
    Mid: any;
    Leaf: any;
    makeLeafFromChain: () => any;
}

export function registerChainCases(api: ChainApi): void {
    const {Base, Mid, Leaf, makeLeafFromChain} = api;

    // ── class shape ───────────────────────────────────────────────────────

    test("constructors are named after the JS class", (t) => {
        t.is(Base.name, "InheritBase");
        t.is(Mid.name, "InheritMid");
        t.is(Leaf.name, "InheritLeaf");
    });

    test("prototype chain links child to parent", (t) => {
        t.is(Object.getPrototypeOf(Leaf.prototype), Mid.prototype);
        t.is(Object.getPrototypeOf(Mid.prototype), Base.prototype);
        t.is(Object.getPrototypeOf(Base.prototype), Object.prototype);
    });

    test("constructor chain links child to parent", (t) => {
        t.is(Object.getPrototypeOf(Leaf), Mid);
        t.is(Object.getPrototypeOf(Mid), Base);
        t.is(Object.getPrototypeOf(Base), Function.prototype);
    });

    test("prototype.constructor points back to the class", (t) => {
        t.is(Base.prototype.constructor, Base);
        t.is(Mid.prototype.constructor, Mid);
        t.is(Leaf.prototype.constructor, Leaf);
    });

    test("ctor.prototype is non-enumerable and non-configurable (spec shape)", (t) => {
        const d = Object.getOwnPropertyDescriptor(Leaf, "prototype")!;
        t.true(d.writable);
        t.false(d.enumerable);
        t.false(d.configurable);
    });

    // ── construction ──────────────────────────────────────────────────────

    test("new Leaf passes args through every layer", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.baseValue, 7);
        t.is(leaf.midValue, 8);
        t.is(leaf.leafValue, 9);
    });

    test("instanceof works across the whole chain", (t) => {
        const leaf = new Leaf(1, 2, 3);
        t.true(leaf instanceof Leaf);
        t.true(leaf instanceof Mid);
        t.true(leaf instanceof Base);
        t.true(leaf instanceof Object);

        const mid = new Mid(1, 2);
        t.false(mid instanceof Leaf);
        t.true(mid instanceof Mid);
        t.true(mid instanceof Base);
    });

    test("calling the constructor without new throws", (t) => {
        t.throws(() => (Leaf as any)(1, 2, 3), {message: /requires 'new'/});
        t.throws(() => (Base as any)(1), {message: /requires 'new'/});
    });

    test("instances are isolated from each other", (t) => {
        const a = new Leaf(1, 1, 1);
        const b = new Leaf(2, 2, 2);
        t.is(a.baseValue, 1);
        t.is(b.baseValue, 2);
        t.is(a.leafShout(), "leaf:1+mid:1+base:1");
        t.is(b.leafShout(), "leaf:2+mid:2+base:2");
    });

    test("JS subclass extends the leaf and stays wired", (t) => {
        class CustomLeaf extends Leaf {}
        const c = new CustomLeaf(7, 8, 9);
        t.true(c instanceof CustomLeaf);
        t.true(c instanceof Leaf);
        t.true(c instanceof Mid);
        t.true(c instanceof Base);
        t.is(c.baseValue, 7);
        t.is(c.leafShout(), "leaf:9+mid:8+base:7");
        t.is(Object.getPrototypeOf(CustomLeaf.prototype), Leaf.prototype);
        t.is(c.baseSeenAfterSuper, 7);
        t.is(c.midSeenAfterSuper, 8);
    });

    // ── super ordering ────────────────────────────────────────────────────

    test("parent own block is readable after super call", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.baseSeenAfterSuper, 7);
        t.is(leaf.midSeenAfterSuper, 8);
    });

    // ── cross-layer access ────────────────────────────────────────────────

    test("getters resolve through the prototype chain", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.baseValue, 7);
        t.is(Mid.prototype.hasOwnProperty("baseValue"), false);
        t.is(Leaf.prototype.hasOwnProperty("baseValue"), false);
    });

    test("methods compose data across layers", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.baseGreet(), "base:7");
        t.is(leaf.midDescribe(), "mid:8/base:7");
        t.is(leaf.leafShout(), "leaf:9+mid:8+base:7");
    });

    test("with_own_mut mutates a layer in place, isolated per instance", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.baseValue, 7);
        t.is(leaf.bumpBase(10), 17);
        t.is(leaf.baseValue, 17);
        const other = new Leaf(1, 2, 3);
        t.is(other.baseValue, 1);
    });

    test("instance setter writes the layer slot, isolated per instance", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.baseValue, 7);
        leaf.baseValue = 42;
        t.is(leaf.baseValue, 42);
        const other = new Leaf(1, 2, 3);
        t.is(other.baseValue, 1);
    });

    test("static accessor pair reads and writes on the constructor", (t) => {
        t.is(Base.counter, 10);
        Base.counter = 42;
        t.is(Base.counter, 42);
    });

    test("Result-returning instance getter resolves on success and throws on error", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.checkedValue, 7);
        const zero = new Leaf(0, 0, 0);
        t.throws(() => zero.checkedValue, {message: /base_value is zero/});
    });

    test("Result-returning instance setter applies on success and throws on error", (t) => {
        const leaf = new Leaf(7, 8, 9);
        leaf.checkedValue = 42;
        t.is(leaf.checkedValue, 42);
        t.throws(() => {
            leaf.checkedValue = 0;
        }, {message: /cannot set to zero/});
        t.is(leaf.baseValue, 42);
    });

    test("static constants sit on the constructor and inherit down the ctor chain", (t) => {
        t.is(Base.BASE_CONST, 1);
        t.is(Mid.MID_CONST, 2);
        t.is(Leaf.LEAF_CONST, 3);
        t.is(Mid.BASE_CONST, 1);
        t.is(Leaf.BASE_CONST, 1);
        t.is(Leaf.MID_CONST, 2);
    });

    // ── property shapes ───────────────────────────────────────────────────

    test("members are non-enumerable and configurable", (t) => {
        const getter = Object.getOwnPropertyDescriptor(Mid.prototype, "midValue")!;
        t.is(typeof getter.get, "function");
        t.false(getter.enumerable);
        t.true(getter.configurable);

        const setter = Object.getOwnPropertyDescriptor(Base.prototype, "baseValue")!;
        t.is(typeof setter.set, "function");
        t.false(setter.enumerable);
        t.true(setter.configurable);

        const method = Object.getOwnPropertyDescriptor(Base.prototype, "baseGreet")!;
        t.is(typeof method.value, "function");
        t.false(method.enumerable);
        t.true(method.writable);
        t.true(method.configurable);
    });

    test("static accessor is non-enumerable and configurable", (t) => {
        const d = Object.getOwnPropertyDescriptor(Base, "counter")!;
        t.is(typeof d.get, "function");
        t.is(typeof d.set, "function");
        t.false(d.enumerable);
        t.true(d.configurable);
    });

    test("Result-returning static getter/setter resolve and throw on error", (t) => {
        const before = Base.counter;
        t.is(Base.checkedCounter, before);
        Base.checkedCounter = 33;
        t.is(Base.checkedCounter, 33);
        t.throws(() => {
            Base.checkedCounter = 0;
        }, {message: /cannot set counter to zero/});
        t.is(Base.checkedCounter, 33);
    });

    test("Result-returning methods resolve and throw on error", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.guardedGreet(), "base:7");
        t.is(leaf.guard(5), "guard:5/base:7");
        t.is(Leaf.staticGuard(5), 10);

        const zero = new Leaf(0, 0, 0);
        t.throws(() => zero.guardedGreet(), {message: /base is zero/});
        t.throws(() => leaf.guard(0), {message: /guard rejects zero/});
        t.throws(() => Leaf.staticGuard(0), {message: /static guard rejects zero/});
    });

    test("constructor rejects invalid args with an error", (t) => {
        t.throws(() => new Leaf(1, 2, 999), {message: /leaf_value too large/});
        const leaf = new Leaf(7, 8, 9);
        t.is(leaf.leafValue, 9);
    });

    test("static constants are read-only, non-enumerable and non-configurable (WebIDL shape)", (t) => {
        const d = Object.getOwnPropertyDescriptor(Leaf, "LEAF_CONST")!;
        t.false(d.writable);
        t.false(d.enumerable);
        t.false(d.configurable);
    });

    test("static constants reject JS-side mutation", (t) => {
        // ESM is strict-mode: assignment to a read-only property throws.
        t.throws(() => {
            Base.BASE_CONST = 999;
        }, {message: /read only/});
        t.is(Base.BASE_CONST, 1);
        // non-configurable: redefinition is blocked too.
        t.throws(() => {
            Object.defineProperty(Base, "BASE_CONST", {value: 777});
        }, {message: /redefine/});
        t.is(Base.BASE_CONST, 1);
    });

    test("own data is invisible to JS enumeration", (t) => {
        const leaf = new Leaf(7, 8, 9);
        t.is(Object.getOwnPropertySymbols(leaf).length, 0);
        t.deepEqual(Object.keys(leaf), []);
        t.is(JSON.stringify(leaf), "{}");
    });

    // ── own-block safety ──────────────────────────────────────────────────

    test("getter invoked on a foreign receiver throws (no registry)", (t) => {
        const desc = Object.getOwnPropertyDescriptor(Mid.prototype, "midValue")!;
        t.throws(() => (desc.get as any).call({}), {message: /no OwnDataRegistry/});
    });

    test("getter of a child layer throws on a parent-only instance", (t) => {
        const base = new Base(1);
        const desc = Object.getOwnPropertyDescriptor(Mid.prototype, "midValue")!;
        t.throws(() => (desc.get as any).call(base), {message: /out of range/});
    });

    // ── Rust-side construction from a data chain ──────────────────────────

    test("makeLeafFromChain builds a fully wired instance", (t) => {
        const leaf = makeLeafFromChain() as any;
        t.true(leaf instanceof Leaf);
        t.true(leaf instanceof Mid);
        t.true(leaf instanceof Base);
        t.is(leaf.baseValue, 100);
        t.is(leaf.midValue, 200);
        t.is(leaf.leafValue, 300);
        t.is(leaf.baseSeenAfterSuper, 100);
        t.is(leaf.leafShout(), "leaf:300+mid:200+base:100");
        t.is(Object.getOwnPropertySymbols(leaf).length, 0);
    });
}
