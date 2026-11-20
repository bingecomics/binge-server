import type { BooksToMark } from "./api/reading";
import { store } from "./store";

/**
 * What can be picked on a page, to do one thing to several at once. The
 * kinds are named as the server names them in a request.
 */
export type Pickable = { kind: keyof BooksToMark; id: number };

/** How a card names what it is, in its `data-pick`. */
export function keyOf(what: Pickable): string {
  return `${what.kind}:${what.id}`;
}

/**
 * What is picked on the page being shown. Kept outside the components, so
 * that picking one card draws that card again and not the whole page.
 */
const picked = store<ReadonlySet<string>>(new Set());
/**
 * The card picked last, and the list it is in: a pick "up to" another card
 * of the list starts from it, and picking all takes that list. A page can
 * hold several lists, of different kinds and with the same book in two.
 */
let last: { key: string; list: ParentNode } | undefined;

/** Whether this is picked. */
export function useIsPicked(key: string): boolean {
  return picked.useValue((keys) => keys.has(key));
}

/** How many are picked. */
export function usePickedCount(): number {
  return picked.useValue((keys) => keys.size);
}

/** Whether something is picked and all of it is of one kind. */
export function usePickedOnly(kind: Pickable["kind"]): boolean {
  return picked.useValue(
    (keys) => keys.size > 0 && [...keys].every((key) => key.startsWith(`${kind}:`)),
  );
}

/** Whether anything is picked. Once something is, a click on a cover picks. */
export function isPicking(): boolean {
  return picked.get().size > 0;
}

/** The cards of a list that can be picked, in the order they show. */
function cardsIn(list: ParentNode): string[] {
  const cards = list.querySelectorAll<HTMLElement>("[data-pick]");
  return Array.from(cards, (card) => card.dataset.pick ?? "");
}

/**
 * Picks a card, or lets go of one that was picked. `upTo` picks every card
 * of its list from the one picked last up to this one, as a click with Shift
 * does in a list.
 */
export function pick(card: HTMLElement, upTo = false) {
  const key = card.dataset.pick;
  const list = card.parentElement;
  if (!key || !list) {
    return;
  }
  const next = new Set(picked.get());
  const cards = upTo && last?.list === list ? cardsIn(list) : [];
  const from = last ? cards.indexOf(last.key) : -1;
  const to = cards.indexOf(key);
  if (from >= 0 && to >= 0) {
    for (const between of cards.slice(Math.min(from, to), Math.max(from, to) + 1)) {
      next.add(between);
    }
  } else if (!next.delete(key)) {
    next.add(key);
  }
  last = { key, list };
  picked.set(next);
}

/** Picks every card of the list the last card was picked in. */
export function pickAll() {
  const list = last?.list.isConnected ? last.list : document;
  picked.set(new Set([...picked.get(), ...cardsIn(list)]));
}

/** Lets go of everything. */
export function pickNone() {
  last = undefined;
  if (picked.get().size > 0) {
    picked.set(new Set());
  }
}

/** What is picked, as the server takes it: the ids of each kind. */
export function pickedIds(): Required<BooksToMark> {
  const ids: Required<BooksToMark> = { books: [], groups: [], series: [], lists: [] };
  for (const key of picked.get()) {
    const [kind, id] = key.split(":");
    ids[kind as Pickable["kind"]].push(Number(id));
  }
  return ids;
}
