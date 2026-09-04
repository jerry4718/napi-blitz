# Basic：Pump 循环与线程模型

本文件介绍 napi-blitz 的泵循环（pump loop）概念、为什么叫 "pump"、
线程模型，以及日常用法。

## pump 是什么

pump（泵）是驱动整个应用事件循环的机制。一次 pump 调用从事件队列中
**泵出一批**待处理的 winit 事件交给 handler 处理（输入分发、blitz DOM
派发、渲染、窗口生命周期），处理完返回；应用的生命周期就是一次次
pump 拍的串联。

- **单拍**：`app.pumpAppEvents(timeoutMs)` —— 同步 NAPI 调用，最多阻塞
  `timeoutMs` 毫秒处理事件，返回 `PumpResult { continue, exit, code }`。
  每拍内部顺序：`poll_live_views`（让上一轮 JS 的 DOM 变更走
  poll → request_redraw 路径）→ `drain_closing_windows` → 合成退出检查
  → `event_loop.pump_app_events(timeout, handler)` → 再 drain/poll → 返回
- **循环**：`app.pumpLoop(options)` 启动异步泵循环

## 为什么叫 pump

命名直接来自 winit 的 `EventLoop::pump_app_events`（`pump_events` 族）：
**非阻塞地"泵出"一帧待处理事件**，与之相对的是 `run`/`run_app` 的
阻塞式主循环（接管线程直到退出）。泵的比喻：每次调用把事件队列里积压
的事件像水泵一样抽取上来、分发给 handler，`pump_app_events` 返回，
循环再抽下一批。

## 线程模型

单线程模型。全部 UI 代码——winit、blitz DOM、渲染、JS——运行在同一个
主线程（UI 线程）上：

- **winit EventLoop 必须留在主线程**：OS 窗口与输入事件在主线程投递
- **JS 事件循环是主线程的让出机制**：Rust 侧没有独占线程的循环；每拍
  JS 通过同步 NAPI 调用把控制权交给原生（`pumpAppEvents`），原生 pump
  完一轮事件后返回，JS 继续
- **一拍一次 NAPI 穿越**：主线程同步调用，不涉及 worker 线程
- **无锁、无跨线程共享**：DOM 突变在 JS 与 Rust 之间直接可见，不需要
  同步原语
- **代价**：pump 内的耗时就是 UI 卡顿——单拍必须尽快返回

循环骨架刻意放在 JS 侧：移回 Rust 每拍反而多两次 NAPI 穿越，而且主线程
的让出机制原本就是 JS 事件循环。

## 用法

```ts
const app = new BlitzApp();

// 顶层启动循环：默认 cadence 16.67ms（约 60fps）
const handle = app.pumpLoop({
  targetPeriod: 16.67, // 目标周期（毫秒）
  timeout: 16.67,      // 单拍最多阻塞等待事件的时间（默认 = targetPeriod）
  // signal: abortSignal, // 可选外部停止信号
});

app.addEventListener("pump", (e) => {
  const {result} = e;
  if (result.exit) {
    // 所有窗口已关闭，循环即将结束
  }
});

app.addEventListener("pump:end", (e) => {
  const {end} = e; // { kind: "exit" | "stop" | "abort", reason? }
});

// 主动停止（可选与之协调的 reason）
handle.stop("app-quit");
// await handle.done;
```

- **节奏**：循环锚定在绝对的 `performance.now()` 时间轴上——每拍睡到
  目标拍，`setTimeout` 的误差不会累积成 cadence 漂移；某拍超时（如重
  渲染）时立即续拍，并对齐 `now + targetPeriod`
- **事件流**：`pump:start` → `pump`（每拍携带 `PumpResult`）→ `pump:end`
  （`exit`/`stop`/`abort` 三种结束方式）；循环抛错时广播 `pump:error`
- **停止**：所有窗口关闭 → 原生报 `exit`；`handle.stop(reason)` → `stop`；
  外部 `AbortSignal` → `abort`
- **约束**：一个 app 同时只能有一个 pump 循环（重复调用抛错）；循环必须
  从顶层设置启动，不能在事件 handler 内启动——那样会重入原生循环