import { store } from "./store";

/**
 * What is said over the page for a while, one sentence at a time, about
 * something done from a menu or a bar, which has no place of its own to say
 * it: what went wrong, or that it was done when nothing on the page shows
 * it. See `components/Problem.tsx`.
 */
export type Said = { sentence: string; wrong: boolean };

const said = store<Said | null>(null);
let gone: ReturnType<typeof setTimeout> | undefined;

/** How long a sentence stays up, in milliseconds. */
const SHOWN_FOR = 8000;

function say(sentence: string, wrong: boolean) {
  clearTimeout(gone);
  gone = setTimeout(dismissProblem, SHOWN_FOR);
  said.set({ sentence, wrong });
}

/** Says what went wrong, in a sentence. It takes the place of the one before. */
export function report(sentence: string) {
  say(sentence, true);
}

/** Says that something was done, where nothing on the page shows it. */
export function tell(sentence: string) {
  say(sentence, false);
}

export function dismissProblem() {
  clearTimeout(gone);
  said.set(null);
}

/** What to say over the page, if anything. */
export function useProblem(): Said | null {
  return said.useValue((now) => now);
}
