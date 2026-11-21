import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";
import { memo } from "react";
import type { Book, Group } from "../api/client";
import { useCollectionsOfSeries } from "../api/collections";
import { useBooks, useLibrary, useSeriesOverview } from "../api/libraries";
import { useListsOfSeries } from "../api/lists";
import { BookGrid, choiceProps, type Shown, shownIn } from "../components/BookGrid";
import { BooksHeader } from "../components/BooksHeader";
import { coverGridClass } from "../components/Cover";
import { CoverChoices } from "../components/CoverChoices";
import { In } from "../components/Facts";
import { GroupCard } from "../components/GroupCard";
import { EmptyGroup, EmptySeries } from "../components/NoBooks";
import { NotOnServer } from "../components/Notice";
import { BooksMenu } from "../components/ReadMarks";
import { shelfTitleClass } from "../components/Shelf";
import { ErrorText, upLinkClass } from "../components/text";
import { books as bookCount, count, groupName } from "../format";

/**
 * The parts a series' page can show, in the order they are read. A part
 * holds the groups of one kind, or with no kind the books in no group.
 * Every kind is shown the same way: see [`Tab`].
 */
const PARTS = [
  { id: "volumes", label: "Volumes", kind: "volume" },
  { id: "limited", label: "Limited series", kind: "limited" },
  { id: "issues", label: "Issues", kind: null },
  { id: "annuals", label: "Annuals", kind: "annual" },
  { id: "collected", label: "Collected editions", kind: "collected" },
  { id: "one-shots", label: "One-shots", kind: "one_shot" },
  { id: "appearances", label: "Appearances", kind: "appearance" },
] as const;
type Part = (typeof PARTS)[number];

/** Which of a series' books the page shows. */
type Showing = Shown & {
  /** The part. Missing for the one the reader is in. */
  tab?: Part["id"];
};

export const Route = createFileRoute("/series/$seriesId")({
  params: {
    parse: ({ seriesId }) => ({ seriesId: Number(seriesId) }),
    stringify: ({ seriesId }) => ({ seriesId: String(seriesId) }),
  },
  // In the address, so that coming back from a book finds the same books.
  validateSearch: (search: Record<string, unknown>): Showing => ({
    ...shownIn(search),
    tab: PARTS.find((part) => part.id === search.tab)?.id,
  }),
  component: SeriesPage,
});

/**
 * A part of one series. Its groups show as cards, each opening the group's
 * own page, when there are several to choose from. A part that is one group
 * shows that group's books at once, under its name or number when it has
 * one: a card would only say the tab's name again. So do the books of the
 * group with no name and no number, which are the ones of the kind that are
 * in no named part of it.
 */
type Tab = Part & {
  groups: Group[];
  /** The groups shown as cards. */
  cards: Group[];
  /** The books shown themselves, in the order they are read. */
  books: Book[];
  /** The group whose books show themselves and that holds none: it shows the way to delete it. */
  empty: Group | undefined;
};

/** Whether a group is told apart from the others of its kind by a name or a number. */
function isNamed(group: Group): boolean {
  return group.name != null || group.number != null;
}

/** A series' books and groups split into the parts that have anything in them. */
function tabsOf(books: Book[], groups: Group[]): Tab[] {
  const kinds = new Map(groups.map((group) => [group.id, group.kind]));
  // A book in a group the page has not been told about yet counts as in none.
  const kindOf = (book: Book) =>
    book.group_id == null ? null : (kinds.get(book.group_id) ?? null);
  return PARTS.map((part) => {
    const ofKind = groups.filter((group) => group.kind === part.kind);
    const cards = ofKind.length > 1 ? ofKind.filter(isNamed) : [];
    const inCards = new Set(cards.map((group) => group.id));
    const shown = books.filter(
      (book) =>
        kindOf(book) === part.kind && !(book.group_id != null && inCards.has(book.group_id)),
    );
    return {
      ...part,
      groups: ofKind,
      cards,
      books: shown,
      // By the group's own count too: while a scan runs, the books may be
      // told of after the group is.
      empty:
        shown.length === 0
          ? ofKind.find((group) => !inCards.has(group.id) && group.book_count === 0)
          : undefined,
    };
  }).filter((tab) => tab.groups.length > 0 || tab.books.length > 0);
}

/** How many things a tab holds: "3 groups" for the cards it shows, else "12 books". */
function held(tab: Tab): { amount: number; said: string } {
  return tab.cards.length > 0
    ? { amount: tab.cards.length, said: count(tab.cards.length, "group", "groups") }
    : { amount: tab.books.length, said: bookCount(tab.books.length) };
}

const tabClass =
  "flex min-h-11 shrink-0 items-center gap-2 border-transparent border-b-2 font-semibold " +
  "text-fg-muted text-metadata hover:text-fg aria-[current]:border-accent aria-[current]:text-fg";

/**
 * What a series holds, one part at a time when there are several: the part
 * asked for in the address, or the one the reader is in.
 */
