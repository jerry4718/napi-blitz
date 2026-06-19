// `CharacterData` — abstract intermediate class for nodes whose
// data is a single string (Text, Comment).
//
// Mirrors the web spec. We delegate to `textContent` on the native side
// since blitz doesn't distinguish a separate `data` slot.
//
// Caveat: blitz currently models `NodeData::Comment` as a unit variant
// (no payload). Reads return "" and writes are silently dropped for
// Comment nodes until blitz grows a string payload. Text nodes work as
// expected.

import { Node } from "./node";

export abstract class CharacterData extends Node {
  /** The character data of this node (Text content or Comment text). */
  get data(): string {
    return this._handle.textContent(this._nodeId) ?? "";
  }
  set data(value: string) {
    this._handle.setTextContent(this._nodeId, value);
  }

  get length(): number {
    return this.data.length;
  }

  /** Spec: returns the node's data. */
  substringData(offset: number, count: number): string {
    return this.data.substring(offset, offset + count);
  }

  appendData(value: string): void {
    this.data = this.data + value;
  }

  insertData(offset: number, value: string): void {
    const cur = this.data;
    this.data = cur.slice(0, offset) + value + cur.slice(offset);
  }

  deleteData(offset: number, count: number): void {
    const cur = this.data;
    this.data = cur.slice(0, offset) + cur.slice(offset + count);
  }

  replaceData(offset: number, count: number, value: string): void {
    const cur = this.data;
    this.data = cur.slice(0, offset) + value + cur.slice(offset + count);
  }
}
