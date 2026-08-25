export function dispatchEvent(target: EventTarget, event: Event): boolean | undefined {
  try {
    return target.dispatchEvent(event);
  } catch (e) {
    console.error(e);
  }
}

declare global {
  export interface EventInit {
    bubbles?: boolean;
    cancelable?: boolean;
    composed?: boolean;
  }
}

interface EventListener {
  (evt: Event): void;
}

interface EventListenerObject {
  handleEvent(object: Event): void;
}

interface TypedEventListener<M, K extends keyof M> {
  (this: TypedEventTarget<M>, ev: M[K]): any;
}

interface TypedEventListenerObject<M, K extends keyof M> {
  handleEvent(this: TypedEventTarget<M>, ev: M[K]): any;
}

/** `addEventListener` options: `EventListenerOptions` plus `once`/`passive`/`signal`. */
export type AddEventListenerOptions =
  & EventListenerOptions
  & {
  once?: boolean;
  passive?: boolean;
  signal?: AbortSignal
};

/**
 * Event-target typed by an event map: `addEventListener`/`removeEventListener`
 * narrow each event name declared in `M` to its event class. At runtime this
 * is the `EventTarget` itself — no methods are overridden; the shape is
 * declared entirely by the factory's signature.
 */
export interface TypedEventTarget<M> {
  addEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListener<M, K>,
    options?: AddEventListenerOptions | boolean,
  ): void;

  addEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListenerObject<M, K>,
    options?: AddEventListenerOptions | boolean,
  ): void;

  addEventListener(
    type: string,
    listener: EventListener | EventListenerObject,
    options?: AddEventListenerOptions | boolean,
  ): void;

  removeEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListener<M, K>,
    options?: EventListenerOptions | boolean,
  ): void;

  removeEventListener<K extends keyof M>(
    type: K,
    listener: TypedEventListenerObject<M, K>,
    options?: EventListenerOptions | boolean,
  ): void;

  removeEventListener(
    type: string,
    listener: EventListener | EventListenerObject,
    options?: EventListenerOptions | boolean,
  ): void;

  dispatchEvent(event: M[keyof M] | Event): boolean;
}

/** Constructor shape of a typed `EventTarget`; usable directly as an `extends` base. */
export interface TypedEventTargetConstructor<M> {
  new(...args: any[]): TypedEventTarget<M>;
}

/** Merge two event maps: keys are the union, values dispatch per key (the second map wins). */
export type ExtendEventMap<A, B> = {
  [K in keyof A | keyof B]: K extends keyof B ? B[K] : K extends keyof A ? A[K] : never;
};

/** Extract the complete event map carried by a parent constructor. */
type ParentEventMap<C> = C extends abstract new (...args: any[]) => TypedEventTarget<infer P> ? P : never;

/** Derive a typed target from a root map or a parent class plus a delta. */
export function TypedEventTarget<M>(Base: typeof EventTarget): TypedEventTargetConstructor<M>;
export function TypedEventTarget<M, C extends abstract new (...args: any[]) => any>(
  Base: C,
): TypedEventTargetConstructor<ExtendEventMap<ParentEventMap<C>, M>> & C;
export function TypedEventTarget<M>(Base: any = EventTarget): TypedEventTargetConstructor<M> {
  return Base as unknown as TypedEventTargetConstructor<M>;
}
