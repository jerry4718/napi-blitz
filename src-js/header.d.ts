/* eslint-disable */

interface TypedEventListener<M, K extends keyof M> {
  (this: TypedEventTarget<M>, ev: M[K]): any;
}

interface TypedEventListenerObject<M, K extends keyof M> {
  handleEvent(this: TypedEventTarget<M>, ev: M[K]): any;
}

/**
 * Event-target typed by an event map: `addEventListener`/`removeEventListener`
 * narrow each event name declared in `M` to its event class. At runtime this
 * is the `EventTarget` itself — no methods are overridden; the shape is
 * declared entirely by the factory's signature.
 */
export interface TypedEventTarget<M> {
  addEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListener<M, K>,
    options?: AddEventListenerOptions | boolean,
  ): void;

  addEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListenerObject<M, K>,
    options?: AddEventListenerOptions | boolean,
  ): void;

  addEventListener(
    type: string,
    listener: EventListener | EventListenerObject,
    options?: AddEventListenerOptions | boolean,
  ): void;

  removeEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListener<M, K>,
    options?: EventListenerOptions | boolean,
  ): void;

  removeEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListenerObject<M, K>,
    options?: EventListenerOptions | boolean,
  ): void;

  removeEventListener(
    type: string,
    listener: EventListener | EventListenerObject,
    options?: EventListenerOptions | boolean,
  ): void;

  dispatchEvent(event: M[keyof M] | Event): boolean;
}

/** Constructor shape of a typed `EventTarget`; usable directly as an `extends` base. */
export interface TypedEventTargetConstructor<M> {
  new(...args: any[]): TypedEventTarget<M>;
}

/** Merge two event maps: keys are the union, values dispatch per key (the second map wins). */
export type ExtendEventMap<A, B> = {
  [K in keyof A | keyof B]: K extends keyof B ? B[K] : K extends keyof A ? A[K] : never;
};

/** Extract the complete event map carried by a parent constructor. */
type ParentEventMap<C> = C extends abstract new (...args: any[]) => TypedEventTarget<infer P> ? P : {};

/** Derive a typed target from a root map or a parent class plus a delta. */
declare function TypedEventTarget<M>(Base: typeof EventTarget): TypedEventTargetConstructor<M>;
declare function TypedEventTarget<M, C extends abstract new (...args: any[]) => any>(
  Base: C,
): TypedEventTargetConstructor<ExtendEventMap<ParentEventMap<C>, M>> & C;
declare function TypedEventTarget<M>(Base: any): TypedEventTargetConstructor<M>;

/** Derive a typed target from a root map or a parent class plus a delta. */
declare function TypedEventTarget2<C extends abstract new (...args: any[]) => any, M>(
  Base: C,
): TypedEventTargetConstructor<ExtendEventMap<ParentEventMap<C>, M>> & C;

export type Anything = any;


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


/** Event map for `BlitzApp`, typing its `addEventListener`/`removeEventListener`. */
interface BlitzAppEventMap {
  "window:open": UiEvent;
  "window:opened": UiEvent;
  "window:close": UiEvent;
  "window:closed": UiEvent;
  "pump:start": PumpStartEvent;
  pump: PumpEvent;
  "pump:end": PumpEndEvent;
  "pump:error": PumpErrorEvent;
}


/*declare class SomeSpec {
  addEventListener(type: 'open', listener: (e: OpenEvent) => void): void;
}

declare class Yyy extends TypedEventTarget2<typeof EventTarget, WindowEventMap> {
  constructor(name: string)
}

declare const y: Yyy;


declare class Xxx extends TypedEventTarget2<typeof Yyy, BlitzAppEventMap>(Yyy) {
}

declare const x: Xxx;

x.addEventListener("pump:end", function (f) {

})*/

/* auto-generated by NAPI-RS */