# Events

napi-blitz 的事件模型与派发支持。事件类层级对应 DOM 标准，派发由
blitz 引擎（`blitz-dom/src/events/`）产生 DOM 事件、`src/events/mod.rs`
的 `build_event` 映射到对应 JS 类。

## 事件类层级

每个事件类在 JS 侧的继承链（`#[layer]` 的 `type Parent`）：

| 事件类 | 继承自 | 说明 |
| --- | --- | --- |
| `Event` | — | 基类（UIEvent 等所有事件类的根） |
| `UIEvent` | `Event` | 通用 UI 事件基类 |
| `MouseEvent` | `UIEvent` | 鼠标事件 |
| `PointerEvent` | `MouseEvent` | 指针事件，含鼠标/触摸/笔 |
| `WheelEvent` | `MouseEvent` | 滚轮事件 |
| `KeyboardEvent` | `UIEvent` | 键盘事件 |
| `InputEvent` | `UIEvent` | 文本输入事件 |
| `CompositionEvent` | `UIEvent` | IME 组合事件 |
| `FocusEvent` | `UIEvent` | 焦点事件 |

## 支持的事件

按 type 记录：产生这些事件的引擎路径与映射到的 JS 类。`派发目标`
说明事件到 node 还是 window（window 只收到冒泡出文档的事件）。

| type | JS 事件类 | 继承链 | 派发目标 | 备注 |
| --- | --- | --- | --- | --- |
| `pointerdown` `pointerup` `pointermove` `pointercancel` `pointerover` `pointerout` `pointerenter` `pointerleave` | `PointerEvent` | `PointerEvent → MouseEvent → UIEvent → Event` | node、window（非 hover 的冒泡） | UI 输入事件；enter/leave 不冒泡，到不了 window |
| `mousedown` `mouseup` `mousemove` `mouseover` `mouseout` `mouseenter` `mouseleave` | `MouseEvent` | `MouseEvent → UIEvent → Event` | node、window（非 hover 的冒泡） | 指针事件的鼠标兼容事件（合成）；enter/leave 不冒泡 |
| `click` `dblclick` `contextmenu` | `PointerEvent` | `PointerEvent → MouseEvent → UIEvent → Event` | node、window | 指针合成（blitz 用指针数据构造，故类为 `PointerEvent` 而非 `MouseEvent`） |
| `wheel` | `WheelEvent` | `WheelEvent → MouseEvent → UIEvent → Event` | node、window | 滚轮输入 |
| `keydown` `keyup` | `KeyboardEvent` | `KeyboardEvent → UIEvent → Event` | node、window | 键盘输入 |
| `input` | `InputEvent` | `InputEvent → UIEvent → Event` | node、window | 文本输入，支持 |
| `composition` | `CompositionEvent` | `CompositionEvent → UIEvent → Event` | node、window | IME commit/preedit |
| `focus` `blur` `focusin` `focusout` | `FocusEvent` | `FocusEvent → UIEvent → Event` | node | 焦点变化；`focus`/`blur` 不冒泡（focusin/focusout 冒泡，可到 window） |
| `touchstart` `touchmove` `touchend` `touchcancel` | `UIEvent` | `UIEvent → Event` | node、window | 手指/笔输入合成；没有独立的 `TouchEvent` 类，落到 `UIEvent` |

## 不支持的事件

属性或映射存在、但引擎当前不产生该事件（绑定 handler 不会被调用）：

| type | 原因 |
| --- | --- |
| `change` | 无 `Change` 事件源——`onchange` 属性已声明但不派发 |
| `keypress` | 无 `KeyPress` 事件源——引擎只产生 `keydown`/`keyup` 与 `input`；类映射存在（`KeyboardEvent`） |
| `submit` | 无表单提交事件源 |
| `scroll` | 滚动不产生 `Scroll` 事件 |
| `load` | 无 document/window 的 load 事件源 |
| `afterprint` `beforeprint` `beforeunload` `error` `hashchange` `languagechange` `message` `messageerror` `offline` `online` `pagehide` `pageshow` `popstate` `rejectionhandled` `resize` `storage` `unhandledrejection` `unload` | 无 window 生命周期事件源——window 目前只产生 `close`/`closed`（`close`/`closed` 不在 `on<event>` 表中） |
| `blur` `focus`（到 window） | `focus`/`blur` 不冒泡，window 收不到 |
| `mouseenter` `mouseleave` `pointerenter` `pointerleave`（到 window） | hover 链事件不冒泡，停留在节点链上（节点自身会收到） |

