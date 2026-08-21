// window-events example: exercises the unified Rust-side event dispatcher
// (`JsShellEventHandler`). All lifecycle events are dispatched from Rust —
// JS holds no parallel dispatch model.
//
// Event model — dispatched from Rust, the two receivers handled
// independently at the same moment:
//   win  -> `close`         (cancelable) — window-level; a listener's
//                             preventDefault() rejects close().
//   app  -> `window:close`  (cancelable) — app-level echo; preventDefault()
//                             rejects closeWindow().
//   win  -> `closed`        (non-cancelable) — after teardown.
//   app  -> `window:closed` (non-cancelable) — after teardown.
//   app  -> `window:open`   (cancelable) — app-only; before openWindow
//                             resolves; preventDefault() rejects openWindow.
//
// Driven by timers (no user clicks): each case opens/closes windows on a
// schedule and logs directly to stdout, exiting via `pump:end` once the hold
// window closes.

import {BlitzApp, HTMLDocument, WindowOptions} from "@ylcc/napi-blitz";
import type {Window} from "@ylcc/napi-blitz";

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

const newDoc = (): HTMLDocument =>
  HTMLDocument.create({
    baseHtml:
      "<!doctype html><html><head><title>window-events</title></head><body></body></html>",
  });

const app = BlitzApp.create();

// Once every window is closed the pump loop exits and fires `pump:end`.
app.addEventListener("pump:end", () => {
  console.log("\n=== DONE: all windows closed, pump exited ===");
  process.exit(0);
});
app.pumpLoop();

async function tryOpen(label: string): Promise<Window | null> {
  try {
    const w = await app.openWindow(
      newDoc(),
      WindowOptions.builder().size(300, 200),
    );
    console.log(`[${label}] openWindow resolved`);
    return w;
  } catch (err) {
    console.log(`[${label}] openWindow REJECTED: ${(err as Error).message}`);
    return null;
  }
}

async function main(): Promise<void> {
  // A window that stays open until the very end, so `outstanding_windows`
  // never hits 0 mid-run (which would fire pump:end early).
  const hold = await tryOpen("hold");
  if (!hold) {
    console.log("FATAL: hold window failed to open — aborting");
    process.exit(1);
  }

  // ── Case 1: app blocks openWindow via window:open ─────────────────────
  console.log("\n=== Case 1: app blocks openWindow (window:open preventDefault) ===");
  const blockOpen = (e: Event): void => {
    console.log("[app] window:open received → preventDefault");
    e.preventDefault();
  };
  app.addEventListener("window:open", blockOpen);
  await tryOpen("case1"); // must reject
  app.removeEventListener("window:open", blockOpen);
  await wait(80);

  // ── Case 2: app blocks closeWindow via window:close ───────────────────
  console.log("\n=== Case 2: app blocks closeWindow (window:close preventDefault) ===");
  const w2 = await tryOpen("case2");
  if (w2) {
    const blockAppClose = (e: Event): void => {
      console.log("[app] window:close received → preventDefault");
      e.preventDefault();
    };
    app.addEventListener("window:close", blockAppClose);
    try {
      await app.closeWindow(w2);
      console.log("[case2] closeWindow resolved (unexpected)");
    } catch (err) {
      console.log(`[case2] closeWindow REJECTED: ${(err as Error).message}`);
    }
    app.removeEventListener("window:close", blockAppClose);
    // Not blocked anymore — really close it so the pump can exit.
    await app.closeWindow(w2);
    console.log("[case2] second closeWindow resolved (window gone)");
  }
  await wait(80);

  // ── Case 3: window blocks its own close via close ─────────────────────
  console.log("\n=== Case 3: window blocks its own close (close preventDefault) ===");
  const w3 = await tryOpen("case3");
  if (w3) {
    const blockWinClose = (e: Event): void => {
      console.log("[window case3] close received → preventDefault");
      e.preventDefault();
    };
    w3.addEventListener("close", blockWinClose);
    try {
      await w3.close();
      console.log("[case3] close() resolved (unexpected)");
    } catch (err) {
      console.log(`[case3] close() REJECTED: ${(err as Error).message}`);
    }
    w3.removeEventListener("close", blockWinClose);
    await w3.close();
    console.log("[case3] second close() resolved (window gone)");
  }
  await wait(80);

  // ── Case 4: normal open/close event sequence ──────────────────────────
  console.log("\n=== Case 4: normal open/close event sequence ===");
  const onOpen = (): void =>
    console.log("[app] window:open received (not prevented)");
  const onClosed = (): void => console.log("[app] window:closed received");
  app.addEventListener("window:open", onOpen);
  app.addEventListener("window:closed", onClosed);
  const w4 = await tryOpen("case4");
  if (w4) {
    w4.addEventListener("closed", () =>
      console.log("[window case4] closed received"),
    );
    await wait(80);
    await w4.close();
    console.log("[case4] close() resolved");
  }
  app.removeEventListener("window:open", onOpen);
  app.removeEventListener("window:closed", onClosed);

  // ── Close the hold window: last window gone → pump:end → exit ─────────
  console.log("\n=== closing hold window ===");
  await hold.close();
}

main().catch((err) => {
  console.log(`FATAL: ${err}`);
  process.exit(1);
});
