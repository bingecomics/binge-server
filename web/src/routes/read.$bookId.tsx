import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { BookOpenCheck, FileWarning, History } from "lucide-react";
import { type ReactNode, useCallback, useState } from "react";
import { type Book, gotNoAnswer } from "../api/client";
import { useBook, usePages } from "../api/reading";
import { useSettings } from "../api/settings";
import { Button } from "../components/Button";
import { BookNotOpened, Notice, ServerNotAnswering } from "../components/Notice";
import { textLinkClass } from "../components/text";
import { ago, called, day } from "../format";
import { useFollowing } from "../reader/following";
import { backFrom } from "../reader/home";
import { Reader } from "../reader/Reader";

/** What the address of the reader says besides the book. */
type Asked = {
  /**
   * The reading list the book is read from, which the reader then follows:
   * at the end of the book it goes on to the next of the list.
   */
  list?: number;
  /**
   * The book is read from its group of its series, and the reader stops at
   * the end of the group. Without it, and without a list, it follows the
   * series from one group to the next.
   */
  group?: true;
  /** The page to open on, from 1: for a look at one page, with no question asked. */
  page?: number;
  /**
   * A finished book is started over with no question asked. It says nothing
   * about a book that is being read, so that loading the page again in the
   * middle of the book opens it where it was left.
   */
  again?: true;
  /** The reading is not recorded: turning pages moves no progress. */
  unrecorded?: true;
};

function whole(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) ? value : undefined;
}

export const Route = createFileRoute("/read/$bookId")({
  params: {
    parse: ({ bookId }) => ({ bookId: Number(bookId) }),
    stringify: ({ bookId }) => ({ bookId: String(bookId) }),
  },
  validateSearch: (search: Record<string, unknown>): Asked => ({
    list: whole(search.list),
    group: search.group === true ? true : undefined,
    page: whole(search.page),
    again: search.again === true ? true : undefined,
    unrecorded: search.unrecorded === true ? true : undefined,
  }),
  component: ReadPage,
});

/** The reader has no page around it, so what is shown instead of it brings its own margins. */
function Instead({ children }: { children: ReactNode }) {
  return <main className="px-gutter py-8 md:py-12">{children}</main>;
}

/** Leads back to where the book was opened from, as closing the reader does. */
function BackLink({ book, listId }: { book: Book; listId?: number }) {
  return (
    <Link {...backFrom(book, listId)} className={textLinkClass}>
      {listId !== undefined
        ? "Back to the reading list"
        : book.series_id != null
          ? "Back to the series"
          : "Back to the library"}
    </Link>
  );
}

/** How long a book is left alone before opening it asks where to start, in seconds: 90 days. */
const LONG_AGO = 90 * 24 * 60 * 60;

/** Where a book is opened from when nobody says: where it was left, and a finished one at its start. */
function leftAt(book: Book): number {
  return book.progress && !book.progress.completed ? book.progress.page : 0;
}

/**
 * Asks where to start a book that was finished, or left alone for a long
 * time: opening it and turning a page would move its progress, which is
 * not what someone who only wants a look means.
 */
function WhereToStart({
  book,
  progress,
  listId,
  onStart,
  onLook,
}: {
  book: Book;
  /** How far the book was read: it was opened before. */
  progress: NonNullable<Book["progress"]>;
  /** The reading list the book is read from. */
  listId?: number;
  /** Reads it, recorded, from its start or from where it was left. */
  onStart: (from: "start" | "left") => void;
  /** Reads it without recording. */
  onLook: () => void;
}) {
  const finished = progress.completed;
  return (
    <Notice
      icon={finished ? BookOpenCheck : History}
      title={called(book)}
      action={
        <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
          {finished ? (
            <Button variant="primary" onClick={() => onStart("start")}>
              Read it again
            </Button>
          ) : (
            <>
              <Button variant="primary" onClick={() => onStart("left")}>
                Go on from page {(progress.page + 1).toLocaleString()}
              </Button>
              <Button onClick={() => onStart("start")}>Start again</Button>
            </>
          )}
          <Button onClick={onLook}>Only look</Button>
          <BackLink book={book} listId={listId} />
        </div>
      }
    >
      {finished
        ? `You finished this book on ${day(progress.updated_at)}. Reading it again starts it over, ` +
          "and it counts as not read until you reach its end. A look changes nothing."
        : `You left this book on page ${(progress.page + 1).toLocaleString()} of ` +
          `${book.page_count.toLocaleString()}, ${ago(progress.updated_at)}. ` +
          "A look does not move your place."}
    </Notice>
  );
}

