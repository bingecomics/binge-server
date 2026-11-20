import { Link, linkOptions } from "@tanstack/react-router";
import { ArrowUpDown, GripVertical, Info, ListX } from "lucide-react";
import { type DragEvent, memo, useState } from "react";
import { useIsOwner } from "../api/account";
import type { Book } from "../api/client";
import { useMoveInList, useRemoveFromList } from "../api/lists";
import { called, published, titleUnder, yearOf } from "../format";
import { BookReadMarks, doneOf, ReadUnrecorded, Standing } from "./BookCard";
import { Button } from "./Button";
import { Cover } from "./Cover";
import { Dialog, DialogActions } from "./Dialog";
import { Field } from "./Field";
import { Form } from "./Form";
import { MenuItem, MenuLink, MoreMenu } from "./Menu";
import { navHeadingClass } from "./NavLink";

/**
 * The columns a wide window gives each book, after its name. A narrower
 * one, where they would be cut to a few letters each, says them under the
 * name.
 */
const wideOnly = "max-xl:hidden";
/** Their widths, which the line of headings over the rows has too. */
const creditWidth = "min-w-0 flex-1";
const dateWidth = "w-36 shrink-0";
const standingWidth = "w-32 shrink-0";
const creditClass = `${creditWidth} truncate text-fg-muted text-metadata ${wideOnly}`;
const dateClass = `${dateWidth} text-fg-muted text-metadata ${wideOnly}`;
const standingClass = `${standingWidth} ${wideOnly}`;
/** The grip a row is dragged by, where there is a pointer to drag with. */
const gripClass = "size-4 shrink-0 [@media(hover:none)]:hidden";
export const placeClass = "w-7 shrink-0 text-right text-fg-muted text-metadata tabular-nums";

/** When a book came out, as exactly as is known. */
function dateOf(book: Book): string | null {
  return (book.info ? published(book.info) : null) ?? yearOf(book);
}

/**
 * One book of a reading list, across a row: its place, its cover, what it
 * is called, who made it, when it came out and how far it was read. It
 * opens the reader, which then follows the list.
 */
const Row = memo(function Row({
  book,
  place,
  listId,
  owner,
  withWriter,
  withPenciller,
  withDate,
  onMove,
  onRemove,
}: {
  book: Book;
  /** Its place in the list, from 1. */
  place: number;
  listId: number;
  owner: boolean;
  /** The columns the list has: one that no book of it fills is left out. */
  withWriter: boolean;
  withPenciller: boolean;
  withDate: boolean;
  /** Both take the book's id. */
  onMove: (bookId: number) => void;
  onRemove: (bookId: number) => void;
}) {
  const name = called(book);
  const under = titleUnder(book);
  const { writer, penciller } = book.info ?? {};
  const date = dateOf(book);
  const read = linkOptions({
    to: "/read/$bookId",
    params: { bookId: book.id },
    search: { list: listId },
  });
  return (
    <>
      {owner ? <GripVertical className={`${gripClass} text-fg-muted`} aria-hidden="true" /> : null}
      <span className={placeClass}>
        <span className="sr-only">Place </span>
        {place.toLocaleString()}
      </span>
      {/* The name beside it leads to the same place. */}
      <Link {...read} draggable={false} tabIndex={-1} aria-hidden="true" className="w-11 shrink-0">
        <Cover cover={book.cover} done={doneOf(book)} />
      </Link>
      <div className="min-w-0 flex-1 xl:flex-[2]">
        <h3 className="truncate font-semibold text-metadata" title={name}>
          <Link {...read} draggable={false} className="rounded-control hover:text-accent">
            {name}
          </Link>
        </h3>
        {under ? (
          <p className="truncate text-caption text-fg-muted" title={under}>
            {under}
          </p>
        ) : null}
        <p className="truncate text-caption text-fg-muted empty:hidden xl:hidden">
          {/* The date first: it is the short one, and the end of the line is cut. */}
          {/* A person who did both is said once. */}
          {[date, ...new Set([writer, penciller])].filter(Boolean).join(" · ")}
        </p>
        <div className="xl:hidden">
          <Standing book={book} />
        </div>
      </div>
      {withWriter ? (
        <p className={creditClass} title={writer ?? undefined}>
          {writer ? <span className="sr-only">Writer: </span> : null}
          {writer}
        </p>
      ) : null}
      {withPenciller ? (
        <p className={creditClass} title={penciller ?? undefined}>
          {penciller ? <span className="sr-only">Penciller: </span> : null}
          {penciller}
        </p>
      ) : null}
      {withDate ? <p className={dateClass}>{date}</p> : null}
      <div className={standingClass}>
        <Standing book={book} />
      </div>
      <MoreMenu quiet about={name}>
        <MenuLink to="/books/$bookId" params={{ bookId: book.id }} icon={Info}>
          Details
        </MenuLink>
        <ReadUnrecorded book={book} listId={listId} />
        <BookReadMarks book={book} />
        {owner ? (
          <>
            <MenuItem icon={ArrowUpDown} onSelect={() => onMove(book.id)}>
              Move to a place…
            </MenuItem>
            <MenuItem icon={ListX} onSelect={() => onRemove(book.id)}>
              Remove from the list
            </MenuItem>
          </>
        ) : null}
      </MoreMenu>
    </>
  );
});

