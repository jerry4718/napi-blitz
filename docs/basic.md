# Basic: The pump loop and threading model

This file introduces napi-blitz's pump-loop concept, why it is called a
"pump", the threading model, and everyday usage.

## What a pump is

A pump is the mechanism that drives the application's event loop. One pump
call **pumps out a batch** of pending winit events from the queue and hands
them to the handler (input dispatch, blitz DOM dispatch, rendering, window
lifecycle), then returns; the application's lifetime is a run of pump
iterations.

- **Single iteration**: `app.pumpAppEvents(timeoutMs)` — a synchronous NAPI
  call that processes events for at most `timeoutMs` milliseconds and
  returns `PumpResult { continue, exit, code }`. Each iteration runs, in
  order: `poll_live_views` (letting the previous JS turn's DOM mutations
  flow through poll → request_redraw) → `drain_closing_windows` →
  synthetic-exit check → `event_loop.pump_app_events(timeout, handler)` →
  drain/poll again → return
- **Loop**: `app.pumpLoop(options)` starts the async pump loop

## Why it is called a pump

The name comes straight from winit's `EventLoop::pump_app_events` (the
`pump_events` family): **non-blockingly pumping out one frame of pending
events**, as opposed to `run`/`run_app`, the blocking main loop that owns
the thread until exit. The metaphor: every call draws the queued events up
like a water pump, hands them to the handler, `pump_app_events` returns,
and the loop draws the next batch.

## Threading model

Single-threaded. All UI code — winit, blitz DOM, rendering, JS — runs on
the same main (UI) thread:

- **The winit EventLoop must stay on the main thread**: the OS delivers
  windows and input events on the main thread
- **The JS event loop is the main thread's yield mechanism**: Rust has no
  loop that owns a thread; each iteration JS hands control to native via a
  synchronous NAPI call (`pumpAppEvents`), native pumps one round of
  events and returns, JS continues
- **One NAPI crossing per iteration**: a synchronous main-thread call, no
  worker threads involved
- **Lock-free, no cross-thread sharing**: DOM mutations are directly
  visible between JS and Rust; no synchronization primitives needed
- **The cost**: time spent inside a pump is UI jank — a single iteration
  must return promptly

The loop skeleton deliberately lives on the JS side: moving it back to
Rust would add two NAPI crossings per iteration, and the main thread's
yield mechanism is the JS event loop to begin with.

## Usage

```ts
const app = new BlitzApp();

// Start the loop at top level; default cadence 16.67ms (~60fps)
const handle = app.pumpLoop({
  targetPeriod: 16.67, // target period (ms)
  timeout: 16.67,      // max time one iteration blocks waiting for events (default = targetPeriod)
  // signal: abortSignal, // optional external stop signal
});

app.addEventListener("pump", (e) => {
  const {result} = e;
  if (result.exit) {
    // All windows closed; the loop is about to end
  }
});

app.addEventListener("pump:end", (e) => {
  const {end} = e; // { kind: "exit" | "stop" | "abort", reason? }
});

// Stop explicitly (optionally with a coordinating reason)
handle.stop("app-quit");
// await handle.done;
```

- **Cadence**: the loop anchors to the absolute `performance.now()`
  timeline — each iteration sleeps until the target tick, so `setTimeout`
  imprecision never accumulates into cadence drift; an iteration that
  overruns its period (e.g. heavy rendering) runs again immediately,
  aligned to `now + targetPeriod`
- **Event stream**: `pump:start` → `pump` (each iteration carries a
  `PumpResult`) → `pump:end` (three end kinds: `exit` / `stop` / `abort`);
  a thrown error is broadcast as `pump:error`
- **Stopping**: all windows closed → native reports `exit`;
  `handle.stop(reason)` → `stop`; an external `AbortSignal` → `abort`
- **Constraints**: only one pump loop may run per app (a second call
  throws); the loop must start from top-level setup, never from inside an
  event handler — that would re-enter the native loop