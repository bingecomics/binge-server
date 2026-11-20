import { Link } from "@tanstack/react-router";
import { BookOpen, CircleCheck } from "lucide-react";
import { type ReactNode, useState } from "react";
import {
  type Book,
  type BooksOverview,
  type Credit as Credited,
  coverUrl,
  type ReadSoFar,
} from "../api/client";
import {
  about,
  books as bookCount,
  called,
  issueOf,
  nameOf,
  pageOf,
  pages,
  readingTime,
  titleUnder,
} from "../format";
import { buttonClass } from "./Button";
import { Cover } from "./Cover";
import { Fact, Facts } from "./Facts";
import { NarrowedLink, PeopleLinks } from "./NarrowedLink";
import { ProgressBar } from "./ProgressBar";
import { textLinkClass } from "./text";

/** The cover, blurred into a wash of its colours that fades out down the top of the page. */
function Backdrop({ cover }: { cover: string }) {
  return (
    // Reaches the edges of the page through the page's own padding.
    <div
      aria-hidden="true"
      className="-inset-x-4 -top-8 md:-inset-x-12 md:-top-12 mask-b-from-0% pointer-events-none absolute bottom-0 -z-10 overflow-hidden"
    >
      <img
        src={coverUrl(cover)}
        alt=""
        className="size-full scale-125 object-cover opacity-25 blur-3xl saturate-150 dark:opacity-40"
      />
    </div>
  );
}

/** Whether the reader has read every one of the books that can be read. */
function isFinished(read: ReadSoFar): boolean {
  return read.readable_count > 0 && read.books_read === read.readable_count;
}

/**
 * The button that goes on with a series, a group or a reading list, the
 * menu of what else can be done with it, and what the button opens. In a
 * series a book goes by its number, on the button itself. A reading list
 * (`listId`) holds books of several series, so there the book is named
 * whole beside the button, and reading it follows the list. Read from a
 * group of a series (`inGroup`), it stops at the end of the group.
 */
function NextUp({
  book,
  again,
  menu,
  listId,
  inGroup,
}: {
  book: Book;
  again: boolean;
  menu: ReactNode;
  listId?: number;
  inGroup: boolean;
}) {
  const progress = book.progress;
  const started = progress && !progress.completed;
  const inList = listId !== undefined;
  const number = inList ? null : issueOf(book);
  const verb = again ? "Read again" : started ? "Continue" : "Read";
  const about = [
    inList ? called(book) : null,
    number || inList ? titleUnder(book) : nameOf(book),
    started ? pageOf(progress.page, book) : pages(book.page_count),
  ];
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <Link
        to="/read/$bookId"
        params={{ bookId: book.id }}
        // Asked to read it again, the book opens at its start with no question.
        search={{ list: listId, group: inGroup || undefined, again: again || undefined }}
        className={buttonClass("primary")}
      >
        <BookOpen className="size-5" aria-hidden="true" />
        {number && !again ? `${verb} ${number}` : verb}
      </Link>
      {menu}
      <p className="min-w-0 truncate text-fg-muted text-metadata">
        {about.filter(Boolean).join(" · ")}
      </p>
    </div>
  );
}

/** How far the reader is through the books. Nothing when they have not started. */
function Reading({ read }: { read: ReadSoFar }) {
  if (read.pages_read === 0) {
    return null;
  }
  if (isFinished(read)) {
    return (
      <p className="mt-4 flex items-center gap-1.5 text-fg-muted text-metadata">
        <CircleCheck className="size-4 text-success" aria-hidden="true" />
        You have read all of it
      </p>
    );
  }
  // At the pace the reader reads at, once they have read enough to tell.
  const measured = read.seconds_left != null;
  const left =
    read.seconds_left != null
      ? about(read.seconds_left)
      : readingTime(read.page_count - read.pages_read);
  return (
    <div className="mt-4 max-w-md">
      <ProgressBar done={read.pages_read / read.page_count} className="h-1.5 rounded-full" />
      <p className="mt-1.5 text-fg-muted text-metadata">
        {read.books_read.toLocaleString()} of {bookCount(read.readable_count)} read
        {left ? ` · ${left} left${measured ? " at your pace" : ""}` : null}
      </p>
    </div>
  );
}

/** A summary longer than this is cut short until asked for. */
const LONG_SUMMARY = 320;

/**
 * What the books are about, with the book whose words these are. With no
 * book, they are the words of what holds the books.
 */
function Summary({ text, from }: { text: string; from?: Book | null }) {
  const [open, setOpen] = useState(false);
  const long = text.length > LONG_SUMMARY;
  return (
    <div className="mt-6 max-w-prose">
      <p className={`whitespace-pre-line ${long && !open ? "line-clamp-4" : ""}`}>{text}</p>
      <p className="mt-1.5 flex gap-3 text-caption text-fg-muted empty:hidden">
        {from ? (
          <span>
            From{" "}
            <Link to="/books/$bookId" params={{ bookId: from.id }} className={textLinkClass}>
              {issueOf(from) ?? nameOf(from)}
            </Link>
          </span>
        ) : null}
        {long ? (
          <button
            type="button"
            onClick={() => setOpen(!open)}
            aria-expanded={open}
            className={textLinkClass}
          >
            {open ? "Show less" : "Show more"}
          </button>
        ) : null}
      </p>
    </div>
  );
}

