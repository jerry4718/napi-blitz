# Possible future API shapes

There's a pain point right now. Opening an app takes two steps:

```ts
const app = BlitzApp.create();
app.pumpLoop();
```

And those two steps only start the pump loop — it makes people struggle
with "how to decide the pump details". Providing the capability is
necessary, but the "call" arguably shouldn't be mandatory.

## Form one: the loop starts while napi loads + global config APIs to tweak App / pump config at runtime

```ts
import {configApp, configPump} from "@ylcc/napi-blitz";
configApp({ xxx });
configPump({targetPeriod: 16.67, timeout: 100}); // the loop is already running when loaded
```

## Form two: a single global entry, construction and loop details hidden inside; the callback lets the app side run its own logic

```ts
import {createBlitz} from "@ylcc/napi-blitz";
createBlitz({appConfig: xxx, pumpConfig: xxx, ...}, (app: BlitzApp, pumpHandle: PumpHandle) => { ... });
```

I'm not sure which one would be more popular, or maybe a better design
can be imagined.

Besides, single-threaded with multiple windows may not be a good design,
though I don't really like IPC — IPC makes things more complicated.
Maybe there will be a dedicated IPC variant whose API feel is closer to a
browser, e.g. globally injected `document`/`window` instead of using them
as locals.

For now you can still spawn another process and DIY an IPC, just keep in
mind that only one App is allowed per process.