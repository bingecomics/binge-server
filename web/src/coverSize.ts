/**
 * How wide a cover is in a grid at its narrowest, in rem: the least and the
 * most the slider gives, and its steps.
 *
 * It is one size for every grid and row of covers, kept in this browser.
 * A window too narrow for the slider does not follow it: see `--cover` in
 * `styles.css`, which also has the width until one is chosen.
 */
export const COVER_SIZE = { min: 6, max: 13, step: 0.5 } as const;

const KEY = "binge.cover-size";

/** The size chosen in this browser, if one was. */
function chosen(): number | undefined {
  try {
    const stored = Number(localStorage.getItem(KEY));
    return stored >= COVER_SIZE.min && stored <= COVER_SIZE.max ? stored : undefined;
  } catch {
    // Storage is blocked: the covers keep their own size.
    return undefined;
  }
}

function show(size: number) {
  document.documentElement.style.setProperty("--cover-chosen", `${size}rem`);
}

/** Gives the covers the size chosen in this browser. For before the first page is shown. */
export function showChosenCoverSize() {
  const size = chosen();
  if (size !== undefined) {
    show(size);
  }
}

/** The size the covers have: the one chosen, or else the one the styles start with. */
export function coverSize(): number {
  const root = document.documentElement;
  // What is shown is the truth, also when storage could not keep it.
  const shown =
    root.style.getPropertyValue("--cover-chosen") ||
    getComputedStyle(root).getPropertyValue("--cover-unchosen");
  return Number.parseFloat(shown);
}

/** Gives the covers another size and keeps it for next time. */
export function chooseCoverSize(size: number) {
  show(size);
  try {
    localStorage.setItem(KEY, String(size));
  } catch {
    // Not kept for next time; it still applies now.
  }
}
