// `#[layer(generator)]` / `#[layer(async_generator)]` end-to-end cases:
// every iteration step drives a Rust callback, values are read from the
// instance at loop time, and the async protocol consumes the same
// `{value, done}` results through `await`.
import test from "ava";

import {GenSource} from "../out/index.cjs";

test("generator: for...of sees constructor items and terminates", (t) => {
    const g = new GenSource("src");
    const seen: number[] = [];
    for (const v of g) seen.push(v);
    t.deepEqual(seen, [10, 20, 30]);
});

test("generator: spread and manual next() match the protocol", (t) => {
    const g = new GenSource("src");
    t.deepEqual([...g], [10, 20, 30]);

    const it = g[Symbol.iterator]();
    t.is(it.next().value, 10);
    t.is(it.next().value, 20);
    t.is(it.next().value, 30);
    t.true(it.next().done);
});

test("generator: a push mid-loop is visible to the same loop", (t) => {
    const g = new GenSource("src");
    const seen: number[] = [];
    let pushed = false;
    for (const v of g) {
        seen.push(v);
        if (!pushed) {
            g.push(40);
            pushed = true;
        }
    }
    t.deepEqual(seen, [10, 20, 30, 40]);
});

test("async_generator: for await...of consumes the same items", async (t) => {
    const g = new GenSource("src");
    const seen: number[] = [];
    for await (const v of g) seen.push(v);
    t.deepEqual(seen, [10, 20, 30]);
});

test("async_generator: Symbol.asyncIterator is callable and terminates", async (t) => {
    const g = new GenSource("src");
    t.is(typeof g[Symbol.asyncIterator], "function");
    const it = g[Symbol.asyncIterator]();
    t.deepEqual(it.next(), {value: 10, done: false});
    t.is(it.next().value, 20);
    t.is(it.next().value, 30);
    t.true(it.next().done);
});

test("generator: separate loops get independent cursors", (t) => {
    const g = new GenSource("src");
    const a = g[Symbol.iterator]();
    const b = g[Symbol.iterator]();
    a.next();
    t.is(a.next().value, 20);
    t.is(b.next().value, 10);
});
