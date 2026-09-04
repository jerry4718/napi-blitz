# Events

napi-blitz's event model and dispatch support. The event class hierarchy
mirrors the DOM standard. Dispatch pipeline: the blitz engine
(`blitz-dom/src/events/`) produces DOM events, and `src/events/mod.rs`
`build_event` maps them to the corresponding JS classes.

## Event class hierarchy

Inheritance chain of each event class on the JS side (`#[layer]`'s
`type Parent`):

| Event class | Inherits from | Notes |
| --- | --- | --- |
| `Event` | — | Base class (root of all event classes such as UIEvent) |
| `UIEvent` | `Event` | Generic UI event base |
| `MouseEvent` | `UIEvent` | Mouse events |
| `PointerEvent` | `MouseEvent` | Pointer events, covering mouse/touch/pen |
| `WheelEvent` | `MouseEvent` | Wheel events |
| `KeyboardEvent` | `UIEvent` | Keyboard events |
| `InputEvent` | `UIEvent` | Text input events |
| `CompositionEvent` | `UIEvent` | IME composition events |
| `FocusEvent` | `UIEvent` | Focus events |

## Supported events

Recorded by `type`: the engine paths that produce these events and the JS
class each maps to. "Dispatched to" says whether the event reaches a node
or the window (the window only receives events that bubble out of the
document).

| type | JS event class | Inheritance chain | Dispatched to | Notes |
| --- | --- | --- | --- | --- |
| `pointerdown` `pointerup` `pointermove` `pointercancel` `pointerover` `pointerout` `pointerenter` `pointerleave` | `PointerEvent` | `PointerEvent → MouseEvent → UIEvent → Event` | node, window (bubbling ones, not hover) | UI input events; enter/leave do not bubble, never reach the window |
| `mousedown` `mouseup` `mousemove` `mouseover` `mouseout` `mouseenter` `mouseleave` | `MouseEvent` | `MouseEvent → UIEvent → Event` | node, window (bubbling ones, not hover) | Mouse-compatibility events synthesized from pointer events; enter/leave do not bubble |
| `click` `dblclick` `contextmenu` | `PointerEvent` | `PointerEvent → MouseEvent → UIEvent → Event` | node, window | Pointer synthesis (blitz builds them from pointer data, so the class is `PointerEvent`, not `MouseEvent`) |
| `wheel` | `WheelEvent` | `WheelEvent → MouseEvent → UIEvent → Event` | node, window | Wheel input |
| `keydown` `keyup` | `KeyboardEvent` | `KeyboardEvent → UIEvent → Event` | node, window | Keyboard input |
| `input` | `InputEvent` | `InputEvent → UIEvent → Event` | node, window | Text input, supported |
| `composition` | `CompositionEvent` | `CompositionEvent → UIEvent → Event` | node, window | IME commit/preedit |
| `focus` `blur` `focusin` `focusout` | `FocusEvent` | `FocusEvent → UIEvent → Event` | node | Focus changes; `focus`/`blur` do not bubble (focusin/focusout bubble, can reach the window) |
| `touchstart` `touchmove` `touchend` `touchcancel` | `UIEvent` | `UIEvent → Event` | node, window | Synthesized from finger/pen input; no dedicated `TouchEvent` class, falls back to `UIEvent` |

## Unsupported events

Attributes or mappings exist, but the engine currently produces no event
of these types (bound handlers are never called):

| type | Reason |
| --- | --- |
| `change` | No `Change` source — the `onchange` attribute is declared but not dispatched |
| `keypress` | No `KeyPress` source — the engine only produces `keydown`/`keyup` and `input`; a class mapping exists (`KeyboardEvent`) |
| `submit` | No form-submit source |
| `scroll` | Scrolling does not emit a `Scroll` event |
| `load` | No document/window load source |
| `afterprint` `beforeprint` `beforeunload` `error` `hashchange` `languagechange` `message` `messageerror` `offline` `online` `pagehide` `pageshow` `popstate` `rejectionhandled` `resize` `storage` `unhandledrejection` `unload` | No window-lifecycle source — the window currently only produces `close`/`closed` (`close`/`closed` are not in the `on<event>` tables) |
| `blur` `focus` (on window) | These do not bubble, the window never receives them |
| `mouseenter` `mouseleave` `pointerenter` `pointerleave` (on window) | Hover-chain events do not bubble; they stay on the node chain (the node itself receives them) |

