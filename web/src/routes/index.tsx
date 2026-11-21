import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowUpDown, Plus } from "lucide-react";
import { Fragment, type ReactNode, useState } from "react";
import { useIsOwner } from "../api/account";
import type { Book, Collection, Library, ReadingList, Series } from "../api/client";
import { useCollectionsOnHome, useFirstCollectionSeries } from "../api/collections";
import { useLibraries } from "../api/libraries";
import { useReadingLists } from "../api/lists";
import {
  useContinueReading,
  useRecentlyAdded,
  useRecentlyUpdated,
  useUpNext,
} from "../api/reading";
import { useSettings } from "../api/settings";
import { useWantToRead } from "../api/wanted";
import { AddLibraryDialog } from "../components/AddLibraryDialog";
import { ArrangeHomeDialog } from "../components/ArrangeHomeDialog";
import { BookCard } from "../components/BookCard";
import { Button } from "../components/Button";
import { ComicLibraryIcon } from "../components/ComicLibraryIcon";
import { ListCard } from "../components/ListCard";
import { Notice, ServerNotAnswering } from "../components/Notice";
import { ScanProgress } from "../components/ScanProgress";
import { SeriesCard } from "../components/SeriesCard";
import { Shelf, shelfTitleClass } from "../components/Shelf";
import { ErrorText, textLinkClass } from "../components/text";
import { contents } from "../format";
import { arranged, arrangementIn, collectionRow, HOME_ROWS_KEPT_AS } from "../homeRows";

export const Route = createFileRoute("/")({ component: Home });

function LibraryCard({ library }: { library: Library }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId"
        params={{ libraryId: library.id }}
        className="flex h-full flex-col rounded-card border border-divider bg-surface p-6 transition-colors duration-150 hover:border-outline"
      >
        <ComicLibraryIcon className="size-8 text-accent" />
        <h3 className="mt-3 truncate font-semibold text-card-title">{library.name}</h3>
        <p className="mt-1 text-fg-muted text-metadata">{contents(library)}</p>
        <div className="mt-3 empty:hidden">
          <ScanProgress scan={library.scan} />
        </div>
      </Link>
    </li>
  );
}

function bookCard(book: Book) {
  return <BookCard key={book.id} book={book} />;
}

function seriesCard(series: Series) {
  return <SeriesCard key={series.id} series={series} titleAs="h3" />;
}

/** How many series a row of them shows. The rest are a click away. */
const SERIES_IN_A_ROW = 24;

/** The way from a row of series to all of them, which says how many when the row shows fewer. */
function allOf(total: number): string {
  return total > SERIES_IN_A_ROW ? `All ${total.toLocaleString()}` : "See all";
}

/**
 * A titled line of covers, a card for each of `items`. Nothing at all while
 * there are none to show. `action` goes at the end of the title's line.
 */
function Row<Item>({
  title,
  items,
  problem,
  card,
  action,
}: {
  title: string;
  items: Item[] | undefined;
  /** Why the items could not be asked for, said in the row's place. */
  problem: Error | null;
  card: (item: Item) => ReactNode;
  action?: ReactNode;
}) {
  if (problem) {
    return (
      <ErrorText className="mb-8">
        {title}: {problem.message}
      </ErrorText>
    );
  }
  if (!items?.length) {
    return null;
  }
  return (
    <Shelf title={title} action={action} inOneLine>
      {items.map(card)}
    </Shelf>
  );
}

/** What Home says while the server has no library. Adding one is the owner's. */
function NoLibraries() {
  const isOwner = useIsOwner();
  const [adding, setAdding] = useState(false);
  if (!isOwner) {
    return (
      <Notice icon={ComicLibraryIcon} title="Nothing to read yet">
        The owner of this server has not given you a library yet.
      </Notice>
    );
  }
  return (
    <>
      <Notice
        icon={ComicLibraryIcon}
        title="No libraries yet"
        action={
          <Button variant="primary" onClick={() => setAdding(true)}>
            <Plus className="size-5" aria-hidden="true" />
            Add a library
          </Button>
        }
      >
        Tell the server where your comics are. It reads the folder, makes covers, and never changes
        anything in it.
      </Notice>
      <AddLibraryDialog open={adding} onClose={() => setAdding(false)} />
    </>
  );
}

/** How many reading lists Home's row shows. The rest are on their own page. */
const LISTS_SHOWN = 24;

/**
 * Where a reading list stands in Home's row: the ones being read first,
 * then the ones not started, then the ones read.
 */
function standing(list: ReadingList): number {
  if (list.unread_count === 0) {
    return 2;
  }
  return list.unread_count < list.book_count ? 0 : 1;
}

/** A collection as a row: the first of its series, by name, and the way to all of them. */
function CollectionRow({ collection }: { collection: Collection }) {
  const series = useFirstCollectionSeries(collection.id, SERIES_IN_A_ROW);
  return (
    <Row
      title={collection.name}
      items={series.data}
      problem={series.error}
      card={seriesCard}
      action={
        <Link
          to="/collections/$collectionId"
          params={{ collectionId: collection.id }}
          className={textLinkClass}
        >
          {allOf(collection.series_count)}
        </Link>
      }
    />
  );
}

