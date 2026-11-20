import type { ReactNode } from "react";
import { coverGridClass, coverLineClass } from "./Cover";

export const shelfTitleClass = "font-semibold text-section-title";

/**
 * A titled grid of covers. `children` are its cards; `note` goes under them
 * and `action` at the end of the title's line.
 * `inOneLine` keeps the cards in one line that scrolls sideways, for a page
 * with several shelves.
 */
export function Shelf({
  title,
  children,
  note,
  action,
  inOneLine = false,
}: {
  title: string;
  children: ReactNode;
  note?: ReactNode;
  action?: ReactNode;
  inOneLine?: boolean;
}) {
  // The line brings its own space for a card's lift, over and under.
  const cards = inOneLine ? `${coverLineClass} mt-3 -mb-1` : `${coverGridClass} mt-4`;
  return (
    <section className="mb-12">
      <div className="flex items-baseline justify-between gap-4">
        <h2 className={shelfTitleClass}>{title}</h2>
        {action}
      </div>
      <ul className={cards}>{children}</ul>
      {note}
    </section>
  );
}
