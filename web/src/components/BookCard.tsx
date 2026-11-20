import { linkOptions } from "@tanstack/react-router";
import { EyeOff, Info, ListX } from "lucide-react";
import { memo } from "react";
import type { Book } from "../api/client";
import { useTakeOffUpNext } from "../api/reading";
import { called, issueOf, pageOf, pages, same, titleUnder, yearOf } from "../format";
import { AddToListChoice } from "./AddToList";
import { Card } from "./Card";
import { ReadLine } from "./Held";
import { MenuItem, MenuLink, MoreMenu } from "./Menu";
import { ReadMarks } from "./ReadMarks";

/**
 * How far a book was read, from 0 to 1, for the bar on its cover. Nothing
 * for one never opened, read to the end, or that cannot be read.
 */
export function doneOf(book: Book): number | undefined {
  const progress = book.progress;
  return progress && !progress.completed && !book.problem
    ? (progress.page + 1) / book.page_count
    : undefined;
}

/**
 * What there is to say about a book under its name, in one line: its
 * trouble, or how far it was read.
 */
export function Standing({ book }: { book: Book }) {
  if (book.problem) {
    return <p className="text-caption text-danger">{book.problem}</p>;
  }
  const progress = book.progress;
  if (progress?.completed) {
    return <ReadLine>Read</ReadLine>;
  }
  if (progress) {
    return <p className="text-caption text-fg-muted">{pageOf(progress.page, book)}</p>;
  }
  return <p className="text-caption text-fg-muted">{pages(book.page_count)}</p>;
}

/**
 * What a card says under a book's cover: its name, and a line that tells it
 * from its neighbours and never says the name again.
 *
 * `called` is what the book is called away from its series: see
 * [`called`]. On the page of its own series (`inSeries`) the series is
 * already said, so a book that goes by its number is only that: "#12".
 * Under a number is the book's title. Under a book that goes by its title
 * is its series, or the year where the series is already said or is the
 * same name. A file that could not be read has nothing under its name.
 */
function wording(
  book: Book,
  called: string,
  inSeries: boolean,
): { name: string; under: string | null } {
  if (book.problem) {
    return { name: called, under: null };
  }
  const number = issueOf(book);
  if (number) {
    return { name: inSeries ? number : called, under: titleUnder(book) ?? yearOf(book) };
  }
  const series = book.series_name;
  return {
    name: called,
    under: inSeries || !series || same(called, series) ? yearOf(book) : series,
  };
}

/**
 * The two choices that mark one book: as read while it is not read to the
 * end, as unread once it was opened.
 */
export function BookReadMarks({ book }: { book: Book }) {
  return (
    <ReadMarks
      what={{ books: [book.id] }}
      canRead={!book.progress?.completed}
      canUnread={Boolean(book.progress)}
    />
  );
}

/**
 * The choice that opens a book for a look: its pages are turned without
 * moving how far it was read. `listId` and `inGroup` say what the reader
 * follows from it, as for a reading that is recorded.
 */
export function ReadUnrecorded({
  book,
  listId,
  inGroup = false,
}: {
  book: Book;
  listId?: number;
  inGroup?: boolean;
}) {
  return (
    <MenuLink
      to="/read/$bookId"
      params={{ bookId: book.id }}
      search={{ list: listId, group: inGroup || undefined, unrecorded: true }}
      icon={EyeOff}
    >
      Read without recording
    </MenuLink>
  );
}

/** The choice that takes a book's series off "Up next", for a card in that row. */
function TakeOffUpNext({ seriesId }: { seriesId: number }) {
  const takeOff = useTakeOffUpNext();
  return (
    <MenuItem icon={ListX} onSelect={() => takeOff.mutate(seriesId)}>
      Remove from this row
    </MenuItem>
  );
}

/**
 * A book as a card. It opens the reader, unless its file could not be read.
 * The menu on its cover leads to its details and marks it, and its tick
 * picks it. `upNext` is for a card in the row of that name, which can be
 * taken off it.
 *
 * Its name is a heading one level under the one above the cards, which every
 * page that lists books has. `inSeries` is for the page of the book's own
 * series: see [`wording`]. `inGroup` is for the page of its group of that
 * series: read from there, the reader stops at the end of the group.
 */
// Memoized: the pages that list books change every second while a scan runs.
export const BookCard = memo(function BookCard({
  book,
  inSeries = false,
  inGroup = false,
  upNext = false,
}: {
  book: Book;
  inSeries?: boolean;
  inGroup?: boolean;
  upNext?: boolean;
}) {
  const unreadable = Boolean(book.problem);
  const about = called(book);
  const read = linkOptions({
    to: "/read/$bookId",
    params: { bookId: book.id },
    search: { group: inGroup || undefined },
  });
  return (
    <Card
      what={unreadable ? undefined : { kind: "books", id: book.id }}
      about={about}
      link={unreadable ? undefined : read}
      cover={book.cover}
      unreadable={unreadable}
      done={doneOf(book)}
      {...wording(book, about, inSeries)}
      standing={<Standing book={book} />}
      menu={
        <MoreMenu corner about={about}>
          <MenuLink to="/books/$bookId" params={{ bookId: book.id }} icon={Info}>
            Details
          </MenuLink>
          {unreadable ? null : <ReadUnrecorded book={book} inGroup={inGroup} />}
          {unreadable ? null : <BookReadMarks book={book} />}
          {unreadable ? null : <AddToListChoice what={{ books: [book.id] }} />}
          {upNext && book.series_id != null ? <TakeOffUpNext seriesId={book.series_id} /> : null}
        </MoreMenu>
      }
    />
  );
});
