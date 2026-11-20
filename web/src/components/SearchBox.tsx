import { Link, type LinkProps, useSearch as useAddress, useNavigate } from "@tanstack/react-router";
import { ArrowRight, ListOrdered, Search, User } from "lucide-react";
import {
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { coverUrl, type SearchResults } from "../api/client";
import { useSearch } from "../api/libraries";
import { books as bookCount, called, madeBy, titleUnder, workOf, yearOf } from "../format";
import { AFTER_TYPING, useSettled } from "../settled";
import { navHeadingClass } from "./NavLink";

/**
 * The keys that lead to the box from anywhere work without being shown in
 * it. On a Mac it is the command key: there Ctrl+K belongs to text fields,
 * where it cuts to the end of the line.
 */
const ON_A_MAC = /Mac|iPhone|iPad/.test(navigator.userAgent);

/** How many of each kind show under the box. The page with everything shows the rest. */
const OF_EACH = 5;

/** One thing found, as a line under the box. */
type Found = {
  /** Unlike every other line's. */
  key: string;
  name: string;
  under: string | null;
  /** Its cover, or for what has none the icon that stands for its kind. */
  cover: string | null | undefined;
  icon?: typeof User;
  link: LinkProps;
};

/** What was found, by kind, each kind with its lines. Kinds with nothing are left out. */
function groupsOf(found: SearchResults): { title: string; lines: Found[] }[] {
  const groups = [
    {
      title: "Series",
      lines: found.series.map(
        (series): Found => ({
          key: `series ${series.id}`,
          name: series.name,
          under: bookCount(series.book_count),
          cover: series.cover,
          link: { to: "/series/$seriesId", params: { seriesId: series.id } },
        }),
      ),
    },
    {
      title: "Books",
      lines: found.books.map(
        (book): Found => ({
          key: `book ${book.id}`,
          name: called(book),
          under: titleUnder(book) ?? yearOf(book),
          cover: book.cover,
          link: { to: "/books/$bookId", params: { bookId: book.id } },
        }),
      ),
    },
    {
      title: "Reading lists",
      lines: found.lists.map(
        (list): Found => ({
          key: `list ${list.id}`,
          name: list.name,
          // Two people may each have a list of one name.
          under: [bookCount(list.book_count), madeBy(list)].filter(Boolean).join(" · "),
          cover: list.cover ?? list.covers[0],
          icon: ListOrdered,
          link: { to: "/lists/$listId", params: { listId: list.id } },
        }),
      ),
    },
    {
      title: "People",
      lines: found.people.map(
        (person): Found => ({
          key: `person ${person.id}`,
          name: person.name,
          under: workOf(person),
          cover: null,
          icon: User,
          link: { to: "/people/$personId", params: { personId: person.id } },
        }),
      ),
    },
  ];
  return groups.filter((group) => group.lines.length > 0);
}

const lineClass =
  "flex min-h-11 items-center gap-3 rounded-control px-2 py-1 text-fg aria-selected:bg-raised";

/**
 * The search box in the bar at the top. What it finds shows under it as it
 * is typed, a few of each kind: series, books, reading lists and people.
 * Enter leads to the search page, which shows everything found and keeps
 * the words in its address, so a search can be reloaded and shared. The
 * arrow keys go through what is under the box, and Enter then opens it.
 */
export function SearchBox() {
  // Only the search page has words in its address. This reads the address
  // as it is, before any page has checked it, so `?q=300` typed by hand
  // arrives as a number: that is no search.
  const { q } = useAddress({ strict: false });
  const words = typeof q === "string" ? q : "";
  const navigate = useNavigate();
  const [typed, setTyped] = useState(words);
  const [open, setOpen] = useState(false);
  /** The line the arrow keys are on, from 0. The last is the way to everything. */
  const [at, setAt] = useState<number | null>(null);
  const box = useRef<HTMLInputElement>(null);
  const listId = useId();

  // When the address changes from outside, as by going back to a search,
  // the box follows it.
  useEffect(() => setTyped(words), [words]);

  const wanted = typed.trim();
  const settled = useSettled(wanted, AFTER_TYPING);
  const found = useSearch(open ? settled : "", OF_EACH);
  // An answer to other words than the ones in the box says nothing about these.
  const groups = found.data && wanted ? groupsOf(found.data) : [];
  const lines = groups.flatMap((group) => group.lines);
  const showing = open && wanted !== "" && found.data !== undefined;
  const everything: LinkProps = { to: "/search", search: { q: wanted } };

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      // Some keydowns, such as a browser filling a field in, have no key.
      if (event.key?.toLowerCase() === "k" && (ON_A_MAC ? event.metaKey : event.ctrlKey)) {
        event.preventDefault();
        box.current?.focus();
        box.current?.select();
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  function close() {
    setOpen(false);
    setAt(null);
  }

  /** Goes to what was found: the box is empty on its page, as on any but the search page. */
  function leave() {
    close();
    setTyped("");
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    if (!wanted) {
      return;
    }
    const line = at === null ? undefined : lines[at];
    if (line) {
      leave();
    } else {
      close();
    }
    // From the search page the address is replaced, so going back leaves
    // the search and not the one before it.
    void navigate(
      line ? line.link : { ...everything, replace: window.location.pathname === "/search" },
    );
  }

  function onKeyDown(event: ReactKeyboardEvent) {
    if (event.key === "Escape" && open) {
      // The first Escape puts away what was found; the box's own clears it.
      event.preventDefault();
      close();
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!showing) {
        setOpen(true);
        return;
      }
      // One more than the lines: the way to everything, after them.
      const stops = lines.length + 1;
      const step = event.key === "ArrowDown" ? 1 : -1;
      setAt(at === null ? (step === 1 ? 0 : stops - 1) : (at + step + stops) % stops);
    }
  }

  const lineId = (place: number) => `${listId}-${place}`;
  return (
    // biome-ignore lint/a11y/useSemanticElements: `<search>` is not yet in every browser the readers' owners use
    <form
      role="search"
      onSubmit={submit}
      // Put away when the keyboard or a click goes anywhere else.
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          close();
        }
      }}
      className="relative min-w-0 max-w-xl flex-1"
    >
      <Search
        className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-muted"
        aria-hidden="true"
      />
      <input
        ref={box}
        type="search"
        role="combobox"
        aria-expanded={showing}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={showing && at !== null ? lineId(at) : undefined}
        value={typed}
        onChange={(event) => {
          setTyped(event.target.value);
          setOpen(true);
          setAt(null);
        }}
        onKeyDown={onKeyDown}
        aria-label="Search series, books, reading lists and people"
        aria-keyshortcuts={ON_A_MAC ? "Meta+K" : "Control+K"}
        placeholder="Search…"
        autoComplete="off"
        spellCheck={false}
        className="h-10 w-full rounded-control border border-outline bg-background pr-3 pl-9 text-body text-fg placeholder:text-fg-muted md:text-metadata"
      />
      {/* In a narrow window what is found takes the window's width, not the box's. */}
      <div
        id={listId}
        role="listbox"
        aria-label="Found"
        hidden={!showing}
        // A click here keeps the keyboard in the box: where a link takes
        // none, the box would lose it and put this away under the click.
        onMouseDown={(event) => event.preventDefault()}
        className="fixed inset-x-2 top-14 z-30 max-h-[calc(100dvh-4.5rem)] overflow-y-auto rounded-card border border-divider bg-surface p-2 shadow-lg sm:absolute sm:inset-x-0 sm:top-full sm:mt-1"
      >
        {groups.map((group) => (
          // biome-ignore lint/a11y/useSemanticElements: a fieldset is for a form's fields, and these are choices of a listbox
          <div key={group.title} role="group" aria-label={group.title} className="mb-2">
            <p className={`px-2 py-1 ${navHeadingClass}`} aria-hidden="true">
              {group.title}
            </p>
            {group.lines.map((line) => {
              const place = lines.indexOf(line);
              const Icon = line.icon ?? Search;
              return (
                <Link
                  key={line.key}
                  {...line.link}
                  id={lineId(place)}
                  role="option"
                  aria-selected={place === at}
                  // The keyboard stays in the box: the arrows choose.
                  tabIndex={-1}
                  onClick={leave}
                  className={`${lineClass} hover:bg-raised`}
                >
                  {line.cover ? (
                    <img
                      src={coverUrl(line.cover)}
                      alt=""
                      loading="lazy"
                      className="h-12 w-8 shrink-0 rounded-sm object-cover ring-1 ring-divider"
                    />
                  ) : (
                    <span className="grid h-12 w-8 shrink-0 place-items-center rounded-sm bg-raised">
                      <Icon className="size-4 text-fg-muted" aria-hidden="true" />
                    </span>
                  )}
                  <span className="min-w-0">
                    <span className="block truncate font-semibold text-metadata">{line.name}</span>
                    {line.under ? (
                      <span className="block truncate text-caption text-fg-muted">
                        {line.under}
                      </span>
                    ) : null}
                  </span>
                </Link>
              );
            })}
          </div>
        ))}
        {/* While the answer shown is still the one to the words before, it says nothing about these. */}
        {lines.length === 0 && !found.isPlaceholderData && settled === wanted ? (
          <p className="px-2 py-2 text-fg-muted text-metadata">Nothing found for “{wanted}”.</p>
        ) : null}
        {lines.length > 0 ? (
          <Link
            {...everything}
            id={lineId(lines.length)}
            role="option"
            aria-selected={at === lines.length}
            tabIndex={-1}
            onClick={close}
            className={`${lineClass} font-semibold text-accent text-metadata hover:bg-raised`}
          >
            <ArrowRight className="size-4 shrink-0" aria-hidden="true" />
            <span className="min-w-0 truncate">Everything for “{wanted}”</span>
          </Link>
        ) : null}
      </div>
    </form>
  );
}