// Home's own rows. Each asks for what it shows, so one put away asks for
// nothing.

function ContinueReading({ title }: { title: string }) {
  const reading = useContinueReading();
  return <Row title={title} items={reading.data} problem={reading.error} card={bookCard} />;
}

function UpNext({ title }: { title: string }) {
  const next = useUpNext();
  return (
    <Row
      title={title}
      items={next.data}
      problem={next.error}
      card={(book) => <BookCard key={book.id} book={book} upNext />}
    />
  );
}

function WantToRead({ title }: { title: string }) {
  const wanted = useWantToRead(1, SERIES_IN_A_ROW);
  return (
    <Row
      title={title}
      items={wanted.data?.series}
      problem={wanted.error}
      card={seriesCard}
      action={
        <Link to="/want-to-read" className={textLinkClass}>
          {allOf(wanted.data?.total ?? 0)}
        </Link>
      }
    />
  );
}

function ReadingLists({ title }: { title: string }) {
  const lists = useReadingLists();
  return (
    <Row
      title={title}
      items={[...(lists.data ?? [])]
        .sort((one, other) => standing(one) - standing(other))
        .slice(0, LISTS_SHOWN)}
      problem={lists.error}
      card={(list) => <ListCard key={list.id} list={list} titleAs="h3" />}
      action={
        <Link to="/lists" className={textLinkClass}>
          All reading lists
        </Link>
      }
    />
  );
}

function RecentlyUpdated({ title }: { title: string }) {
  const updated = useRecentlyUpdated();
  return <Row title={title} items={updated.data} problem={updated.error} card={seriesCard} />;
}

function RecentlyAdded({ title }: { title: string }) {
  const recent = useRecentlyAdded();
  return <Row title={title} items={recent.data} problem={recent.error} card={bookCard} />;
}

/** One of Home's own rows, under the id an arrangement names it by. */
function own(id: string, name: string, Shown: (props: { title: string }) => ReactNode) {
  return { id, name, row: <Shown title={name} /> };
}

function Home() {
  const libraries = useLibraries();
  // Only the collections that are rows: the rest are not worked out for Home.
  const collections = useCollectionsOnHome();
  const settings = useSettings();
  const [arranging, setArranging] = useState(false);

  const onHome = collections.data ?? [];
  // Every row there is, in the order they come in until arranged: what
  // the reader wants to read comes before the reading lists, and the
  // collections that are rows, by name, after them.
  const rows = [
    own("continue", "Continue reading", ContinueReading),
    own("up-next", "Up next", UpNext),
    own("want-to-read", "Want to read", WantToRead),
    own("lists", "Reading lists", ReadingLists),
    ...onHome.map((collection) => ({
      id: collectionRow(collection.id),
      name: collection.name,
      row: <CollectionRow collection={collection} />,
    })),
    own("updated", "Recently updated", RecentlyUpdated),
    own("added", "Recently added", RecentlyAdded),
  ];
  const arrangement = arrangementIn(settings.data?.[HOME_ROWS_KEPT_AS]);
  const inOrder = arranged(rows, arrangement.order);

  return (
    <>
      {/* The menu already says which page this is, and adds a library. */}
      <h1 className="sr-only">Home</h1>

      {libraries.isError ? (
        <ServerNotAnswering />
      ) : libraries.data?.length === 0 ? (
        <NoLibraries />
      ) : (
        <>
          {collections.isError ? (
            <ErrorText className="mb-8">Collections: {collections.error.message}</ErrorText>
          ) : null}
          {/* Until it is known how they are arranged, no row shows where it will not stay. */}
          {settings.isPending || collections.isPending
            ? null
            : inOrder
                .filter((row) => !arrangement.hidden.includes(row.id))
                .map((row) => <Fragment key={row.id}>{row.row}</Fragment>)}
          {libraries.data ? (
            <section>
              <h2 className={shelfTitleClass}>Libraries</h2>
              <ul className="mt-4 grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-4">
                {libraries.data.map((library) => (
                  <LibraryCard key={library.id} library={library} />
                ))}
              </ul>
            </section>
          ) : null}
          <div className="mt-12">
            {/* Not while the collections that are rows are unknown: saving then would forget them. */}
            <Button
              variant="quiet"
              onClick={() => setArranging(true)}
              disabled={!collections.isSuccess}
              className="-ml-4"
            >
              <ArrowUpDown className="size-4" aria-hidden="true" />
              Arrange Home
            </Button>
          </div>
          <ArrangeHomeDialog
            open={arranging}
            onClose={() => setArranging(false)}
            rows={inOrder}
            hidden={arrangement.hidden}
          />
        </>
      )}
    </>
  );
}
