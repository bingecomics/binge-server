import { createFileRoute, Link } from "@tanstack/react-router";
import { BookOpen, ChevronLeft } from "lucide-react";
import type { ReactNode } from "react";
import type { Book } from "../api/client";
import { useBookDetails, useLibrary } from "../api/libraries";
import { useListsOfBook } from "../api/lists";
import { useBookCredits } from "../api/people";
import { AddToListChoice } from "../components/AddToList";
import { BookReadMarks, doneOf, ReadUnrecorded, Standing } from "../components/BookCard";
import { buttonClass } from "../components/Button";
import { Cover } from "../components/Cover";
import { CoverChoices } from "../components/CoverChoices";
import { Fact, Facts } from "../components/Facts";
import { MoreMenu } from "../components/Menu";
import { NarrowedLink, PeopleLinks } from "../components/NarrowedLink";
import { BookNotOpened } from "../components/Notice";
import { textLinkClass, upLinkClass } from "../components/text";
import { called, language, moment, published, size, titleUnder, yearOf } from "../format";
import { homeOf } from "../reader/home";

export const Route = createFileRoute("/books/$bookId")({
  params: {
    parse: ({ bookId }) => ({ bookId: Number(bookId) }),
    stringify: ({ bookId }) => ({ bookId: String(bookId) }),
  },
  component: BookPage,
});

/** The way back up: to the book's series, or to its library when it is in no series. */
function Up({ book }: { book: Book }) {
  const { library } = useLibrary(book.library_id);
  // There from the start, so the page does not jump when the library's name arrives.
  return (
    <Link {...homeOf(book)} className={upLinkClass}>
      <ChevronLeft className="size-4" aria-hidden="true" />
      {book.series_name ?? library?.name ?? "Library"}
    </Link>
  );
}

/** One thing known about the book, with its label. Nothing when there is nothing to say. */
function Known({ label, children }: { label: string; children: ReactNode }) {
  return children ? <Fact label={label}>{children}</Fact> : null;
}

/**
 * What is known about the book, each with its label. Its publisher, its
 * year and the people it names lead to its library narrowed by them. Until
 * the people have arrived, the names are as the file spells them.
 */
function BookFacts({ book }: { book: Book }) {
  const credits = useBookCredits(book.id).data;
  const info = book.info;
  const year = yearOf(book);
  const when = info ? published(info) : null;
  return (
    <>
      <Known label="Writer">
        {credits?.writers.length ? (
          <PeopleLinks libraryId={book.library_id} by="writer" people={credits.writers} />
        ) : (
          info?.writer
        )}
      </Known>
      <Known label="Penciller">
        {credits?.pencillers.length ? (
          <PeopleLinks libraryId={book.library_id} by="penciller" people={credits.pencillers} />
        ) : (
          info?.penciller
        )}
      </Known>
      <Known label="Publisher">
        {info?.publisher ? (
          <NarrowedLink libraryId={book.library_id} by="publisher" value={info.publisher}>
            {info.publisher}
          </NarrowedLink>
        ) : null}
      </Known>
      <Known label="Published">
        {when && year ? (
          <NarrowedLink libraryId={book.library_id} by="year" value={year}>
            {when}
          </NarrowedLink>
        ) : (
          when
        )}
      </Known>
      <Known label="Volume">{info?.volume?.toString()}</Known>
      <Known label="Language">{info?.language ? language(info.language) : null}</Known>
      <Known label="Pages">{book.problem ? null : book.page_count.toLocaleString()}</Known>
      <Known label="File name">{book.name}</Known>
      <Known label="File size">{size(book.file_size)}</Known>
      <Known label="Added">{moment(book.added_at)}</Known>
    </>
  );
}

/** The reading lists the book is in, each leading to its page. Nothing for a book in none. */
function InLists({ bookId }: { bookId: number }) {
  const lists = useListsOfBook(bookId);
  if (!lists.data?.length) {
    return null;
  }
  return (
    <Fact label={lists.data.length === 1 ? "Reading list" : "Reading lists"}>
      <ul>
        {lists.data.map((list) => (
          <li key={list.id}>
            <Link to="/lists/$listId" params={{ listId: list.id }} className={textLinkClass}>
              {list.name}
            </Link>
          </li>
        ))}
      </ul>
    </Fact>
  );
}

function BookPage() {
  const { bookId } = Route.useParams();
  const found = useBookDetails(bookId);

  if (found.isPending) {
    return null;
  }
  if (found.isError) {
    return <BookNotOpened error={found.error} />;
  }

  const book = found.data;
  // Called as its card is away from its series: "The Flash #12", its title under.
  const heading = called(book);
  const under = titleUnder(book);
  const started = book.progress && !book.progress.completed;
  return (
    <>
      <Up book={book} />
      <div className="mt-2 flex flex-col gap-8 sm:flex-row">
        <div className="w-40 shrink-0 sm:w-56">
          <Cover cover={book.cover} unreadable={Boolean(book.problem)} done={doneOf(book)} />
          <div className="mt-2">
            <Standing book={book} />
          </div>
        </div>

        <div className="min-w-0 flex-1">
          <h1 className="font-bold text-screen-title tracking-tight">{heading}</h1>
          {under ? <p className="mt-1 text-body text-fg-muted">{under}</p> : null}

          {book.problem ? null : (
            <div className="mt-6 flex gap-2">
              <Link
                to="/read/$bookId"
                params={{ bookId: book.id }}
                className={buttonClass("primary")}
              >
                <BookOpen className="size-5" aria-hidden="true" />
                {started ? "Continue reading" : "Read"}
              </Link>
              <MoreMenu about={heading}>
                <ReadUnrecorded book={book} />
                <BookReadMarks book={book} />
                <AddToListChoice what={{ books: [book.id] }} />
                <CoverChoices of={{ kind: "books", id: book.id }} chosen={book.cover_chosen} />
              </MoreMenu>
            </div>
          )}

          {book.info?.summary ? (
            <p className="mt-6 max-w-prose whitespace-pre-line">{book.info.summary}</p>
          ) : null}

          <Facts className="mt-8">
            <BookFacts book={book} />
            <InLists bookId={book.id} />
          </Facts>
        </div>
      </div>
    </>
  );
}
