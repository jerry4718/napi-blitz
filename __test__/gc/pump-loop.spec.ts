// One app per process (see window-teardown.spec.ts).
//
// This file: the BlitzApp wrapper after its async pump loop has been
// stopped and awaited. `BlitzAppLayer.pumping_loop` keeps a strong napi
// reference to the last loop handle, and the handle's closure holds the
// app, so the app is expected to stay pinned (native <-> JS cycle through
// its own own-block). Expected to FAIL until that retention is fixed.

import test from "ava";

import {BlitzApp} from "../_shim.ts";
import {track, waitForFinalization} from "./_gc-helpers.ts";

// The app and its loop handle must be dropped before the GC loop runs, so
// the flow lives in a helper that returns only the tracking id.
async function stoppedPumpApp(): Promise<string> {
  const app = BlitzApp.create();
  const handle = app.pumpLoop({targetPeriod: 0, timeout: 0});
  handle.stop();
  await handle.done;
  return track(app).id;
}

test("BlitzApp is finalized after its pumpLoop stops", async (t) => {
  const id = await stoppedPumpApp();
  t.true(await waitForFinalization(id), "BlitzApp was never finalized after pumpLoop stopped: pinned through the retained pump-loop handle");
});
