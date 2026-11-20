import { createFileRoute, Link } from "@tanstack/react-router";
import { User } from "lucide-react";
import type { Person } from "../api/client";
import { useSearch } from "../api/libraries";
import { BookCard } from "../components/BookCard";
import { CollectionCard } from "../components/CollectionCard";
import { ListCard } from "../components/ListCard";
import { SeriesCard } from "../components/SeriesCard";
import { Shelf, shelfTitleClass } from "../components/Shelf";
import { ErrorText } from "../components/text";
import { workOf } from "../format";

export const Route = createFileRoute("/search")({
  // The words are in the address, so a search can be reloaded and shared.
  validateSearch: (search: Record<string, unknown>): { q?: string } =>
    typeof search.q === "string" && search.q ? { q: search.q } : {},
  component: SearchPage,
});

/** Says that a list is not all that was found. */
function Cut({ shown, what }: { shown: number; what: string }) {
  return (
    <p className="mt-4 text-fg-muted text-metadata">
      These are the first {shown} {what}. Add a word to narrow the search.
    </p>
  );
}

/** The people found, each leading to their page. They have no cover to make a card of. */
function People({ people, more }: { people: Person[]; more: boolean }) {
  return (
    <section className="mb-12">
      <h2 className={shelfTitleClass}>People</h2>
      <ul className="mt-4 grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-3">
        {people.map((person) => (
          <li key={person.id}>
            <Link
              to="/people/$personId"
              params={{ personId: person.id }}
              className="flex min-h-14 items-center gap-3 rounded-card border border-divider bg-surface px-4 py-2 transition-colors duration-150 hover:bg-raised"
            >
              <User className="size-5 shrink-0 text-fg-muted" aria-hidden="true" />
              <span className="min-w-0">
                <span className="block truncate font-semibold text-metadata">{person.name}</span>
                <span className="block truncate text-caption text-fg-muted">{workOf(person)}</span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
      {more ? <Cut shown={people.length} what="people" /> : null}
    </section>
  );
}

function Results({ words }: { words: string }) {
  const found = useSearch(words);
  if (found.isError) {
    return <ErrorText>{found.error.message}</ErrorText>;
  }
  if (!found.data) {
    return null;
  }
  const { series, books, lists, collections, people } = found.data;
  const { more_series, more_books, more_lists, more_collections, more_people } = found.data;
  if (series.length + books.length + lists.length + collections.length + people.length === 0) {
    // While the answer shown is still the one to the words before, it says
    // nothing about these.
    return found.isPlaceholderData ? null : (
      <p className="text-fg-muted">Nothing found for “{words}”.</p>
    );
  }
  return (
    <>
      {series.length ? (
        <Shelf
          title="Series"
          note={more_series ? <Cut shown={series.length} what="series" /> : null}
        >
          {series.map((one) => (
            <SeriesCard key={one.id} series={one} titleAs="h3" />
          ))}
        </Shelf>
      ) : null}
      {/* The short kinds first: books can be many, and what follows them is far down. */}
      {lists.length ? (
        <Shelf
          title="Reading lists"
          note={more_lists ? <Cut shown={lists.length} what="reading lists" /> : null}
        >
          {lists.map((list) => (
            <ListCard key={list.id} list={list} titleAs="h3" />
          ))}
        </Shelf>
      ) : null}
      {collections.length ? (
        <Shelf
          title="Collections"
          note={more_collections ? <Cut shown={collections.length} what="collections" /> : null}
        >
          {collections.map((collection) => (
            <CollectionCard key={collection.id} collection={collection} titleAs="h3" />
          ))}
        </Shelf>
      ) : null}
      {people.length ? <People people={people} more={more_people} /> : null}
      {books.length ? (
        <Shelf title="Books" note={more_books ? <Cut shown={books.length} what="books" /> : null}>
          {books.map((book) => (
            <BookCard key={book.id} book={book} />
          ))}
        </Shelf>
      ) : null}
    </>
  );
}

/**
 * Everything the words in the address find. They are typed in the box at
 * the top, which shows a few of each kind itself: see `SearchBox`.
 */
function SearchPage() {
  const { q: words } = Route.useSearch();
  return (
    <>
      {/* The box at the top already says what this page is. */}
      <h1 className="sr-only">Search</h1>
      {words ? (
        <Results words={words} />
      ) : (
        <p className="max-w-xl text-fg-muted">
          Type in the box at the top. It finds series, reading lists and collections by their name,
          books by their name, title, writer, penciller or publisher, and people by any of the names
          they go by. Capital letters and accents do not matter.
        </p>
      )}
    </>
  );
}