/** How many names a credit shows before the rest are asked for. */
const NAMES_SHOWN = 3;

/**
 * Who did one job on the books: the first few people, and the rest on
 * request. In a library each leads to its series by them. Where the books
 * are of several libraries, as in a reading list, each leads to their own
 * page.
 */
function Credit({
  job,
  by,
  people,
  libraryId,
}: {
  job: [one: string, many: string];
  by: "writer" | "penciller";
  people: Credited[];
  libraryId: number | undefined;
}) {
  const [all, setAll] = useState(false);
  if (people.length === 0) {
    return null;
  }
  const shown = all ? people : people.slice(0, NAMES_SHOWN);
  const more = people.length - shown.length;
  return (
    <Fact label={people.length === 1 ? job[0] : job[1]}>
      <PeopleLinks libraryId={libraryId} by={by} people={shown} />
      {more > 0 ? (
        <>
          {" "}
          <button type="button" onClick={() => setAll(true)} className={textLinkClass}>
            and {more.toLocaleString()} more
          </button>
        </>
      ) : null}
    </Fact>
  );
}

/**
 * The top of the page of a series, of a group of one or of a reading list:
 * its cover over a wash of its colours, its name, what its books say about
 * it together, how far the reader is through it and the way to go on.
 */
export function BooksHeader({
  cover,
  covers,
  title,
  bookCount: held,
  overview,
  libraryId,
  listId,
  inGroup = false,
  menu,
  up,
}: {
  cover: string | null | undefined;
  /** The covers a reading list shows together as one: see `Cover`. */
  covers?: string[];
  title: string;
  bookCount: number;
  overview: BooksOverview;
  /**
   * The library the books are all in. What they say then leads to that
   * library narrowed by it. The books of a reading list are in several.
   */
  libraryId?: number;
  /** The reading list these books are, which reading then follows. */
  listId?: number;
  /** The books are one group of a series: reading stops at the end of it. */
  inGroup?: boolean;
  /** The menu of what can be done with the books, beside the way to go on. */
  menu: ReactNode;
  /** The way back up to what holds the page, above everything else. */
  up: ReactNode;
}) {
  const { read, summary, writers, pencillers } = overview;
  const { publisher, first_year: first, last_year: last } = overview;
  const year = (year: number) => (
    <NarrowedLink libraryId={libraryId} by="year" value={String(year)}>
      {year}
    </NarrowedLink>
  );
  const time = readingTime(read.page_count);
  return (
    <header className="relative isolate pb-10">
      {cover ? <Backdrop cover={cover} /> : null}
      {up}
      {/* Beside a tall cover, the name keeps its own height and the rest takes what is left. */}
      <div className="mt-2 grid grid-cols-[6.5rem_minmax(0,1fr)] items-start gap-x-4 gap-y-6 sm:grid-cols-[13rem_minmax(0,1fr)] sm:grid-rows-[auto_1fr] sm:gap-x-8">
        <div className="shadow-lg sm:row-span-2">
          <Cover cover={cover} covers={covers} />
        </div>

        <div className="self-center sm:self-start">
          <h1 className="font-bold text-screen-title leading-tight tracking-tight">{title}</h1>
          {/* What there is to say in a few words each, a dot between them. What is missing is left out. */}
          <p className="mt-2 text-fg-muted text-metadata [&>span+span]:before:mx-1.5 [&>span+span]:before:content-['·']">
            {publisher ? (
              <span>
                <NarrowedLink libraryId={libraryId} by="publisher" value={publisher}>
                  {publisher}
                </NarrowedLink>
              </span>
            ) : null}
            {/* The years the books ran: "1968", or "1968–1988". */}
            {first && last ? (
              <span>
                {year(first)}
                {first === last ? null : <>–{year(last)}</>}
              </span>
            ) : null}
            <span>{bookCount(held)}</span>
            {read.page_count ? <span>{pages(read.page_count)}</span> : null}
            {time ? <span>{time}</span> : null}
          </p>
        </div>

        <div className="col-span-2 sm:col-span-1 sm:col-start-2">
          {read.next ? (
            <NextUp
              book={read.next}
              again={isFinished(read)}
              menu={menu}
              listId={listId}
              inGroup={inGroup}
            />
          ) : null}
          <Reading read={read} />
          {summary ? <Summary text={summary.text} from={summary.book} /> : null}
          {writers.length > 0 || pencillers.length > 0 ? (
            <Facts className="mt-6">
              <Credit
                job={["Writer", "Writers"]}
                by="writer"
                people={writers}
                libraryId={libraryId}
              />
              <Credit
                job={["Penciller", "Pencillers"]}
                by="penciller"
                people={pencillers}
                libraryId={libraryId}
              />
            </Facts>
          ) : null}
        </div>
      </div>
    </header>
  );
}
