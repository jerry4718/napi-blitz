// Standard DOM Event hierarchy.
//
// Node.js provides a built-in `Event` with `stopPropagation`,
// `preventDefault`, `stopImmediatePropagation`, `bubbles`,
// `cancelable`, `composed`, `defaultPrevented`, `cancelBubble`,
// `target`, `currentTarget`, `eventPhase`, `type`, `timeStamp`,
// `isTrusted`. We extend it with the standard UIEvent → MouseEvent →
// PointerEvent / WheelEvent chain and the KeyboardEvent, InputEvent,
// CompositionEvent, FocusEvent subclasses.

import type {EventPayload, ImeData, InputData, KeyData, PointerData, WheelData,} from "../native";

/**
 * Base class for every event we dispatch into the JS layer.
 *
 * `target`, `currentTarget`, and `eventPhase` are set by Rust via
 * `napi_define_properties` (see `dom/event.rs`): the `target` /
 * `currentTarget` getters wrap the node only when JS reads them.
 */
export class UIEvent extends Event {
  constructor(payload: EventPayload, init?: EventInit) {
    super(payload.type, {
      bubbles: payload.bubbles,
      cancelable: payload.cancelable,
      composed: false,
      ...init,
    });
  }
}

/** Mouse events: click, mousedown, mouseup, mousemove, etc. */
export class MouseEvent extends UIEvent {
  protected readonly _ptr: PointerData;

  constructor(payload: EventPayload, pointer: PointerData) {
    super(payload);
    this._ptr = pointer;
  }

  get screenX() {
    return this._ptr.screenX;
  }

  get screenY() {
    return this._ptr.screenY;
  }

  get clientX() {
    return this._ptr.clientX;
  }

  get clientY() {
    return this._ptr.clientY;
  }

  get pageX() {
    return this._ptr.pageX;
  }

  get pageY() {
    return this._ptr.pageY;
  }

  get button() {
    return this._ptr.button;
  }

  get buttons() {
    return this._ptr.buttons;
  }

  get ctrlKey() {
    return (this._ptr.modsBits & 2) !== 0;
  }

  get shiftKey() {
    return (this._ptr.modsBits & 1) !== 0;
  }

  get altKey() {
    return (this._ptr.modsBits & 4) !== 0;
  }

  get metaKey() {
    return (this._ptr.modsBits & 8) !== 0;
  }
}

/** Pointer events: pointerdown, pointermove, pointerup, etc. */
export class PointerEvent extends MouseEvent {
  get pointerId() {
    return this._ptr.pointerId;
  }

  get pointerType() {
    return this._ptr.kind;
  }

  get isPrimary() {
    return this._ptr.isPrimary;
  }

  get pressure() {
    return this._ptr.pressure;
  }

  get tiltX() {
    return this._ptr.tiltX;
  }

  get tiltY() {
    return this._ptr.tiltY;
  }

  get twist() {
    return this._ptr.twist;
  }
}

/** Wheel / scroll events. */
export class WheelEvent extends UIEvent {
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
    return this._wheel.mode === "lines" ? 1 : 0;
  }

  get pageX() {
    return this._wheel.pageX;
  }

  get pageY() {
    return this._wheel.pageY;
  }

  get clientX() {
    return this._wheel.clientX;
  }

  get clientY() {
    return this._wheel.clientY;
  }

  get buttons() {
    return this._wheel.buttons;
  }

  get ctrlKey() {
    return (this._wheel.modsBits & 2) !== 0;
  }

  get shiftKey() {
    return (this._wheel.modsBits & 1) !== 0;
  }

  get altKey() {
    return (this._wheel.modsBits & 4) !== 0;
  }

  get metaKey() {
    return (this._wheel.modsBits & 8) !== 0;
  }
}

/** Keyboard events: keydown, keyup. */
export class KeyboardEvent extends UIEvent {
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

  get ctrlKey() {
    return (this._key.modsBits & 2) !== 0;
  }

  get shiftKey() {
    return (this._key.modsBits & 1) !== 0;
  }

  get altKey() {
    return (this._key.modsBits & 4) !== 0;
  }

  get metaKey() {
    return (this._key.modsBits & 8) !== 0;
  }

  get text() {
    return this._key.text;
  }
}

/** Input events: input, beforeinput. */
export class InputEvent extends UIEvent {
  private readonly _input: InputData;

  constructor(payload: EventPayload, input: InputData) {
    super(payload);
    this._input = input;
  }

  get data() {
    return this._input.value;
  }

  get inputType() {
    return this.type;
  }
}

/** IME composition events: compositionstart, compositionupdate, compositionend. */
export class CompositionEvent extends UIEvent {
  private readonly _ime: ImeData;

  constructor(payload: EventPayload, ime: ImeData) {
    super(payload);
    this._ime = ime;
  }

  get data() {
    return this._ime.text ?? "";
  }
}

/** Focus events: focus, blur, focusin, focusout. */
export class FocusEvent extends UIEvent {
  constructor(payload: EventPayload) {
    super(payload);
  }
}

/**
 * Build the most specific event subclass for a given payload.
 */
export function buildEvent(payload: EventPayload): Event {
  if (payload.pointer) return new PointerEvent(payload, payload.pointer);
  if (payload.wheel) return new WheelEvent(payload, payload.wheel);
  if (payload.key) return new KeyboardEvent(payload, payload.key);
  if (payload.input) return new InputEvent(payload, payload.input);
  if (payload.ime) return new CompositionEvent(payload, payload.ime);
  return new UIEvent(payload);
}
