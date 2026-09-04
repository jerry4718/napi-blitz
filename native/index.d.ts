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
interface MouseEventMap {
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
interface PointerEventMap {
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
interface KeyboardEventMap {
  keydown: KeyboardEvent;
  keyup: KeyboardEvent;
  keypress: KeyboardEvent;
}

/** Text input events. */
interface InputEventMap {
  input: InputEvent;
}

/** IME composition events. */
interface CompositionEventMap {
  composition: CompositionEvent;
}

/** Focus events. */
interface FocusEventMap {
  focus: FocusEvent;
  blur: FocusEvent;
  focusin: FocusEvent;
  focusout: FocusEvent;
}

/** Wheel events. */
interface WheelEventMap {
  wheel: WheelEvent;
}

/** Events with no payload beyond the base `UIEvent`. */
interface UIEventMap {
  scroll: UIEvent;
  touchstart: UIEvent;
  touchmove: UIEvent;
  touchend: UIEvent;
  touchcancel: UIEvent;
}

/** Full event map of an `Element` per the DOM `GlobalEventHandlers`/`PointerEvents` mixins. */
interface ElementEventMap
  extends MouseEventMap,
    PointerEventMap,
    KeyboardEventMap,
    InputEventMap,
    CompositionEventMap,
    FocusEventMap,
    WheelEventMap,
    UIEventMap {
}

/** `Node` has no events of its own in the DOM standard. */
type NodeEventMap = {};

/** `Document` has no events of its own in the DOM standard. */
type DocumentEventMap = {};

/**
 * Window-level events: the lifecycle events Rust dispatches to the JS
 * `Window` (`close`/`closed`) plus the pointer/mouse events it forwards to
 * the window after the DOM chain walk (`dispatch_to_window`).
 */
interface WindowEventMap extends MouseEventMap, PointerEventMap {
  close: UIEvent;
  closed: UIEvent;
}

/** Event map for `BlitzApp`, typing its `addEventListener`/`removeEventListener`. */
interface BlitzAppEventMap {
  "window:open": UIEvent;
  "window:opened": UIEvent;
  "window:close": UIEvent;
  "window:closed": UIEvent;
  "pump:start": PumpStartEvent;
  pump: PumpEvent;
  "pump:end": PumpEndEvent;
  "pump:error": PumpErrorEvent;
}

export type ClassType<T extends abstract new (...args: any) => any> =
  & { new(...args: ConstructorParameters<T>): InstanceType<T> }
  & { [K in keyof T]: T[K] };

/* auto-generated by NAPI-RS */

export declare namespace NodeTypes {
  export const COMMENT_NODE: number
  export const DOCUMENT_NODE: number
  export const ELEMENT_NODE: number
  export const OTHER_NODE: number
  export const TEXT_NODE: number
}

/** Information about a monitor. Wraps winit's `MonitorHandle`. */
export declare class MonitorInfo {
  get id(): string
  get name(): string | null
  get x(): number
  get y(): number
  get scaleFactor(): number
  get currentVideoMode(): VideoModeInfo | null
  get videoModes(): Array<VideoModeInfo>
}

/** A fullscreen video mode of a monitor. Wraps winit's `VideoMode`. */
export declare class VideoModeInfo {
  get width(): number
  get height(): number
  get bitDepth(): number | null
  get refreshRateMillihertz(): number | null
}

/**
 * Opaque wrapper around a platform-specific raw window handle.
 *
 * Obtained from native objects (e.g. `NativeWindow.windowHandle()`).
 * Pass it to APIs that need a parent window, such as `WindowOptions.parentWindow()`,
 * or to `rfd` dialog calls.
 */
export declare class WindowHandle {

}

/**
 * Options accepted by `BlitzApp.openWindow`. Construct via
 * `WindowOptions.builder()`.
 */
export declare class WindowOptions {
  /** Create a new builder with all fields unset. */
  static builder(): WindowOptions
  title(value: string): this
  size(width: number, height: number): this
  resizable(value: boolean): this
  minSize(width: number, height: number): this
  maxSize(width: number, height: number): this
  maximized(value: boolean): this
  visible(value: boolean): this
  transparent(value: boolean): this
  blur(value: boolean): this
  decorations(value: boolean): this
  /** Set borderless fullscreen on the specified monitor. */
  fullscreenBorderless(monitor: MonitorInfo): this
  /** Set exclusive fullscreen using the specified monitor and video mode. */
  fullscreenExclusive(monitor: MonitorInfo, videoMode: VideoModeInfo): this
  enabledButtons(value: Array<string>): this
  windowIcon(value: Uint8Array): this
  /**
   * Set the parent window for this window.
   *
   * Pass a `RawWindowHandle` obtained from `NativeWindow.windowHandle()`.
   */
  parentWindow(handle: WindowHandle): this
}

/**
 * `dictionary AddEventListenerOptions : EventListenerOptions`.
 * The `signal` member is not implemented yet.
 */
export interface AddEventListenerOptions {
  capture?: boolean
  passive?: boolean
  once?: boolean
}

/** Plain attribute pair used by the create/insert APIs. */
export interface AttrInit {
  name: string
  value: string
  namespace?: string
}

/**
 * Create a new document from Rust and return the JS Document object
 * (an `HTMLDocument` layer chain).
 */
export declare function createDocument(config?: DocHandleConfig | undefined | null): object

/** `dictionary CustomEventInit : EventInit { any detail = null; }` */
export interface CustomEventInit {
  detail?: Anything
  bubbles?: boolean
  cancelable?: boolean
  composed?: boolean
}

/**
 * Define the window-reflecting attributes on `HTMLBodyElement.prototype`.
 * Called once from the JS bootstrap, like `defineNodeOnEventAttributes`.
 */
export declare function defineHtmlBodyEventAttributes(): void

/**
 * Define the `on<event>` IDL-style attributes on `Node.prototype`. Called
 * once from the JS bootstrap so every node (and derived element) exposes
 * the interaction handler attributes.
 */
export declare function defineNodeOnEventAttributes(): void

/**
 * Define the `on<event>` IDL-style attributes on `Window.prototype`. The
 * `WindowLayer` chain is `EventTarget`-rooted, so the attributes read and
 * write the window's own attribute listeners; called once from the JS
 * bootstrap.
 */
export declare function defineWindowEventAttributes(): void

/** Options shared by all dialog methods. */
export interface DialogOptions {
  /** Dialog title. */
  title?: string
  /** Starting directory. */
  directory?: string
  /** Starting file name (save dialog) or default name. */
  fileName?: string
  /** Extension filters. */
  filters?: Array<FileFilter>
}

/** Configuration passed to `createDocument`. */
export interface DocHandleConfig {
  uaStylesheets?: Array<string>
  baseHtml?: string
}

export interface DomRect {
  x: number
  y: number
  width: number
  height: number
  top: number
  left: number
  bottom: number
  right: number
}

/** `dictionary EventInit { boolean bubbles = false; boolean cancelable = false; boolean composed = false; }` */
export interface EventInit {
  bubbles?: boolean
  cancelable?: boolean
  composed?: boolean
}

/** `dictionary EventListenerOptions { boolean capture = false; }` */
export interface EventListenerOptions {
  capture?: boolean
}

/** Extension filter entry, e.g. `{ name: "Images", extensions: ["png", "jpg"] }`. */
export interface FileFilter {
  /** Display name shown in the filter dropdown. */
  name: string
  /** File extensions without leading dot, e.g. `["png", "jpg"]`. */
  extensions: Array<string>
}

/** `dictionary FontFaceDescriptors` — the descriptor subset we honor. */
export interface FontFaceDescriptors {
  style?: string
  weight?: string
  stretch?: string
  unicodeRange?: string
  variant?: string
  featureSettings?: string
  display?: string
}

export interface ListenerOps {
  insertListener: (target: EventTarget, listener: Function | { handleEvent: Function }, spec: ListenerSpec) => boolean
  deleteListener: (target: EventTarget, listener: Function | { handleEvent: Function }, spec: ListenerSpec) => boolean
}

export interface ListenerSpec {
  type: string
  capture: boolean
  kind: string
}

/**
 * `dictionary MessageEventInit : EventInit`.
 * `source` and `ports` are not implemented yet.
 */
export interface MessageEventInit {
  data?: Anything
  origin?: string
  lastEventId?: string
  bubbles?: boolean
  cancelable?: boolean
  composed?: boolean
}

/** Open a single-file picker. Returns the chosen path or `null`. */
export declare function pickFile(options?: DialogOptions | undefined | null, parent?: WindowHandle | undefined | null): Promise<string | null>

/** Open a multi-file picker. Returns an array of paths (may be empty). */
export declare function pickFiles(options?: DialogOptions | undefined | null, parent?: WindowHandle | undefined | null): Promise<Array<string>>

/** Open a single-folder picker. Returns the chosen path or `null`. */
export declare function pickFolder(options?: DialogOptions | undefined | null, parent?: WindowHandle | undefined | null): Promise<string | null>

/** Open a multi-folder picker. Returns an array of paths (may be empty). */
export declare function pickFolders(options?: DialogOptions | undefined | null, parent?: WindowHandle | undefined | null): Promise<Array<string>>

/** Result of one `pumpAppEvents` call. */
export interface PumpResult {
  /** The loop is still running. Caller should pump again later. */
  continue: boolean
  /** The loop has exited (e.g. all windows closed). */
  exit: boolean
  /** Exit code, if `exit`. */
  code?: number
}

/** Options for `DocHandle.registerFont`. */
export interface RegisterFontOptions {
  familyName?: string
  weight?: string
  style?: string
  stretch?: string
}

/** Open a save-file dialog. Returns the chosen path or `null`. */
export declare function saveFile(options?: DialogOptions | undefined | null, parent?: WindowHandle | undefined | null): Promise<string | null>

export declare function setListenerOps(ops: ListenerOps): void

/**
 * Register the JS-side `pumpAppLoop(app, options)` function that runs the
 * async pump loop, so `BlitzApp.pumpLoop` can forward to it.
 */
export declare function setPumpAppLoop(pumpAppLoop: object): void
export declare class AttributesHandlerClass {
  constructor()
  /** The `get` trap: the attribute's value, or `undefined` when absent. */
  get(target: Anything, prop: Anything, receiver: Anything): Anything
  /** The `set` trap: sets the content attribute. */
  set(target: Anything, prop: Anything, value: string, receiver: Anything): boolean
  /**
   * The `getOwnPropertyDescriptor` trap: `Object.keys` walks the
   * `ownKeys` result and asks for each key's descriptor, so present
   * attributes must answer with an enumerable descriptor.
   */
  getOwnPropertyDescriptor(target: Anything, prop: Anything): Anything
  /** The `has` trap: the attribute exists. */
  has(target: Anything, prop: Anything): boolean
  /** The `deleteProperty` trap: removes the content attribute. */
  deleteProperty(target: Anything, prop: Anything): boolean
  /** The `ownKeys` trap: all content attribute names. */
  ownKeys(target: Anything): Array<string>
}

/** Own block of the `BlitzApp` class. */
export declare class BlitzAppClass extends EventTarget {
  constructor()
  /**
   * Build the winit event loop and register this app as the lifecycle
   * dispatch target for app-level events (`window:open` and friends).
   */
  static create(): BlitzApp
  /**
   * Open a new window for an existing `HTMLDocument`.
   * Construct window attributes with `WindowOptions.builder()`.
   *
   * Async: the window is physically created by the next event-loop pump, so
   * this resolves once the OS window exists. Safe to call from inside an
   * event handler (e.g. a click) — the native side never recursively
   * pumps the event loop.
   *
   * Rust dispatches the cancelable app-level `window:open` event while
   * creating the window, before this promise resolves. A listener's
   * `preventDefault()` rejects this promise (the native side drops the
   * fresh view, so no `Window` is ever handed out).
   */
  openWindow(doc: object, options?: object | undefined | null): Promise<Anything>
  /**
   * Queue the given window for closure and return a promise that
   * resolves once the native `View` has actually been torn down, or
   * rejects if a `close` listener calls `preventDefault()`.
   */
  closeWindow(window: object): Promise<undefined>
  /**
   * List all available monitors with full metadata. Returns `[]` if
   * no windows have been created yet.
   */
  availableMonitors(): Array<MonitorInfo>
  /**
   * The primary monitor. Returns `None` if no windows have been
   * created yet.
   */
  primaryMonitor(): MonitorInfo | null
  /** Pump pending winit events for at most `millis` milliseconds. */
  pumpAppEvents(millis: number): PumpResult
  /**
   * Whether a pump loop is currently running, read from the loop object
   * the JS side returned for the latest `pumpLoop` call. The handle is
   * held weakly, so a collected handle reports `false`.
   */
  get pumping(): boolean
  /**
   * Start the async pump loop. The loop itself lives in JS (the bundle
   * registers `pumpAppLoop`), so this only forwards to it and keeps the
   * returned loop object for the `pumping` getter.
   * @deprecated
   */
  pumpLoop(options?: object | undefined | null): Anything
}

/** Own block of the `CharacterData` class. */
export declare class CharacterDataClass extends Node {
  constructor()
  get data(): string
  set data(data: string)
  /**
   * `CharacterData.length` — the size of the string in UTF-16 code
   * units, which is how the standard defines a string's length.
   */
  get length(): number
  /**
   * `CharacterData.nextElementSibling` — the first Element sibling after
   * this node, skipping non-element siblings.
   */
  get nextElementSibling(): Element | null
  /**
   * `CharacterData.previousElementSibling` — the first Element sibling
   * before this node, skipping non-element siblings.
   */
  get previousElementSibling(): Element | null
  /** `CharacterData.appendData` — append to the node's text. */
  appendData(data: string): void
}

/** Own block of the `Comment` class. */
export declare class CommentClass extends CharacterData {
  constructor()
}

/** Own block of the `CompositionEvent` class. */
export declare class CompositionEventClass extends UIEvent {
  constructor(type: string, init?: EventInit | undefined | null)
  get data(): string
}

/** Own block of the `CustomEvent` class. */
export declare class CustomEventClass extends Event {
  /**
   * `new CustomEvent(type, init?)` — `init` follows
   * `dictionary CustomEventInit`.
   */
  constructor(type: string, init?: CustomEventInit | undefined | null)
  get detail(): Anything
}

/**
 * Own block of the `Document` class. The blitz document node is always
 * the root node, so the members here work off `doc` alone (the parent
 * `NodeLayer` slot carries the `node_id`).
 */
export declare class DocumentClass extends Node {
  constructor()
  querySelector(selector: string): Element | null
  querySelectorAll(selector: string): Array<Element>
  getElementById(id: string): Element | null
  getElementsByTagName(name: string): Array<Element>
  getElementsByClassName(className: string): Array<Element>
  createElement(localName: string, namespace?: string | undefined | null, attrs?: Array<AttrInit> | undefined | null): Element
  createTextNode(text: string): Text
  createComment(text: string): Comment
  get documentElement(): Element | null
  get head(): Element | null
  get body(): Element | null
  get title(): string
  /**
   * `document.title = ...` — update the existing `<title>`'s text, or
   * create one inside `<head>` when the document has none.
   */
  set title(title: string)
}

/**
 * Own block of the `Element` class. Carries its own `node_id`/`doc` copy
 * (they never change once assigned) so the members here don't need to
 * re-materialize the parent `NodeLayer` slot on every call. The style and
 * attributes proxies live here so their lifetime is the wrapper's own:
 * no document-level cache pins them, and identity is stable for as long
 * as the wrapper is alive.
 */
export declare class ElementClass extends Node {
  constructor(type: string, init?: undefined | undefined | null)
  get tagName(): string | null
  /** `element.hasAttribute(name)` — presence of the content attribute. */
  hasAttribute(name: string): boolean
  getAttribute(name: string): string | null
  getAttributes(): Array<AttrInit>
  setAttribute(name: string, value: string, namespace?: string | undefined | null): void
  removeAttribute(name: string, namespace?: string | undefined | null): void
  getStyleProperty(name: string): string | null
  setStyleProperty(name: string, value: string): void
  removeStyleProperty(name: string): void
  getStylePropertyNames(): Array<string>
  getStyleAttribute(): string
  set innerHTML(html: string)
  /** `element.id` — mirrors the `id` content attribute. */
  get id(): string | null
  set id(id: string)
  /** `element.className` — mirrors the `class` content attribute. */
  get className(): string
  set className(value: string)
  /**
   * `element.getElementsByTagName` — tag-matching descendants in tree
   * order. Tag comparison is ASCII case-insensitive per the HTML spec.
   */
  getElementsByTagName(name: string): Array<Element>
  /**
   * `element.getElementsByClassName` — descendants whose `class`
   * attribute contains the exact token.
   */
  getElementsByClassName(className: string): Array<Element>
  get innerHTML(): string | null
  get outerHTML(): string | null
  querySelector(selector: string): Element | null
  querySelectorAll(selector: string): Array<Element>
  getBoundingClientRect(): DomRect | null
  get scrollTop(): number
  set scrollTop(value: number)
  get scrollLeft(): number
  set scrollLeft(value: number)
  get scrollHeight(): number
  get scrollWidth(): number
  get clientHeight(): number
  get clientWidth(): number
  focus(): boolean
  blur(): void
  /**
   * `element.style` — the CSSOM `CSSStyleDeclaration` Proxy over the
   * inline style block. The proxy is cached on the wrapper, so repeated
   * reads return the same object.
   */
  get style(): Anything
  /**
   * `element.attributes` — the NamedNodeMap-ish Proxy over the content
   * attributes. Cached on the wrapper like `style`.
   */
  get attributes(): Anything
}

/** Own block of the `Event` class. */
export declare class EventClass {
  readonly bubbles: boolean
  readonly cancelable: boolean
  readonly composed: boolean
  readonly timeStamp: number
  readonly isTrusted: boolean
  /** `new Event(type, init?)` — `init` follows `dictionary EventInit`. */
  constructor(type: string, init?: EventInit | undefined | null)
  get type(): string
  /** `event.target` — resolves the target only when read. */
  get target(): EventTarget | null
  /** `event.currentTarget` — the current receiver during dispatch. */
  get currentTarget(): EventTarget | null
  get eventPhase(): number
  get defaultPrevented(): boolean
  stopPropagation(): void
  /** `cancelBubble` mirrors whether `stopPropagation()` was called. */
  get cancelBubble(): boolean
  stopImmediatePropagation(): void
  preventDefault(): void
  /**
   * `event.composedPath()`. Placeholder: the dispatch chain is populated
   * by the dispatch side.
   */
  composedPath(): Array<EventTarget>
}

/** Own block of the `EventTarget` class. */
export declare class EventTargetClass {
  constructor()
  /**
   * `target.addEventListener(type, callback)` — `callback` may also be
   * registered as
   * `addEventListener(type, callback, options?)`, where `options` is an
   * `AddEventListenerOptions` object or a `useCapture` boolean.
   */
  addEventListener(eventType: string, callback: Anything, options?: AddEventListenerOptions | boolean | undefined | null): void
  /**
   * `target.removeEventListener(type, callback)` — `callback` may also be
   * unregistered as
   * `removeEventListener(type, callback, options?)`, where `options` is an
   * `EventListenerOptions` object or a `useCapture` boolean.
   */
  removeEventListener(eventType: string, callback: Anything, options?: EventListenerOptions | boolean | undefined | null): void
  /**
   * `target.dispatchEvent(event) -> boolean`. Invokes the matching
   * listeners, honouring `stopImmediatePropagation`; returns whether the
   * default was NOT prevented.
   * Dispatch a single event to this target's listeners. The event must
   * be an `Event`-derived layer instance; the canceled flag is read
   * back from its `EventLayer` state. `pub` so the Rust dispatch driver
   * (`napi-blitz-dom`) can invoke it directly.
   */
  dispatchEvent(event: object): boolean
}

/** Own block of the `FocusEvent` class. */
export declare class FocusEventClass extends UIEvent {
  constructor(type: string, init?: EventInit | undefined | null)
  get relatedTarget(): EventTarget | null
}

/** Own block of the `FontFace` class. */
export declare class FontFaceClass {
  /**
   * `new FontFace(family, source, descriptors?)` — the bytes are kept
   * until `FontFaceSet.add` registers them with the engine.
   */
  constructor(family: string, source: Uint8Array, descriptors?: FontFaceDescriptors | undefined | null)
  get family(): string
  set family(value: string)
  get style(): string
  set style(value: string)
  get weight(): string
  set weight(value: string)
  get stretch(): string
  set stretch(value: string)
  get unicodeRange(): string
  set unicodeRange(value: string)
  get variant(): string
  set variant(value: string)
  get featureSettings(): string
  set featureSettings(value: string)
  get display(): string
  set display(value: string)
  /** `"unloaded" | "loading" | "loaded" | "error"`. */
  get status(): string
  /** Promise resolving to this face once loaded. */
  get loaded(): Anything
  /**
   * Trigger loading. Buffer-backed faces complete synchronously, so the
   * returned promise is already resolved.
   */
  load(): Anything
}

/** Own block of the `FontFaceSet` class. */
export declare class FontFaceSetClass extends EventTarget {
  constructor()
  /** Always `"loaded"`: registration is synchronous. */
  get status(): string
  /** Already-resolved promise of this set. */
  get ready(): Anything
  /** Number of faces currently in the set. */
  get size(): number
  /**
   * Add `face` to the set, registering its bytes with the underlying
   * font cache. Returns the set (per spec).
   */
  add(face: object): Anything
  /**
   * Remove `face` from the set. The engine-side registration has no
   * unregister path, so the bytes stay resolvable inside the engine;
   * iteration stops yielding the face.
   */
  delete(face: object): boolean
  /** Whether `face` is currently in the set. */
  has(face: object): boolean
  /** Drop every face (same engine-side caveat as `delete`). */
  clear(): void
  /** Iterate over registered faces in insertion order. */
  forEach(callback: Anything, thisArg?: Anything | undefined | null): void
  keys(): Array<Anything>
  entries(): Array<Array<Anything>>
  /**
   * Not implemented: needs a CSS font-shorthand parser plus shaping
   * queries. Throwing keeps the gap obvious.
   */
  load(font: string, text?: string | undefined | null): void
  /** Same gap as `load`. */
  check(font: string, text?: string | undefined | null): void
  [Symbol.iterator](): IterableIterator<Anything>
}

/** Own block of the `HTMLBodyElement` class. */
export declare class HTMLBodyElementClass extends HTMLElement {
  constructor()
}

/** Own block of the `HTMLDocument` class. */
export declare class HTMLDocumentClass extends Document {
  constructor()
  static create(config?: DocHandleConfig | undefined | null): HTMLDocument
  /**
   * `document.fonts` — the document's `FontFaceSet`, created when the
   * document is initialized.
   */
  get fonts(): FontFaceSet | null
}

/** Own block of the `HTMLElement` class. */
export declare class HTMLElementClass extends Element {
  constructor()
}

/** Own block of the `HTMLHtmlElement` class. */
export declare class HTMLHtmlElementClass extends HTMLElement {
  constructor()
}

/** Own block of the `HTMLInputElement` class. */
export declare class HTMLInputElementClass extends HTMLElement {
  constructor()
  get value(): string
  set value(value: string)
  get checked(): boolean
  set checked(checked: boolean)
  get focused(): boolean
  get type(): string
  set type(value: string)
  get disabled(): boolean
  set disabled(value: boolean)
  get placeholder(): string
  set placeholder(value: string)
  /** `readOnly` mirrors the `readonly` content attribute. */
  get readOnly(): boolean
  set readOnly(value: boolean)
  get required(): boolean
  set required(value: boolean)
  get name(): string
  set name(value: string)
  /** `defaultValue` mirrors the `value` content attribute. */
  get defaultValue(): string
  set defaultValue(value: string)
}

/** Own block of the `HTMLTextAreaElement` class. */
export declare class HTMLTextAreaElementClass extends HTMLElement {
  constructor()
  get value(): string
  set value(value: string)
  get focused(): boolean
  /** `rows` defaults to 2 per the old implementation. */
  get rows(): number
  set rows(value: number)
  /** `cols` defaults to 20 per the old implementation. */
  get cols(): number
  set cols(value: number)
  get placeholder(): string
  set placeholder(value: string)
  /** `readOnly` mirrors the `readonly` content attribute. */
  get readOnly(): boolean
  set readOnly(value: boolean)
  get required(): boolean
  set required(value: boolean)
  get name(): string
  set name(value: string)
  get disabled(): boolean
  set disabled(value: boolean)
  /** `defaultValue` mirrors the `value` content attribute. */
  get defaultValue(): string
  set defaultValue(value: string)
}

/** Own block of the `InputEvent` class. */
export declare class InputEventClass extends UIEvent {
  constructor(type: string, init?: EventInit | undefined | null)
  get data(): string
}

/** Own block of the `KeyboardEvent` class. */
export declare class KeyboardEventClass extends UIEvent {
  readonly location: number
  readonly ctrlKey: boolean
  readonly shiftKey: boolean
  readonly altKey: boolean
  readonly metaKey: boolean
  readonly repeat: boolean
  readonly isComposing: boolean
  constructor(type: string, init?: EventInit | undefined | null)
  get key(): string
  get code(): string
}

/** Own block of the `MessageEvent` class. */
export declare class MessageEventClass extends Event {
  /**
   * `new MessageEvent(type, init?)` — `init` follows
   * `dictionary MessageEventInit`.
   */
  constructor(type: string, init?: MessageEventInit | undefined | null)
  get data(): Anything
  get origin(): string
  get lastEventId(): string
}

/** Own block of the `MouseEvent` class. */
export declare class MouseEventClass extends UIEvent {
  readonly screenX: number
  readonly screenY: number
  readonly clientX: number
  readonly clientY: number
  readonly ctrlKey: boolean
  readonly shiftKey: boolean
  readonly altKey: boolean
  readonly metaKey: boolean
  readonly button: number
  readonly buttons: number
  constructor(type: string, init?: EventInit | undefined | null)
  get relatedTarget(): EventTarget | null
}

/** Own block of the `Node` class. */
export declare class NodeClass extends EventTarget {
  constructor()
  get nodeType(): number
  get parentNode(): Node | null
  get firstChild(): Node | null
  get lastChild(): Node | null
  get nextSibling(): Node | null
  get previousSibling(): Node | null
  get childNodes(): Array<Node>
  /**
   * `node.contains(other)` — true for the node itself and its
   * descendants. Non-`Node` arguments are false, per spec.
   */
  contains(other: object): boolean
  get textContent(): string | null
  set textContent(text: string)
  appendChild(child: object): Node
  insertBefore(node: object, anchor?: object | undefined | null): Node
  /** `parent.removeChild(child)` — detach `child` and return it. */
  removeChild(child: object): Node
  remove(): void
  replaceWith(node: object): Node
  cloneNode(deep: boolean): Node
}

/** Own block of the `PointerEvent` class (pointer-specific fields). */
export declare class PointerEventClass extends MouseEvent {
  readonly pointerId: number
  readonly width: number
  readonly height: number
  readonly pressure: number
  readonly tangentialPressure: number
  readonly tiltX: number
  readonly tiltY: number
  readonly twist: number
  readonly isPrimary: boolean
  constructor(type: string, init?: EventInit | undefined | null)
  get pointerType(): string
}

/**
 * Own block of the `style` Proxy handler. Holds the element it serves
 * and its lazily-built CSSOM method slots.
 */
export declare class StyleHandlerClass {
  constructor()
  /**
   * The `get` trap: spec members (`cssText`, `length`, CSSOM
   * methods) resolve to their values; anything else is read as a CSS
   * property, camelCase mapped to kebab-case. Missing properties
   * read as `""`, matching CSSOM.
   */
  get(target: Anything, prop: Anything, receiver: Anything): Anything
  /**
   * The `getOwnPropertyDescriptor` trap: `Object.keys` walks the
   * `ownKeys` result and asks for each key's descriptor, so present
   * properties must answer with an enumerable descriptor.
   */
  getOwnPropertyDescriptor(target: Anything, prop: Anything): Anything
  /**
   * The `set` trap: writes a CSS property (camelCase or kebab),
   * except `cssText`, which replaces the whole block. Spec members
   * are read-only; setting them is ignored.
   */
  set(target: Anything, prop: Anything, value: string, receiver: Anything): boolean
  /**
   * The `has` trap: spec members are always present; a CSS property
   * is present when it exists in the block.
   */
  has(target: Anything, prop: Anything): boolean
  /** The `deleteProperty` trap: removes the property's declaration. */
  deleteProperty(target: Anything, prop: Anything): boolean
  /**
   * The `ownKeys` trap: the canonical (kebab-case) declaration
   * names, in declaration order.
   */
  ownKeys(target: Anything): Array<string>
}

/** Own block of the `Text` class. */
export declare class TextClass extends CharacterData {
  constructor()
}

/** Own block of the `UiEvent` class. */
export declare class UIEventClass extends Event {
  readonly detail: number
  constructor(type: string, init?: EventInit | undefined | null)
}

/** Own block of the `WheelEvent` class. */
export declare class WheelEventClass extends MouseEvent {
  readonly deltaX: number
  readonly deltaY: number
  readonly deltaZ: number
  readonly deltaMode: number
  constructor(type: string, init?: EventInit | undefined | null)
}

/**
 * Own block of the `Window` class. Constructed by the open-flow
 * (`Lifecycle::drain_opening_windows`), never from JS directly.
 */
export declare class WindowClass extends EventTarget {
  constructor()
  /**
   * The HTMLDocument painted in this window. Resolved through the
   * shared document's two-state reference: strong while the window is
   * live, weak after teardown, so no strong edge is parked on the
   * window wrapper itself.
   */
  get document(): HTMLDocument
  /** Whether `close()` has run for this window. */
  get closed(): boolean
  /**
   * Opaque window identifier. The lifecycle routes winit events back to
   * the right window by it.
   */
  get windowId(): bigint
  /**
   * Get the raw window handle for this window.
   *
   * The returned `RawWindowHandle` can be passed to `WindowOptions.parentWindow()`
   * to create child windows, or to `rfd` dialogs that need a parent window.
   */
  windowHandle(): WindowHandle
  setTitle(title: string): void
  /**
   * Replace the window's document the way assigning `location.href`
   * would: a fresh document object is built and swapped in, and the
   * old document — wrappers, caches and all — is retired. The swap is
   * the cycle-breaking switch: the old document drops every native
   * strong edge (`detach_window`) and the new one gains them
   * (`attach_window`) in the same step. Returns the fresh document,
   * like `DOMParser.parseFromString` does for its parsed document.
   * This is a blitz-specific navigation API, not a DOM-standard one,
   * so it lives on the window rather than on `Document`.
   */
  loadHtml(html: string): HTMLDocument
  setSize(width: number, height: number): void
  resize(width: number, height: number): void
  getSize(): Array<number>
  getResizable(): boolean
  currentMonitor(): MonitorInfo | null
  setMinSize(width: number, height: number): void
  setMaxSize(width: number, height: number): void
  setResizable(value: boolean): void
  setMaximized(value: boolean): void
  setVisible(value: boolean): void
  setTransparent(value: boolean): void
  setBlur(value: boolean): void
  setDecorations(value: boolean): void
  setFullscreenBorderless(monitor: MonitorInfo): void
  setFullscreenExclusive(monitor: MonitorInfo, videoMode: VideoModeInfo): void
  setFullscreenNone(): void
  setEnabledButtons(buttons: Array<string>): void
  setWindowIcon(data: Uint8Array): void
  /**
   * Set the document zoom level. `1.0` is unzoomed. Combined with the
   * system scale factor to produce the total viewport scale
   * (`hidpi_scale * zoom`) that scales layout and CSS transforms.
   */
  setZoom(zoom: number): void
  /** Get the current document zoom level. */
  getZoom(): number
  /**
   * Queue this window for closure and return a promise that resolves
   * once the native `View` has actually been torn down (during the next
   * pump), or rejects if a `close` listener calls `preventDefault()`.
   * `close()` is idempotent.
   */
  close(): Promise<undefined>
  /**
   * Register `callback` to run before the next redraw of this window;
   * returns a handle for `cancelAnimationFrame`. Registering while none
   * is pending requests a redraw, so a waiting callback is always
   * scheduled. Callbacks stay alive until they run, are cancelled, or
   * the window closes.
   */
  requestAnimationFrame(callback: (arg0: number) => Anything): number
  /**
   * Cancel a callback previously registered with
   * `requestAnimationFrame`; its reference is released here.
   */
  cancelAnimationFrame(handle: number): void
}

export type AttributesHandler =
  InstanceType<typeof AttributesHandlerClass>;

export declare const AttributesHandler: ClassType<typeof AttributesHandlerClass>;

export type BlitzApp =
  InstanceType<typeof BlitzAppClass>;

export declare const BlitzApp: ClassType<typeof BlitzAppClass>;

export type CharacterData =
  InstanceType<typeof CharacterDataClass>;

export declare const CharacterData: ClassType<typeof CharacterDataClass>;

export type Comment =
  InstanceType<typeof CommentClass>;

export declare const Comment: ClassType<typeof CommentClass>;

export type CompositionEvent =
  InstanceType<typeof CompositionEventClass>;

export declare const CompositionEvent: ClassType<typeof CompositionEventClass>;

export type CustomEvent =
  InstanceType<typeof CustomEventClass>;

export declare const CustomEvent: ClassType<typeof CustomEventClass>;

export type Document =
  InstanceType<typeof DocumentClass>;

export declare const Document: ClassType<typeof DocumentClass>;

export type Element =
  InstanceType<typeof ElementClass>;

export declare const Element: ClassType<typeof ElementClass>;

export type Event =
  InstanceType<typeof EventClass>;

export declare const Event: ClassType<typeof EventClass>;

export type EventTarget =
  InstanceType<typeof EventTargetClass>;

export declare const EventTarget: ClassType<typeof EventTargetClass>;

export type FocusEvent =
  InstanceType<typeof FocusEventClass>;

export declare const FocusEvent: ClassType<typeof FocusEventClass>;

export type FontFace =
  InstanceType<typeof FontFaceClass>;

export declare const FontFace: ClassType<typeof FontFaceClass>;

export type FontFaceSet =
  InstanceType<typeof FontFaceSetClass>;

export declare const FontFaceSet: ClassType<typeof FontFaceSetClass>;

export type HTMLBodyElement =
  InstanceType<typeof HTMLBodyElementClass>;

export declare const HTMLBodyElement: ClassType<typeof HTMLBodyElementClass>;

export type HTMLDocument =
  InstanceType<typeof HTMLDocumentClass>;

export declare const HTMLDocument: ClassType<typeof HTMLDocumentClass>;

export type HTMLElement =
  InstanceType<typeof HTMLElementClass>;

export declare const HTMLElement: ClassType<typeof HTMLElementClass>;

export type HTMLHtmlElement =
  InstanceType<typeof HTMLHtmlElementClass>;

export declare const HTMLHtmlElement: ClassType<typeof HTMLHtmlElementClass>;

export type HTMLInputElement =
  InstanceType<typeof HTMLInputElementClass>;

export declare const HTMLInputElement: ClassType<typeof HTMLInputElementClass>;

export type HTMLTextAreaElement =
  InstanceType<typeof HTMLTextAreaElementClass>;

export declare const HTMLTextAreaElement: ClassType<typeof HTMLTextAreaElementClass>;

export type InputEvent =
  InstanceType<typeof InputEventClass>;

export declare const InputEvent: ClassType<typeof InputEventClass>;

export type KeyboardEvent =
  InstanceType<typeof KeyboardEventClass>;

export declare const KeyboardEvent: ClassType<typeof KeyboardEventClass>;

export type MessageEvent =
  InstanceType<typeof MessageEventClass>;

export declare const MessageEvent: ClassType<typeof MessageEventClass>;

export type MouseEvent =
  InstanceType<typeof MouseEventClass>;

export declare const MouseEvent: ClassType<typeof MouseEventClass>;

export type Node =
  InstanceType<typeof NodeClass>;

export declare const Node: ClassType<typeof NodeClass>;

export type PointerEvent =
  InstanceType<typeof PointerEventClass>;

export declare const PointerEvent: ClassType<typeof PointerEventClass>;

export type StyleHandler =
  InstanceType<typeof StyleHandlerClass>;

export declare const StyleHandler: ClassType<typeof StyleHandlerClass>;

export type Text =
  InstanceType<typeof TextClass>;

export declare const Text: ClassType<typeof TextClass>;

export type UIEvent =
  InstanceType<typeof UIEventClass>;

export declare const UIEvent: ClassType<typeof UIEventClass>;

export type WheelEvent =
  InstanceType<typeof WheelEventClass>;

export declare const WheelEvent: ClassType<typeof WheelEventClass>;

export type Window =
  InstanceType<typeof WindowClass>;

export declare const Window: ClassType<typeof WindowClass>;
