import { useLocation } from "@tanstack/react-router";
import { Layers, ListChecks, ListPlus, X } from "lucide-react";
import { useEffect } from "react";
import { useMarkRead } from "../api/reading";
import { useWant } from "../api/wanted";
import { count } from "../format";
import { pickAll, pickedIds, pickNone, usePickedCount, usePickedOnly } from "../selection";
import { askToAddToCollection } from "./AddToCollection";
import { askToAddToList } from "./AddToList";
import { Button } from "./Button";
import { isOpenOverPage } from "./Menu";
import { iconButtonClass } from "./NavLink";
import { MARKS } from "./ReadMarks";
import { WantToReadIcon } from "./WantToRead";

/**
 * A button of the bar says what it does beside its icon only where there is
 * room for all of them on one line: `withMore` is for the bar that has the
 * buttons for series too.
 */
const SAID = { few: "max-md:sr-only", withMore: "max-xl:sr-only" };

/**
 * What can be done to the cards that are picked, in a bar that stays in
 * sight at the bottom of the window while any are: one line, or two on a
 * phone. "Select all" takes the
 * list the last card was picked in. Leaving the page, or
 * changing what it shows, lets go of them, and so does Escape.
 */
export function SelectionBar() {
  const picked = usePickedCount();
  // A collection and want to read hold whole series.
  const onlySeries = usePickedOnly("series");
  const mark = useMarkRead(pickNone);
  const want = useWant(pickNone);
  const saidClass = onlySeries ? SAID.withMore : SAID.few;
  const page = useLocation({ select: (location) => location.href });
  const picking = picked > 0;

  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the page changes
  useEffect(() => pickNone, [page]);

  useEffect(() => {
    if (!picking) {
      return;
    }
    // Every card shows its tick while any is picked: see `picking` in the styles.
    document.documentElement.toggleAttribute("data-picking", true);
    function onKey(event: KeyboardEvent) {
      // A menu or a dialog that is open takes Escape for itself.
      if (event.key === "Escape" && !isOpenOverPage()) {
        pickNone();
      }
    }
    document.addEventListener("keydown", onKey);
    return () => {
      document.documentElement.toggleAttribute("data-picking", false);
      document.removeEventListener("keydown", onKey);
    };
  }, [picking]);

  if (!picking) {
    return null;
  }
  return (
    <>
      {/* Room under the page, so its last row can be brought clear of the bar. */}
      <div className="h-20 max-sm:h-36" aria-hidden="true" />
      <section
        aria-label="What is selected"
        className="fixed inset-x-4 bottom-4 z-30 mx-auto flex w-fit max-w-full items-center gap-1 rounded-card border border-divider bg-surface p-2 shadow-xl max-sm:w-auto max-sm:flex-wrap"
      >
        <p className="px-3 font-semibold text-metadata max-sm:flex-1" aria-live="polite">
          {count(picked, "selected", "selected")}
        </p>
        {/* A phone has no room for all of it in one line: there, what is done to them is a line of its own under the rest. */}
        <div className="flex items-center gap-1 max-sm:order-last max-sm:basis-full max-sm:[&>button]:min-w-0 max-sm:[&>button]:flex-1 max-sm:[&>button]:px-0">
          {MARKS.map(({ read, icon: Icon, one }) => (
            <Button
              key={one}
              disabled={mark.isPending}
              onClick={() => mark.mutate({ ...pickedIds(), read })}
              title={one}
            >
              <Icon className="size-5" aria-hidden="true" />
              <span className={saidClass}>{one}</span>
            </Button>
          ))}
          <Button
            onClick={() => askToAddToList(pickedIds(), pickNone)}
            title="Add to a reading list"
          >
            <ListPlus className="size-5" aria-hidden="true" />
            <span className={saidClass}>Add to a list</span>
          </Button>
          {onlySeries ? (
            <>
              <Button
                disabled={want.isPending}
                onClick={() => want.mutate(pickedIds().series)}
                title="Add to want to read"
              >
                <WantToReadIcon className="size-5" aria-hidden="true" />
                <span className={saidClass}>Want to read</span>
              </Button>
              <Button
                onClick={() => askToAddToCollection(pickedIds().series, pickNone)}
                title="Add to a collection"
              >
                <Layers className="size-5" aria-hidden="true" />
                <span className={saidClass}>Add to a collection</span>
              </Button>
            </>
          ) : null}
        </div>
        <Button variant="quiet" onClick={pickAll} title="Select everything in this list">
          <ListChecks className="size-5" aria-hidden="true" />
          <span className={saidClass}>Select all</span>
        </Button>
        <button
          type="button"
          onClick={pickNone}
          aria-label="Clear the selection"
          title="Clear the selection"
          className={`${iconButtonClass} size-11`}
        >
          <X className="size-5" aria-hidden="true" />
        </button>
      </section>
    </>
  );
}
