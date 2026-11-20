import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";
import type { Book } from "../api/client";
import { BookCard } from "./BookCard";
import { buttonClass } from "./Button";
import { coverGridClass } from "./Cover";
import { shelfTitleClass } from "./Shelf";

/** Whether a book is still to be read: not one read to the end, and not a file that cannot be read. */
function isUnread(book: Book): boolean {
  return Boolean(book.identity) && !book.progress?.completed;
}

/**
 * What makes a link a choice of what a page shows: it keeps the page where
 * it is and is not a page to go back to. The router marks a link as the
 * current page when it leads here; only one that asks for exactly what is
 * showing does.
 */
export const choiceProps = {
  replace: true,
  resetScroll: false,
  activeOptions: { exact: true },
} as const;

/**
 * What a page with a grid of books keeps in its address about the grid, so
 * that coming back from a book finds the same books.
 */
export type Shown = {
  /** Leaves out the books already read. */
  show?: "unread";
};

/** Reads `Shown` from what an address asks for. */
export function shownIn(search: Record<string, unknown>): Shown {
  return { show: search.show === "unread" ? "unread" : undefined };
}

/** One of the two choices of which books to show. It keeps whatever else the address asks for. */
function Choice({ show, chosen, children }: Shown & { chosen: boolean; children: ReactNode }) {
  return (
    <Link
      to="."
      search={(asked) => ({ ...asked, show })}
      {...choiceProps}
      aria-current={chosen ? "true" : undefined}
      className={`${buttonClass()} aria-[current]:bg-fg aria-[current]:text-background`}
    >
      {children}
    </Link>
  );
}

/**
 * Books of one series in a grid, each by its number or else its name, under a title and the
 * choice between all of them and the unread ones. The page it is on keeps
 * that choice in its address: see [`Shown`].
 */
export function BookGrid({
  title,
  titleHidden = false,
  books,
  unread,
  inGroup = false,
}: {
  title: string;
  /** For a grid whose title says nothing the page has not said: it is only read out. */
  titleHidden?: boolean;
  /** In the order they are read. */
  books: Book[];
  /** Leaves out the books already read. */
  unread: boolean;
  /** The books are one group of their series: reading one stops at the end of the group. */
  inGroup?: boolean;
}) {
  const shown = unread ? books.filter(isUnread) : books;
  // Nothing to leave out until a book has been read.
  const withChoices = unread || books.some((book) => book.progress?.completed);
  // With its title hidden and no choice to offer, the row takes no room.
  const rowGap = titleHidden && !withChoices ? "" : "mb-4";

  return (
    <section aria-labelledby="books-title">
      <div className={`flex flex-wrap items-center justify-between gap-x-6 gap-y-3 ${rowGap}`}>
        <h2 id="books-title" className={titleHidden ? "sr-only" : shelfTitleClass}>
          {title}
        </h2>
        {withChoices ? (
          // biome-ignore lint/a11y/useSemanticElements: a fieldset is for form fields, and these are links
          <div role="group" aria-label="Which books to show" className="ml-auto flex gap-2">
            <Choice chosen={!unread}>All</Choice>
            <Choice show="unread" chosen={unread}>
              Unread
            </Choice>
          </div>
        ) : null}
      </div>

      {shown.length > 0 ? (
        <ul className={coverGridClass}>
          {shown.map((book) => (
            <BookCard key={book.id} book={book} inSeries inGroup={inGroup} />
          ))}
        </ul>
      ) : (
        <p className="text-fg-muted text-metadata">Nothing left to read here.</p>
      )}
    </section>
  );
}
