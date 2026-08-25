// DOM event maps, grouped by the standard interface that owns them. Each map
// is a `type` -> event-class table for the events blitz actually dispatches
// from Rust (`DomEventData`). Maps are assembled from small reusable blocks so
// subclasses can pick up exactly the events the standard assigns to them.

import {
  CompositionEvent,
  FocusEvent,
  InputEvent,
  KeyboardEvent,
  MouseEvent,
  PointerEvent,
  UIEvent,
  WheelEvent,
} from "./events";

/** Pointer/mouse events: `MouseEvent` subclasses with coordinates/buttons. */
export interface MouseEventMap {
  click: MouseEvent;
  contextmenu: MouseEvent;
  dblclick: MouseEvent;
  mousemove: MouseEvent;
  mousedown: MouseEvent;
  mouseup: MouseEvent;
  mouseenter: MouseEvent;
  mouseleave: MouseEvent;
  mouseover: MouseEvent;
  mouseout: MouseEvent;
}

/** Pointer events: `PointerEvent` (extends `MouseEvent`). */
export interface PointerEventMap {
  pointermove: PointerEvent;
  pointerdown: PointerEvent;
  pointerup: PointerEvent;
  pointercancel: PointerEvent;
  pointerenter: PointerEvent;
  pointerleave: PointerEvent;
  pointerover: PointerEvent;
  pointerout: PointerEvent;
}

/** Keyboard events. */
export interface KeyboardEventMap {
  keydown: KeyboardEvent;
  keyup: KeyboardEvent;
  keypress: KeyboardEvent;
}

/** Text input events. */
export interface InputEventMap {
  input: InputEvent;
}

/** IME composition events. */
export interface CompositionEventMap {
  composition: CompositionEvent;
}

/** Focus events. */
export interface FocusEventMap {
  focus: FocusEvent;
  blur: FocusEvent;
  focusin: FocusEvent;
  focusout: FocusEvent;
}

/** Wheel events. */
export interface WheelEventMap {
  wheel: WheelEvent;
}

/** Events with no payload beyond the base `UIEvent`. */
export interface UIEventMap {
  scroll: UIEvent;
  touchstart: UIEvent;
  touchmove: UIEvent;
  touchend: UIEvent;
  touchcancel: UIEvent;
}

/** Full event map of an `Element` per the DOM `GlobalEventHandlers`/`PointerEvents` mixins. */
export interface ElementEventMap
  extends MouseEventMap,
    PointerEventMap,
    KeyboardEventMap,
    InputEventMap,
    CompositionEventMap,
    FocusEventMap,
    WheelEventMap,
    UIEventMap {}

/** `Node` has no events of its own in the DOM standard. */
export type NodeEventMap = {};

/** `Document` has no events of its own in the DOM standard. */
export type DocumentEventMap = {};

/**
 * Window-level events: the lifecycle events Rust dispatches to the JS
 * `Window` (`close`/`closed`) plus the pointer/mouse events it forwards to
 * the window after the DOM chain walk (`dispatch_to_window`).
 */
export interface WindowEventMap extends MouseEventMap, PointerEventMap {
  close: UIEvent;
  closed: UIEvent;
}
