import { useLocation } from "@tanstack/react-router";
import { Layers, ListChecks, ListPlus, X } from "lucide-react";
import { useEffect } from "react";
import { useMarkRead } from "../api/reading";
import { count } from "../format";
import { pickAll, pickedIds, pickNone, usePickedCount, usePickedOnly } from "../selection";
import { askToAddToCollection } from "./AddToCollection";
import { askToAddToList } from "./AddToList";
import { Button } from "./Button";
import { isOpenOverPage } from "./Menu";
import { iconButtonClass } from "./NavLink";
import { MARKS } from "./ReadMarks";

/** A button of the bar says what it does beside its icon only where there is room. */
const saidClass = "max-md:sr-only";

/**
 * What can be done to the cards that are picked, in a bar that stays in
 * sight at the bottom of the window while any are. "Select all" takes the
 * list the last card was picked in. Leaving the page, or
 * changing what it shows, lets go of them, and so does Escape.
 */
export function SelectionBar() {
  const picked = usePickedCount();
  // A collection holds whole series.
  const onlySeries = usePickedOnly("series");
  const mark = useMarkRead(pickNone);
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
      <div className="h-20" aria-hidden="true" />
      <section
        aria-label="What is selected"
        className="fixed inset-x-4 bottom-4 z-30 mx-auto flex w-fit max-w-full items-center gap-1 rounded-card border border-divider bg-surface p-2 shadow-xl"
      >
        <p className="px-3 font-semibold text-metadata" aria-live="polite">
          {count(picked, "selected", "selected")}
        </p>
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
        <Button onClick={() => askToAddToList(pickedIds(), pickNone)} title="Add to a reading list">
          <ListPlus className="size-5" aria-hidden="true" />
          <span className={saidClass}>Add to a list</span>
        </Button>
        {onlySeries ? (
          <Button
            onClick={() => askToAddToCollection(pickedIds().series, pickNone)}
            title="Add to a collection"
          >
            <Layers className="size-5" aria-hidden="true" />
            <span className={saidClass}>Add to a collection</span>
          </Button>
        ) : null}
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
