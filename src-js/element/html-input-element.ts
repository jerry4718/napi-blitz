// `HTMLInputElement` - the user-facing class for `<input>` elements.
//
// Extends HTMLElement per the DOM standard. An `InputDataHandle` (native)
// is passed as the third constructor argument by `wrap_node` and stored
// for properties that need native-side access (value, checked, focused).
// Pure attribute-backed properties (type, disabled, placeholder, etc.)
// use the inherited getAttribute/setAttribute from Element.

import {HTMLElement} from "./html-element";
import type {InputDataHandle} from "../native";

export class HTMLInputElement extends HTMLElement {
  private readonly _inputData: InputDataHandle | null;

  /** @internal */
  constructor(handle: import("../native").NodeHandle, doc: import("../internal/internal").DocumentInternals, inputData?: InputDataHandle) {
    super(handle, doc);
    this._inputData = inputData ?? null;
  }

  // ---- Properties backed by InputDataHandle (native editor / special_data) ---

  get value(): string {
    return this._inputData?.value ?? this.getAttribute("value") ?? "";
  }

  set value(v: string) {
    if (this._inputData) {
      this._inputData.value = v;
    } else {
      this.setAttribute("value", v);
    }
  }

  get checked(): boolean {
    return this._inputData?.checked ?? this.hasAttribute("checked");
  }

  set checked(v: boolean) {
    if (this._inputData) {
      this._inputData.checked = v;
    } else if (v) {
      this.setAttribute("checked", "");
    } else {
      this.removeAttribute("checked");
    }
  }

  get focused(): boolean {
    return this._inputData?.focused ?? false;
  }

  // ---- Pure attribute-backed properties ---------------------------------------

  get type(): string {
    return this.getAttribute("type") ?? "text";
  }

  set type(v: string) {
    this.setAttribute("type", v);
  }

  get disabled(): boolean {
    return this.hasAttribute("disabled");
  }

  set disabled(v: boolean) {
    if (v) {
      this.setAttribute("disabled", "");
    } else {
      this.removeAttribute("disabled");
    }
  }

  get placeholder(): string {
    return this.getAttribute("placeholder") ?? "";
  }

  set placeholder(v: string) {
    this.setAttribute("placeholder", v);
  }

  get readOnly(): boolean {
    return this.hasAttribute("readonly");
  }

  set readOnly(v: boolean) {
    if (v) {
      this.setAttribute("readonly", "");
    } else {
      this.removeAttribute("readonly");
    }
  }

  get required(): boolean {
    return this.hasAttribute("required");
  }

  set required(v: boolean) {
    if (v) {
      this.setAttribute("required", "");
    } else {
      this.removeAttribute("required");
    }
  }

  get name(): string {
    return this.getAttribute("name") ?? "";
  }

  set name(v: string) {
    this.setAttribute("name", v);
  }

  get defaultValue(): string {
    return this.getAttribute("value") ?? "";
  }

  set defaultValue(v: string) {
    this.setAttribute("value", v);
  }
}
