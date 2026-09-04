// Shared GC observation helpers for the `__test__/gc` suite.
//
// FinalizationRegistry is the primary signal: a fired callback proves the
// object was actually garbage collected, not merely unreferenced at one
// instant. WeakRef stays for "must still be alive" assertions; the wait
// loop never calls deref(), because a live deref() result extends the
// target's lifetime to the end of the current job.

const finalizedIds = new Set<string>();

const registry = new FinalizationRegistry<string>((id) => {
  finalizedIds.add(id);
});

let nextId = 0;

export function requireGc(): () => void {
  if (typeof globalThis.gc !== "function") {
    throw new Error("GC tests require Node.js --expose-gc");
  }
  return globalThis.gc;
}

/** Register `value` for finalization observation. Returns a diagnostic WeakRef. */
export function track<T extends object>(value: T): {id: string; weak: WeakRef<T>} {
  const id = `obj-${nextId += 1}`;
  registry.register(value, id);
  return {id, weak: new WeakRef(value)};
}

export function isFinalized(id: string): boolean {
  return finalizedIds.has(id);
}

/** Run GC until the tracked object's finalization callback fires. */
export async function waitForFinalization(id: string): Promise<boolean> {
  const gc = requireGc();
  for (let i = 0; i < 40; i += 1) {
    gc();
    await new Promise<void>((resolve) => setTimeout(resolve, 40));
    if (finalizedIds.has(id)) {
      return true;
    }
  }
  return false;
}
