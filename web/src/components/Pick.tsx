import { Check } from "lucide-react";
import { type MouseEvent, type PointerEvent, useEffect, useRef } from "react";
import { isPicking, keyOf, type Pickable, pick, useIsPicked } from "../selection";
import { cardCornerClass } from "./Cover";

/** How long a finger rests on a card before that picks it, in milliseconds. */
const LONG_PRESS = 500;
/** How far a finger may stray and still be resting, in pixels. */
const STRAY = 10;

/** The card something on it belongs to. */
function cardOf(part: Element): HTMLElement | null {
  return part.closest<HTMLElement>("[data-pick]");
}

/** Picks the card something on it belongs to. */
function pickCardOf(part: Element, upTo = false) {
  const card = cardOf(part);
  if (card) {
    pick(card, upTo);
  }
}

/**
 * What makes a card one that can be picked, to do one thing to several at
 * once. A card is picked by its tick, or on a touch screen by a finger
 * resting on it. Once something is picked a click on a card picks too, and
 * with Shift it picks every card from the one picked last.
 *
 * Returns what to put on the card (`card`), on the link around its cover
 * (`link`), and the tick to put after the link. `name` is what the card is
 * called. A card with nothing to name (no `what`) cannot be picked, and
 * gets nothing.
 */
export function usePick(what: Pickable | undefined, name: string) {
  const key = what ? keyOf(what) : "";
  const picked = useIsPicked(key);
  // A finger that is resting on the card: where it came down, and the wait.
  const resting = useRef<{ x: number; y: number; wait: ReturnType<typeof setTimeout> }>(null);
  // Set when the finger's rest picked the card: lifting it is not a click.
  const rested = useRef(false);

  function lift() {
    clearTimeout(resting.current?.wait);
    resting.current = null;
  }
  // A card that goes away is not picked by a finger that was resting on it.
  useEffect(() => () => clearTimeout(resting.current?.wait), []);

  if (!what) {
    return { card: {}, link: {}, tick: null };
  }
  return {
    card: { "data-pick": key, "data-picked": picked ? "" : undefined },
    link: {
      draggable: false,
      onClick(event: MouseEvent) {
        if (rested.current) {
          rested.current = false;
          event.preventDefault();
        } else if (isPicking()) {
          event.preventDefault();
          pickCardOf(event.currentTarget, event.shiftKey);
        }
      },
      onPointerDown(event: PointerEvent) {
        rested.current = false;
        if (event.pointerType === "mouse") {
          return;
        }
        const link = event.currentTarget;
        const wait = setTimeout(() => {
          rested.current = true;
          resting.current = null;
          pickCardOf(link);
        }, LONG_PRESS);
        resting.current = { x: event.clientX, y: event.clientY, wait };
      },
      onPointerMove(event: PointerEvent) {
        const from = resting.current;
        if (from && Math.hypot(event.clientX - from.x, event.clientY - from.y) > STRAY) {
          lift();
        }
      },
      onPointerUp: lift,
      onPointerCancel: lift,
      // A resting finger asks for the browser's own menu of the link.
      onContextMenu(event: MouseEvent) {
        if (rested.current || resting.current) {
          event.preventDefault();
        }
      },
    },
    tick: (
      // biome-ignore lint/a11y/useSemanticElements: a checkbox input cannot hold the drawn tick
      <button
        type="button"
        role="checkbox"
        aria-checked={picked}
        aria-label={`Select ${name}`}
        onClick={(event) => pickCardOf(event.currentTarget, event.shiftKey)}
        // Where there is a pointer to hover with, the tick shows under the
        // pointer. Once something is picked it shows on every card; until
        // then a touch screen has no tick to hit by mistake.
        className={`${cardCornerClass} left-1 picking:pointer-events-auto picking:opacity-100 [@media(hover:none)]:pointer-events-none`}
      >
        <span
          className={`grid size-6 place-items-center rounded-full shadow-md ring-1 ${
            picked ? "bg-accent text-on-accent ring-accent" : "bg-surface ring-outline"
          }`}
        >
          {picked ? <Check className="size-4" aria-hidden="true" /> : null}
        </span>
      </button>
    ),
  };
}