function ReadPage() {
  const { bookId } = Route.useParams();
  const { list: listId, group, page: askedPage, again, unrecorded } = Route.useSearch();
  const navigate = useNavigate();
  const book = useBook(bookId);
  const pages = usePages(bookId);
  // How the reader shows a book is the person's, kept by the server: the
  // book waits for it, so that it does not open one way and turn another.
  const settings = useSettings();
  const { followed, next } = useFollowing(bookId, listId, group === true);
  /**
   * Where the book in the address starts, once that is known without
   * asking: answered here, or opened from the book before it.
   */
  const [told, setTold] = useState<{ bookId: number; from: "start" | "left" }>();

  /**
   * Opens the book that follows in this one's place, so that leaving the
   * reader leads back to where the first book was opened from. The page
   * asked for was this book's.
   */
  const openNext = useCallback(
    (next: Book) => {
      setTold({ bookId: next.id, from: "left" });
      void navigate({
        to: "/read/$bookId",
        params: { bookId: next.id },
        search: (asked) => ({ ...asked, page: undefined }),
        replace: true,
      });
    },
    [navigate],
  );
  const record = useCallback(
    (recorded: boolean) => {
      // The book is open, or opens for a look: where it starts is settled,
      // and recording it later from the page it is on asks nothing.
      setTold((told) => (told?.bookId === bookId ? told : { bookId, from: "left" }));
      void navigate({
        to: ".",
        search: (asked) => ({ ...asked, unrecorded: recorded ? undefined : true }),
        replace: true,
      });
    },
    [navigate, bookId],
  );

  if (book.isError) {
    return (
      <Instead>
        <BookNotOpened error={book.error} />
      </Instead>
    );
  }
  if (gotNoAnswer(pages.error)) {
    return (
      <Instead>
        <ServerNotAnswering />
      </Instead>
    );
  }
  if (book.isPending || pages.isPending || settings.isPending) {
    return null;
  }

  const identity = book.data.identity;
  const problem =
    book.data.problem ?? pages.error?.message ?? (pages.data?.length ? null : "It has no pages.");
  if (problem || !identity || !pages.data) {
    return (
      <Instead>
        <Notice
          icon={FileWarning}
          title={`${called(book.data)} cannot be read`}
          action={<BackLink book={book.data} listId={listId} />}
        >
          {problem}
        </Notice>
      </Instead>
    );
  }

  const progress = book.data.progress;
  const here = told?.bookId === bookId ? told : undefined;
  const toAsk =
    !here &&
    !unrecorded &&
    askedPage === undefined &&
    progress != null &&
    (progress.completed ? !again : Date.now() / 1000 - progress.updated_at > LONG_AGO);
  if (toAsk) {
    return (
      <Instead>
        <WhereToStart
          book={book.data}
          progress={progress}
          listId={listId}
          onStart={(from) => setTold({ bookId, from })}
          onLook={() => record(false)}
        />
      </Instead>
    );
  }

  // Keyed by the book, so another book starts from its own page.
  return (
    <Reader
      key={book.data.id}
      book={book.data}
      identity={identity}
      pages={pages.data}
      startAt={
        askedPage !== undefined ? askedPage - 1 : here?.from === "start" ? 0 : leftAt(book.data)
      }
      listId={listId}
      followed={followed}
      next={next}
      onNext={openNext}
      recorded={!unrecorded}
      onRecord={record}
    />
  );
}