/** Asks which place of the list to move a book to. */
function MoveTo({
  book,
  place,
  last,
  onMove,
  onClose,
}: {
  book: Book;
  /** Where it is now, from 1. */
  place: number;
  /** The last place of the list. */
  last: number;
  onMove: (place: number) => void;
  onClose: () => void;
}) {
  const [typed, setTyped] = useState(String(place));
  const to = Number(typed);
  const valid = Number.isInteger(to) && to >= 1 && to <= last;
  return (
    <Form
      className="mt-4"
      onSubmit={() => {
        if (valid) {
          onMove(to);
          onClose();
        }
      }}
      actions={
        <DialogActions onCancel={onClose}>
          <Button type="submit" variant="primary" disabled={!valid || to === place}>
            Move
          </Button>
        </DialogActions>
      }
    >
      <Field
        label={`Place of ${called(book)}`}
        hint={`From 1 to ${last.toLocaleString()}. It is at ${place.toLocaleString()} now.`}
        type="number"
        inputMode="numeric"
        min={1}
        max={last}
        value={typed}
        onChange={(event) => setTyped(event.target.value)}
        // What is typed takes the place of where the book is now.
        onFocus={(event) => event.target.select()}
        autoFocus
      />
    </Form>
  );
}

/** The row a part of the list belongs to, as its place from 0. */
function rowOf(event: DragEvent): number | null {
  const row = (event.target as Element).closest<HTMLElement>("[data-row]");
  return row ? Number(row.dataset.row) : null;
}

/**
 * The books of a reading list in its order, one to a row. The owner
 * changes the order by dragging a row onto the place it is to take, or
 * from a row's menu by typing the place, which is the way on a touch
 * screen and with the keyboard.
 */
export function ListBooks({ listId, books }: { listId: number; books: Book[] }) {
  const owner = useIsOwner();
  const move = useMoveInList(listId);
  const remove = useRemoveFromList(listId);
  /** The rows being dragged and dragged over, as places from 0. */
  const [dragged, setDragged] = useState<number | null>(null);
  const [over, setOver] = useState<number | null>(null);
  /** The book whose place is being typed, by its id, and where it is now. */
  const [moving, setMoving] = useState<number | null>(null);
  const movingFrom = books.findIndex((book) => book.id === moving);
  const movingBook = books[movingFrom];
  // Files say what they know: many name no writer, no penciller or no date.
  const withWriter = books.some((book) => book.info?.writer);
  const withPenciller = books.some((book) => book.info?.penciller);
  const withDate = books.some((book) => dateOf(book));

  function drop() {
    setDragged(null);
    setOver(null);
  }

  return (
    <section aria-labelledby="list-books">
      <h2 id="list-books" className="sr-only">
        Books
      </h2>
      <div
        aria-hidden="true"
        className={`flex items-center gap-3 pb-2 ${navHeadingClass} ${wideOnly}`}
      >
        {owner ? <span className={gripClass} /> : null}
        <span className={placeClass}>#</span>
        <span className="w-11 shrink-0" />
        <span className="min-w-0 flex-[2]">Book</span>
        {withWriter ? <span className={`${creditWidth} truncate`}>Writer</span> : null}
        {withPenciller ? <span className={`${creditWidth} truncate`}>Penciller</span> : null}
        {withDate ? <span className={dateWidth}>Published</span> : null}
        <span className={standingWidth}>Progress</span>
        <span className="w-11 shrink-0" />
      </div>
      <ol
        className="border-divider border-b"
        onDragStart={(event) => {
          const row = rowOf(event);
          const book = row === null ? undefined : books[row];
          if (!book) {
            return;
          }
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("text/plain", called(book));
          setDragged(row);
        }}
        onDragOver={(event) => {
          if (dragged === null) {
            return;
          }
          // Lets the row be dropped here.
          event.preventDefault();
          setOver(rowOf(event));
        }}
        onDrop={(event) => {
          event.preventDefault();
          const row = rowOf(event);
          const book = dragged === null ? undefined : books[dragged];
          if (book && row !== null && row !== dragged) {
            move.mutate({ bookId: book.id, position: row });
          }
          drop();
        }}
        onDragEnd={drop}
      >
        {books.map((book, row) => {
          // The line on the side of the row the dragged one will come to rest.
          const mark =
            dragged === null || over !== row || dragged === row
              ? ""
              : dragged < row
                ? "shadow-[inset_0_-2px_0_var(--color-accent)]"
                : "shadow-[inset_0_2px_0_var(--color-accent)]";
          return (
            <li
              key={book.id}
              data-row={row}
              draggable={owner}
              className={`flex items-center gap-3 border-divider border-t py-2 ${mark} ${
                dragged === row ? "opacity-50" : ""
              } ${owner ? "[@media(hover:hover)]:cursor-grab" : ""}`}
            >
              <Row
                book={book}
                place={row + 1}
                listId={listId}
                owner={owner}
                withWriter={withWriter}
                withPenciller={withPenciller}
                withDate={withDate}
                onMove={setMoving}
                onRemove={remove.mutate}
              />
            </li>
          );
        })}
      </ol>
      <Dialog
        open={movingBook !== undefined}
        onClose={() => setMoving(null)}
        title="Move to a place"
      >
        {movingBook ? (
          <MoveTo
            book={movingBook}
            place={movingFrom + 1}
            last={books.length}
            onMove={(place) => move.mutate({ bookId: movingBook.id, position: place - 1 })}
            onClose={() => setMoving(null)}
          />
        ) : null}
      </Dialog>
    </section>
  );
}