`on<event>` 属性在 `Node.prototype`（交互集）、`Window.prototype`
（窗口事件 + 冒泡交互集）、`HTMLBodyElement.prototype`（窗口事件转发
集）上分别定义；上述"不支持"类型在这些原型上均只声明属性、不派发。

## 事件目标类与 `on<event>` 属性

`on<event>` IDL 属性通过 `define_on_event_attributes` 定义在三个原型的
`prototype` 上（`Node`、`Window`、`HTMLBodyElement`），其余事件目标类
沿 JS 原型链继承。下表按事件目标类的层级记录：每个类继承自谁、继承
了父类的哪些事件、自身又新增了哪些事件。

| 事件目标类 | 继承自 | 继承的 `on<event>`（来自父类） | 自身新增的 `on<event>` |
| --- | --- | --- | --- |
| `EventTarget` | — | 无（基类，仅 `addEventListener`/`removeEventListener`/`dispatchEvent`） | 无 |
| `Node` | `EventTarget` | 无 | 交互集 35 项：`click` `dblclick` `contextmenu` `mousedown` `mouseup` `mousemove` `mouseenter` `mouseleave` `mouseover` `mouseout` `pointerdown` `pointerup` `pointermove` `pointercancel` `pointerenter` `pointerleave` `pointerover` `pointerout` `touchstart` `touchmove` `touchend` `touchcancel` `keydown` `keyup` `keypress` `input` `change` `focus` `blur` `focusin` `focusout` `submit` `scroll` `wheel` `load` |
| `Element` | `Node` | Node 交互集 35 项 | 无 |
| `HTMLElement` | `Element` | Node 交互集 35 项 | 无 |
| `HTMLInputElement` | `HTMLElement` | Node 交互集 35 项 | 无 |
| `HTMLTextAreaElement` | `HTMLElement` | Node 交互集 35 项 | 无 |
| `HTMLBodyElement` | `HTMLElement` | Node 交互集 35 项 | window 反射集 22 项：`afterprint` `beforeprint` `beforeunload` `blur` `error` `focus` `hashchange` `languagechange` `load` `message` `messageerror` `offline` `online` `pagehide` `pageshow` `popstate` `rejectionhandled` `resize` `scroll` `storage` `unhandledrejection` `unload`（访问器转发到 window 的 attribute listener；整组不派发） |
| `Document` | `Node` | Node 交互集 35 项 | 无 |
| `HTMLDocument` | `Document` | Node 交互集 35 项 | 无 |
| `Window` | `EventTarget` | 无 | 窗口事件 + 冒泡交互集 49 项：`afterprint` `beforeprint` `beforeunload` `blur` `error` `focus` `hashchange` `languagechange` `load` `message` `messageerror` `offline` `online` `pagehide` `pageshow` `popstate` `rejectionhandled` `resize` `scroll` `storage` `unhandledrejection` `unload` `click` `dblclick` `contextmenu` `mousedown` `mouseup` `mousemove` `mouseenter` `mouseleave` `mouseover` `mouseout` `pointerdown` `pointerup` `pointermove` `pointercancel` `pointerenter` `pointerleave` `pointerover` `pointerout` `touchstart` `touchmove` `touchend` `touchcancel` `keydown` `keyup` `keypress` `input` `change` `focusin` `focusout` `submit` `wheel` |

`Window` 不是 `Node` 子类（直接继承 `EventTarget`），所以它的 49 项与
Node 链互不覆盖；`HTMLBodyElement` 的 22 项与 Node 交互集不重叠
（`blur` `error` `focus` `scroll` 在两表中含义不同——Node 表是节点自身
的 DOM 事件，body 表转发到 window）。`keypress` `change` `submit`
`scroll` `load` 在 Node 交互集中已声明但不派发（见"不支持的事件"）。