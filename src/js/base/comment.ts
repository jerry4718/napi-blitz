// `Comment` — a CharacterData node representing an HTML comment.
//
// The class is named `Comment` for spec alignment; we collide with the
// built-in `Comment` global only when running in a browser-like
// environment (Node.js doesn't ship one). If hosting code imports both
// our DOM and a browser global, alias on import.

import { CharacterData } from "./character-data";

export class Comment extends CharacterData {
  // No additional members beyond CharacterData.
}
