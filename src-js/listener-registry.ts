// Protocol types come from the internal module, which forwards the generated
// native declarations; aliased so the type binding does not merge with the
// exported constant below (TS2395).
import type {ListenerOps as ListenerOpsSpec, ListenerSpec} from "./internal";
import type {EventTarget} from "./native.ts";

type ListenerKind = "basic" | "attribute";

type Listener = Function | { handleEvent: Function };

type ListenerEntry = {
  type: string;
  kind: ListenerKind;
  callback: Listener;
  capture: boolean;
};

// The registry is the only strong holder of a listener callback: entries
// are keyed by the weakly-held target, so the callback lives and dies with
// its target. A callback closure that captures JS objects therefore cannot
// form a native-rooted reference cycle.
const ListenerRegistry = new WeakMap<object, Array<ListenerEntry>>();

function entriesOf(target: object): Array<ListenerEntry> {
  const existing = ListenerRegistry.get(target);
  if (existing !== undefined) return existing;
  const created: Array<ListenerEntry> = [];
  ListenerRegistry.set(target, created);
  return created;
}

function insertListener(target: EventTarget, listener: Listener, spec: ListenerSpec): boolean {
  const entries = entriesOf(target);
  const duplicate = entries.some(
    (entry) =>
      // Same (type, callback, capture, kind) quadruple: `on<event> = fn` and
      // `addEventListener(event, fn)` may coexist for one function.
      entry.type === spec.type && entry.capture === spec.capture && entry.kind === spec.kind && entry.callback === listener,
  );
  if (duplicate) return false;
  entries.push({type: spec.type, kind: spec.kind as ListenerKind, callback: listener, capture: spec.capture});
  return true;
}

function deleteListener(target: EventTarget, listener: Listener, spec: ListenerSpec): boolean {
  const entries = ListenerRegistry.get(target);
  if (entries === undefined) return false;
  const index = entries.findIndex(
    (entry) =>
      entry.type === spec.type && entry.capture === spec.capture && entry.kind === spec.kind && entry.callback === listener,
  );
  if (index < 0) return false;
  entries.splice(index, 1);
  if (entries.length === 0) ListenerRegistry.delete(target);
  return true;
}

export const ListenerOps: ListenerOpsSpec = {
  insertListener,
  deleteListener,
};
