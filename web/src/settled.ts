import { useEffect, useState } from "react";

/** How long after the last key a search runs, in milliseconds. */
export const AFTER_TYPING = 250;

/** A value once it has stopped changing for a while. */
export function useSettled<Value>(value: Value, after: number): Value {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const wait = setTimeout(() => setSettled(value), after);
    return () => clearTimeout(wait);
  }, [value, after]);
  return settled;
}
