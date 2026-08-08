// `HTMLTextAreaElement` - the user-facing class for `<textarea>` elements.
//
// Extends HTMLElement per the DOM standard. An `InputDataHandle` (native)
// is passed as the third constructor argument by `wrap_node` and stored
// for the `value` property (which needs native editor access).
// Pure attribute-backed properties (rows, cols, placeholder, etc.)
// use the inherited getAttribute/setAttribute from Element.

import {HTMLElement} from "./html-element";
import type {InputDataHandle} from "../native";

export class HTMLTextAreaElement extends HTMLElement {
  private readonly _inputData: InputDataHandle | null;

  /** @internal */
  constructor(handle: import("../native").NodeHandle, doc: import("../internal/internal").DocumentInternals, inputData?: InputDataHandle) {
    super(handle, doc);
    this._inputData = inputData ?? null;
  }

  // ---- Properties backed by InputDataHandle (native editor) ------------------

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

  get focused(): boolean {
    return this._inputData?.focused ?? false;
  }

  // ---- Pure attribute-backed properties ---------------------------------------

  get rows(): number {
    const v = parseInt(this.getAttribute("rows") ?? "", 10);
    return Number.isNaN(v) ? 2 : v;
  }

  set rows(v: number) {
    this.setAttribute("rows", String(v));
  }

  get cols(): number {
    const v = parseInt(this.getAttribute("cols") ?? "", 10);
    return Number.isNaN(v) ? 20 : v;
  }

  set cols(v: number) {
    this.setAttribute("cols", String(v));
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

  get defaultValue(): string {
    return this.getAttribute("value") ?? "";
  }

  set defaultValue(v: string) {
    this.setAttribute("value", v);
  }
}
