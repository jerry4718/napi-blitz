export function dispatchEvent(target: EventTarget, event: Event): boolean | undefined {
  try {
    return target.dispatchEvent(event);
  } catch (e) {
    console.error(e)
  }
}