// Memoized: the page around it changes every second while a scan runs.
const Contents = memo(function Contents({
  books,
  groups,
  next,
}: {
  /** In the order they are read. */
  books: Book[];
  groups: Group[];
  /** The book the reader goes on with. */
  next: Book | null | undefined;
}) {
  const params = Route.useParams();
  const { tab: asked, show } = Route.useSearch();
  const tabs = tabsOf(books, groups);
  const tab =
    tabs.find((tab) => tab.id === asked) ??
    tabs.find(
      (tab) =>
        tab.books.some((book) => book.id === next?.id) ||
        tab.cards.some((group) => group.id === next?.group_id),
    ) ??
    tabs[0];
  if (!tab) {
    return null;
  }
  const several = tabs.length > 1;
  const withCards = tab.cards.length > 0;
  // What the books are called. Under cards they are the rest of the kind. A
  // lone group goes by its name where no tab says it: always when it has a
  // name or a number of its own, and else when the series has no tabs.
  // Where the page has already said all there is to say, the title is only
  // read out.
  const lone = tab.groups.length === 1 ? tab.groups[0] : undefined;
  const own = lone && (isNamed(lone) || !several) ? groupName(lone) : undefined;
  const title = withCards ? "Others" : (own ?? (several ? tab.label : "Books"));

  return (
    <>
      {several ? (
        <nav
          aria-label="Parts of the series"
          // The line under the tabs is drawn inside, so that the mark of the
          // chosen one sits on it. On a narrow screen the tabs scroll sideways.
          className="mb-6 flex gap-x-6 overflow-x-auto shadow-[inset_0_-1px_0_var(--color-divider)] [scrollbar-width:none]"
        >
          {tabs.map((each) => (
            <Link
              key={each.id}
              to="/series/$seriesId"
              params={params}
              // Cards are not left out for being read.
              search={{ tab: each.id, show: each.books.length > 0 ? show : undefined }}
              {...choiceProps}
              aria-current={each === tab ? "true" : undefined}
              aria-label={`${each.label}, ${held(each).said}`}
              className={tabClass}
            >
              {each.label}
              <span className="rounded-full bg-raised px-2 py-0.5 font-normal text-caption">
                {held(each).amount.toLocaleString()}
              </span>
            </Link>
          ))}
        </nav>
      ) : null}

      {withCards ? (
        <section aria-labelledby="groups-title" className="mb-8">
          <h2 id="groups-title" className={several ? "sr-only" : `mb-4 ${shelfTitleClass}`}>
            {tab.label}
          </h2>
          <ul className={coverGridClass}>
            {tab.cards.map((group) => (
              <GroupCard key={group.id} group={group} />
            ))}
          </ul>
        </section>
      ) : null}
      {tab.empty ? (
        <EmptyGroup group={tab.empty} />
      ) : tab.books.length > 0 ? (
        <BookGrid
          title={title}
          titleHidden={!withCards && !own}
          books={tab.books}
          unread={show === "unread"}
        />
      ) : null}
    </>
  );
});

function SeriesPage() {
  const { seriesId } = Route.useParams();
  const overview = useSeriesOverview(seriesId);
  const { library } = useLibrary(overview.data?.series.library_id);
  const collections = useCollectionsOfSeries(seriesId).data;
  const lists = useListsOfSeries(seriesId).data;
  const books = useBooks(seriesId);

  if (overview.isPending) {
    return null;
  }
  if (overview.isError) {
    return (
      <NotOnServer title="This series is not on the server">
        It may have been deleted, or its library removed.
      </NotOnServer>
    );
  }

  const { series } = overview.data;
  return (
    <>
      <BooksHeader
        // A new header for each series: what was opened in one stays there.
        key={seriesId}
        cover={series.cover}
        title={series.name}
        bookCount={series.book_count}
        overview={overview.data}
        libraryId={series.library_id}
        facts={
          <>
            <In
              label={["Collection", "Collections"]}
              things={collections}
              link={(collection) => ({
                to: "/collections/$collectionId",
                params: { collectionId: collection.id },
              })}
            />
            <In
              label={["Reading list", "Reading lists"]}
              things={lists}
              link={(list) => ({ to: "/lists/$listId", params: { listId: list.id } })}
            />
          </>
        }
        menu={
          <BooksMenu
            about={series.name}
            what={{ series: [series.id] }}
            unreadCount={series.unread_count}
            wanted={series.wanted}
          >
            <CoverChoices of={{ kind: "series", id: series.id }} chosen={series.cover_chosen} />
          </BooksMenu>
        }
        up={
          library ? (
            <Link
              to="/libraries/$libraryId"
              params={{ libraryId: library.id }}
              className={upLinkClass}
            >
              <ChevronLeft className="size-4" aria-hidden="true" />
              {library.name}
            </Link>
          ) : null
        }
      />
      {books.isError ? (
        <ErrorText>{books.error.message}</ErrorText>
      ) : books.data ? (
        <>
          {books.data.length === 0 ? <EmptySeries series={series} /> : null}
          <Contents
            books={books.data}
            groups={overview.data.groups}
            next={overview.data.read.next}
          />
        </>
      ) : null}
    </>
  );
}
