# 未来 API 的可能变化形式

目前有个痛点。开一个应用要两步：

```ts
const app = BlitzApp.create();
app.pumpLoop();
```

而这两步还只是把 pump 循环开起来，这会让人们困扰于"如何决定 pump 的细节"。提供能力是必要的，但"调用"似乎不应该是必须的。

## 形式一：napi 加载期间自动开启循环 + 全局配置接口实时调整 App / pump 配置

```ts
import {configApp, configPump} from "@ylcc/napi-blitz";
configApp({ xxx });
configPump({targetPeriod: 16.67, timeout: 100}); // 循环在加载时已自动开启
```

## 形式二：全局的单一入口，构造和循环等细节隐藏在内部，回调形式激发侧还能刚需逻辑

```ts
import {createBlitz} from "@ylcc/napi-blitz";
createBlitz({appConfig: xxx,pumpConfig: xxx, ...}, (app: BlitzApp, pumpHandle: PumpHandle) => { ... });
```

我不确定哪一种更受欢迎，或者可以想象更好的方案。

另外，单线程加多窗口也许并不是一个好的设计，不过我不是非常喜欢 IPC，IPC 会把问题变复杂。也可能未来会做一个专门的 IPC 版本，api 体验上更接近浏览器，比如全局注入 document/window，而不是以局部变量的方式使用。
目前，仍旧可以通过 spawn 出另一个进程，然后 diy 一个 IPC，只是注意一个进程内只能有一个 App 即可