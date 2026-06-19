// `Text` — a CharacterData node holding text content.

import { CharacterData } from "./character-data";

export class Text extends CharacterData {
  /**
   * Spec: split this Text into two siblings at the given offset. The
   * original node retains everything before `offset`; a new Text node
   * containing the remainder is inserted as the next sibling and
   * returned.
   */
  splitText(offset: number): Text {
    const original = this.data;
    const tail = original.slice(offset);
    this.data = original.slice(0, offset);

    const doc = this._ownerDocument;
    const newId = doc._native.createTextNode(tail);
    const newText = doc._wrap(newId) as Text;

    const parent = this.parentNode;
    if (parent !== null) {
      parent.insertBefore(newText, this.nextSibling);
    }
    return newText;
  }
}
