// `Element` HTML serialization: innerHTML / outerHTML.

import test from "ava";

import { HTMLDocument } from "../dist/index.js";

test("innerHTML / outerHTML reflect the tree", (t) => {
  const doc = new HTMLDocument();
  const div = doc.createElement("div");
  doc.body!.appendChild(div);
  div.innerHTML = "<span>hi</span>";

  t.true(div.innerHTML.includes("<span"));
  t.true(div.innerHTML.includes("hi"));
  t.true(div.outerHTML.startsWith("<div"));
  t.true(div.outerHTML.endsWith("</div>"));
});
