// Web-spec Event subclasses we expose. We extend Node's built-in `Event`
// (Node >= 15 has it globally) so listeners get standard semantics for
// `stopPropagation()`, `preventDefault()`, and `stopImmediatePropagation()`.
//
// We deliberately do NOT expose the underlying nodeId or chain on the
// event. `event.target` is set automatically by the EventTarget machinery
// when Document dispatches along the chain (target first), and consumers
// who need ancestors can walk up via `Document.parentOf(...)`.

import type {EventPayload, ImeData, InputData, KeyData, PointerData, WheelData,} from "../native";

/** Forward declaration to avoid an import cycle with `document.ts`. */

// DocumentLike removed - buildEvent no longer needs a document reference.

/** Base class for every event we dispatch into the JS layer. */
export class BlitzDomEvent extends Event {
  constructor(payload: EventPayload, init?: EventInit) {
    super(payload.eventType, {
      bubbles: payload.bubbles,
      cancelable: payload.cancelable,
      composed: false,
      ...init,
    });
  }

  /**
   * @internal Called by Rust to set a lazy `target` getter. The getter
   * function (created by Rust via `create_function_from_closure`) calls
   * `wrap_node(nodeId)` only when JS code actually reads `event.target`.
   */
  __setLazyTarget(getter: () => EventTarget | null): void {
    Object.defineProperty(this, "target", {
      get: getter,
      configurable: true,
    });
  }

  /**
   * @internal Called by Rust to set a lazy `currentTarget` getter and
   * the current `eventPhase`. The getter wraps the node only on read.
   */
  __setLazyCurrentTarget(getter: () => EventTarget | null, phase: number): void {
    Object.defineProperty(this, "currentTarget", {
      get: getter,
      configurable: true,
    });
    Object.defineProperty(this, "eventPhase", {
      value: phase,
      configurable: true,
    });
  }
}

/** Pointer / mouse / click events. */
export class BlitzPointerEvent extends BlitzDomEvent {
  private readonly _pointer: PointerData;

  constructor(payload: EventPayload, pointer: PointerData) {
    super(payload);
    this._pointer = pointer;
  }

  get pageX() {
    return this._pointer.pageX;
  }

  get pageY() {
    return this._pointer.pageY;
  }

  get clientX() {
    return this._pointer.clientX;
  }

  get clientY() {
    return this._pointer.clientY;
  }

  get screenX() {
    return this._pointer.screenX;
  }

  get screenY() {
    return this._pointer.screenY;
  }

  get button() {
    return this._pointer.button;
  }

  get buttons() {
    return this._pointer.buttons;
  }

  get pressure() {
    return this._pointer.pressure;
  }

  get isPrimary() {
    return this._pointer.isPrimary;
  }

  get pointerType() {
    return this._pointer.kind;
  }

  get pointerId() {
    return this._pointer.pointerId;
  }
}

/** Mouse wheel / scroll wheel events. */
export class BlitzWheelEvent extends BlitzDomEvent {
  private readonly _wheel: WheelData;

  constructor(payload: EventPayload, wheel: WheelData) {
    super(payload);
    this._wheel = wheel;
  }

  get deltaX() {
    return this._wheel.deltaX;
  }

  get deltaY() {
    return this._wheel.deltaY;
  }

  get deltaMode() {
    return this._wheel.mode;
  }
}

/** Keyboard events. */
export class BlitzKeyboardEvent extends BlitzDomEvent {
  private readonly _key: KeyData;

  constructor(payload: EventPayload, key: KeyData) {
    super(payload);
    this._key = key;
  }

  get key() {
    return this._key.key;
  }

  get code() {
    return this._key.code;
  }

  get location() {
    return this._key.location;
  }

  get repeat() {
    return this._key.repeat;
  }

  get isComposing() {
    return this._key.isComposing;
  }

  get text() {
    return this._key.text;
  }
}

/** `<input>` value events. */
export class BlitzInputEvent extends BlitzDomEvent {
  private readonly _input: InputData;

  constructor(payload: EventPayload, input: InputData) {
    super(payload);
    this._input = input;
  }

  get value() {
    return this._input.value;
  }
}

/** IME composition events. */
export class BlitzImeEvent extends BlitzDomEvent {
  private readonly _ime: ImeData;

  constructor(payload: EventPayload, ime: ImeData) {
    super(payload);
    this._ime = ime;
  }

  get imeKind() {
    return this._ime.kind;
  }

  get text() {
    return this._ime.text;
  }
}

/**
 * Build the most specific BlitzDomEvent subclass for a given payload.
 */
export function buildEvent(payload: EventPayload): BlitzDomEvent {
  if (payload.pointer) return new BlitzPointerEvent(payload, payload.pointer);
  if (payload.wheel) return new BlitzWheelEvent(payload, payload.wheel);
  if (payload.key) return new BlitzKeyboardEvent(payload, payload.key);
  if (payload.input) return new BlitzInputEvent(payload, payload.input);
  if (payload.ime) return new BlitzImeEvent(payload, payload.ime);
  return new BlitzDomEvent(payload);
}