`on<event>` attributes are defined on `Node.prototype` (interaction set),
`Window.prototype` (window events + bubbled interaction set), and
`HTMLBodyElement.prototype` (window-event forwarding set); the
"unsupported" types above are declared but not dispatched on those
prototypes.

## Event targets and `on<event>` attributes

`on<event>` IDL attributes are defined via `define_on_event_attributes`
on three prototypes (`Node`, `Window`, `HTMLBodyElement`); the other
event targets inherit them along the JS prototype chain. The table below
records the event-target hierarchy: which class each target inherits
from, which `on<event>` attributes it inherits from its parent, and
which it adds itself.

| Event target | Inherits from | Inherited `on<event>` (from parent) | Own `on<event>` additions |
| --- | --- | --- | --- |
| `EventTarget` | — | None (base class; only `addEventListener`/`removeEventListener`/`dispatchEvent`) | None |
| `Node` | `EventTarget` | None | Interaction set, 35: `click` `dblclick` `contextmenu` `mousedown` `mouseup` `mousemove` `mouseenter` `mouseleave` `mouseover` `mouseout` `pointerdown` `pointerup` `pointermove` `pointercancel` `pointerenter` `pointerleave` `pointerover` `pointerout` `touchstart` `touchmove` `touchend` `touchcancel` `keydown` `keyup` `keypress` `input` `change` `focus` `blur` `focusin` `focusout` `submit` `scroll` `wheel` `load` |
| `Element` | `Node` | Node interaction set (35) | None |
| `HTMLElement` | `Element` | Node interaction set (35) | None |
| `HTMLInputElement` | `HTMLElement` | Node interaction set (35) | None |
| `HTMLTextAreaElement` | `HTMLElement` | Node interaction set (35) | None |
| `HTMLBodyElement` | `HTMLElement` | Node interaction set (35) | Window-reflecting set, 22: `afterprint` `beforeprint` `beforeunload` `blur` `error` `focus` `hashchange` `languagechange` `load` `message` `messageerror` `offline` `online` `pagehide` `pageshow` `popstate` `rejectionhandled` `resize` `scroll` `storage` `unhandledrejection` `unload` (accessors forward to the window's attribute listener; the whole set is not dispatched) |
| `Document` | `Node` | Node interaction set (35) | None |
| `HTMLDocument` | `Document` | Node interaction set (35) | None |
| `Window` | `EventTarget` | None | Window events + bubbled interaction set, 49: `afterprint` `beforeprint` `beforeunload` `blur` `error` `focus` `hashchange` `languagechange` `load` `message` `messageerror` `offline` `online` `pagehide` `pageshow` `popstate` `rejectionhandled` `resize` `scroll` `storage` `unhandledrejection` `unload` `click` `dblclick` `contextmenu` `mousedown` `mouseup` `mousemove` `mouseenter` `mouseleave` `mouseover` `mouseout` `pointerdown` `pointerup` `pointermove` `pointercancel` `pointerenter` `pointerleave` `pointerover` `pointerout` `touchstart` `touchmove` `touchend` `touchcancel` `keydown` `keyup` `keypress` `input` `change` `focusin` `focusout` `submit` `wheel` |

`Window` is not a `Node` subclass (it inherits `EventTarget` directly), so
its 49 types never collide with the Node chain; `HTMLBodyElement`'s 22
types do not overlap the Node interaction set (`blur` `error` `focus`
`scroll` mean different things in the two tables — the Node table holds
the element's own DOM events, the body table forwards to the window).
`keypress` `change` `submit` `scroll` `load` are declared in the Node
interaction set but not dispatched (see "Unsupported events").