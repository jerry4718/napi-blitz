// `Comment` — a CharacterData node representing an HTML comment.
//
// The class is named `Comment` for spec alignment; we collide with the
// built-in `Comment` global only when running in a browser-like
// environment (Node.js doesn't ship one). If hosting code imports both
// our DOM and a browser global, alias on import.
//
// Caveat: blitz currently has no payload on `NodeData::Comment`, so
// writes to `data` are silently dropped at the native layer. We override
// the setter here to log a one-shot warning instead of letting users
// believe the write succeeded.

import { CharacterData } from "./character-data";

let warnedCommentDataIgnored = false;

export class Comment extends CharacterData {
  override get data(): string {
    return super.data;
  }
  override set data(value: string) {
    if (value !== "" && !warnedCommentDataIgnored) {
      warnedCommentDataIgnored = true;
      // Match the standard `console.warn` shape consumers expect from
      // user-agent diagnostics. Fired once per process.
      // eslint-disable-next-line no-console
      console.warn(
        "[napi-blitz] Comment.data writes are ignored: blitz's NodeData::Comment " +
          "currently has no string payload. The API is preserved for spec parity.",
      );
    }
    super.data = value;
  }
}

