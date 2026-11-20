import { useSyncExternalStore } from "react";

/**
 * A value kept outside the components, for something many of them show a
 * part of: a change draws again only the ones whose part changed.
 */
export function store<Value>(initial: Value) {
  let value = initial;
  const listeners = new Set<() => void>();
  function listen(listener: () => void) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }
  return {
    get: () => value,
    set(next: Value) {
      value = next;
      for (const listener of listeners) {
        listener();
      }
    },
    /** What a component shows of the value. `shown` must give the same answer for the same value. */
    useValue<Shown>(shown: (value: Value) => Shown): Shown {
      return useSyncExternalStore(listen, () => shown(value));
    },
  };
}
