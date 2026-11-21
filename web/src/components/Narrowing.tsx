import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowDownAZ,
  ArrowUpDown,
  Ban,
  BookOpen,
  CalendarDays,
  CalendarPlus,
  Check,
  History,
  type LucideIcon,
  Shuffle,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import type { LibraryChoices, NamedPerson } from "../api/client";
import { useLibraryChoices } from "../api/libraries";
import { count } from "../format";
import {
  type Asked,
  decadeOf,
  type NarrowedBy,
  narrowedBy,
  nextStanding,
  READ_STATES,
  type ReadState,
  SORTS,
  type Sort,
  type Standing,
  standingOf,
  unnarrowed,
  type ValueOf,
  withStanding,
} from "../narrowing";
import { choiceProps } from "./BookGrid";
import { buttonClass } from "./Button";
import { Menu, MenuItem, MenuLink } from "./Menu";
import { navHeadingClass } from "./NavLink";
import { textLinkClass } from "./text";

/** What each order is called in the menu, and what the button says of it, turned around or not. */
const ORDER_NAMES: Record<Sort, { label: string; icon: LucideIcon; said: [string, string] }> = {
  name: { label: "Name", icon: ArrowDownAZ, said: ["Name, A to Z", "Name, Z to A"] },
  added: { label: "Date added", icon: CalendarPlus, said: ["Newest added", "Oldest added"] },
  year: { label: "Year", icon: CalendarDays, said: ["Oldest first", "Newest first"] },
  last_read: { label: "Last read", icon: History, said: ["Read lately", "Read longest ago"] },
  unread: { label: "Unread first", icon: BookOpen, said: ["Unread first", "Read first"] },
  random: { label: "Shuffle", icon: Shuffle, said: ["Shuffled", "Shuffled"] },
};

/** The orders that can be turned around, in the order of the menu. */
const ORDERS = SORTS.filter((sort) => sort !== "random");

const READ_NAMES: Record<ReadState, string> = {
  unread: "Unread",
  in_progress: "In progress",
  read: "Read",
};

/** How many values of one thing show before the rest are asked for. */
const SHOWN = 10;
/** How many show once they are: the box over them finds the others. */
const MOST_SHOWN = 60;

/** Whether the choices show, kept in this browser. */
const OPEN_KEY = "binge.narrowing-open";

/** Until someone says, the choices show where there is room for them. */
function startsOpen(): boolean {
  try {
    const kept = localStorage.getItem(OPEN_KEY);
    if (kept) {
      return kept === "yes";
    }
  } catch {
    // Storage is blocked: as if nothing was kept.
  }
  return window.matchMedia("(min-width: 64rem)").matches;
}

function keepOpen(open: boolean) {
  try {
    localStorage.setItem(OPEN_KEY, open ? "yes" : "no");
  } catch {
    // Not kept for next time; it still applies now.
  }
}

/** The menu that orders the series. */
function Order({ libraryId, asked }: { libraryId: number; asked: Asked }) {
  const navigate = useNavigate();
  const sort = asked.sort ?? "name";
  const link = { to: "/libraries/$libraryId", params: { libraryId }, replace: true } as const;
  return (
    <Menu
      label={`Order of the series: ${ORDER_NAMES[sort].said[asked.reverse ? 1 : 0]}`}
      className={buttonClass()}
      button={
        <>
          <ArrowUpDown className="size-4 text-fg-muted" aria-hidden="true" />
          {ORDER_NAMES[sort].said[asked.reverse ? 1 : 0]}
        </>
      }
    >
      {ORDERS.map((order) => (
        <MenuLink
          key={order}
          {...link}
          search={{
            ...asked,
            sort: order === "name" ? undefined : order,
            reverse: undefined,
            seed: undefined,
            page: undefined,
          }}
          icon={ORDER_NAMES[order].icon}
          chosen={order === sort}
        >
          {ORDER_NAMES[order].label}
        </MenuLink>
      ))}
      {/* A new shuffle each time it is asked for. */}
      <MenuItem
        icon={Shuffle}
        chosen={sort === "random"}
        onSelect={() =>
          navigate({
            ...link,
            search: {
              ...asked,
              sort: "random",
              reverse: undefined,
              seed: Math.floor(Math.random() * 1_000_000),
              page: undefined,
            },
          })
        }
      >
        {ORDER_NAMES.random.label}
      </MenuItem>
      {sort === "random" ? null : (
        <MenuLink
          {...link}
          search={{ ...asked, reverse: asked.reverse ? undefined : true, page: undefined }}
          icon={ArrowUpDown}
        >
          Turn the order around
        </MenuLink>
      )}
    </Menu>
  );
}

const chipClass =
  "inline-flex min-h-9 max-w-full items-center gap-1.5 rounded-full px-3 text-metadata " +
  "transition-colors duration-150 [@media(hover:none)]:min-h-11";
const chipLooks: Record<Standing, string> = {
  off: "bg-raised text-fg hover:bg-divider",
  asked: "bg-fg font-semibold text-background",
  ruled_out: "text-fg-muted ring-1 ring-outline ring-inset",
};
/** The box that finds among the values of one thing: as tall as a chip, and in its line. */
const findClass =
  "min-h-9 w-40 rounded-full border border-outline bg-background px-3 text-fg text-metadata " +
  "placeholder:text-fg-muted [@media(hover:none)]:min-h-11";
/** What a click on a value does next, said under the pointer and read out. */
const nextSaid: Record<Standing, string> = {
  off: "Show only these",
  asked: "Asked for. Leave these out instead",
  ruled_out: "Left out. Stop narrowing by this",
};

/**
 * One value the series can be narrowed by. A click asks for it, a second
 * rules it out, and a third lets it be.
 */
function Chip<By extends NarrowedBy>({
  libraryId,
  asked,
  by,
  value,
  children,
}: {
  libraryId: number;
  asked: Asked;
  by: By;
  value: ValueOf[By];
  children: string;
}) {
  const standing = standingOf(asked, by, value);
  return (
    <Link
      to="/libraries/$libraryId"
      params={{ libraryId }}
      search={withStanding(asked, by, value, nextStanding(standing))}
      {...choiceProps}
      title={nextSaid[standing]}
      aria-label={`${children}. ${nextSaid[standing]}`}
      className={`${chipClass} ${chipLooks[standing]}`}
    >
      {standing === "asked" ? <Check className="size-4 shrink-0" aria-hidden="true" /> : null}
      {standing === "ruled_out" ? <Ban className="size-4 shrink-0" aria-hidden="true" /> : null}
      <span className={`truncate ${standing === "ruled_out" ? "line-through" : ""}`}>
        {children}
      </span>
    </Link>
  );
}

/** One thing the series are narrowed by: its name, and its values as chips. */
function Kind({ name, children }: { name: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-x-4 gap-y-1.5 sm:flex-row sm:items-baseline">
      <h3 className={`${navHeadingClass} shrink-0 sm:w-24`}>{name}</h3>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">{children}</div>
    </div>
  );
}

/** A value of a thing with many: what it is in the address, and what it is called. */
type Named<Value> = { value: Value; name: string };

/**
 * The values of a thing that may have hundreds: the ones most series have,
 * with those in force, until the rest are asked for. Then a box finds
 * among them.
 */
function Many<By extends "publisher" | "writer" | "penciller">({
  libraryId,
  asked,
  by,
  name,
  values,
}: {
  libraryId: number;
  asked: Asked;
  by: By;
  name: string;
  /** The ones most series have first. */
  values: Named<ValueOf[By]>[];
}) {
  const [all, setAll] = useState(false);
  const [typed, setTyped] = useState("");
  if (values.length === 0) {
    return null;
  }
  const wanted = typed.trim().toLowerCase();
  const shown = all
    ? values.filter((one) => one.name.toLowerCase().includes(wanted)).slice(0, MOST_SHOWN)
    : values.filter((one, place) => place < SHOWN || standingOf(asked, by, one.value) !== "off");
  const more = values.length - shown.length;
  return (
    <Kind name={name}>
      {all ? (
        <input
          type="search"
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
          aria-label={`Find a ${name.toLowerCase()}`}
          placeholder="Find…"
          // biome-ignore lint/a11y/noAutofocus: it was just asked for, by the button it takes the place of
          autoFocus
          className={findClass}
        />
      ) : null}
      {shown.map((one) => (
        <Chip key={one.value} libraryId={libraryId} asked={asked} by={by} value={one.value}>
          {one.name}
        </Chip>
      ))}
      {all && shown.length === 0 ? (
        <span className="text-fg-muted text-metadata">None like that.</span>
      ) : null}
      {!all && more > 0 ? (
        <button type="button" onClick={() => setAll(true)} className={`${chipClass} text-accent`}>
          {more.toLocaleString()} more…
        </button>
      ) : null}
      {all ? (
        <button
          type="button"
          onClick={() => {
            setAll(false);
            setTyped("");
          }}
          className={`${chipClass} text-accent`}
        >
          Show fewer
        </button>
      ) : null}
    </Kind>
  );
}

/** What a person in the address is called: the library's books say. */
function nameOf(people: NamedPerson[] | undefined, id: number): string {
  return people?.find((person) => person.id === id)?.name ?? "Someone not in this library";
}

/**
 * Every value in force: what to call it, whether it is ruled out, and what
 * is asked without it.
 */
function inForce(asked: Asked, choices: LibraryChoices | undefined) {
  const of = <By extends NarrowedBy>(by: By, said: (value: ValueOf[By]) => string) =>
    ([by, `not_${by}`] as const).flatMap((list) =>
      ((asked[list] ?? []) as ValueOf[By][]).map((value) => ({
        key: `${list} ${value}`,
        name: said(value),
        out: list !== by,
        without: withStanding(asked, by, value, "off"),
      })),
    );
  return [
    ...of("read", (state) => READ_NAMES[state]),
    ...of("publisher", (name) => name),
    ...of("year", (years) => years),
    ...of("writer", (id) => `Written by ${nameOf(choices?.writers, id)}`),
    ...of("penciller", (id) => `Drawn by ${nameOf(choices?.pencillers, id)}`),
  ];
}

/**
 * The order of a library's series and what narrows them, over the grid.
 * All of it is in the address: each choice is a link to the library as it
 * would then be.
 *
 * `shown` is how many series what is asked leaves in, once known, and `of`
 * how many the library has.
 */
export function Narrowing({
  libraryId,
  asked,
  shown,
  of,
}: {
  libraryId: number;
  asked: Asked;
  shown: number | undefined;
  of: number;
}) {
  const [open, setOpen] = useState(startsOpen);
  // The people asked for and ruled out, by their ids.
  const people = [asked.writer, asked.not_writer, asked.penciller, asked.not_penciller].flat();
  // Put away, the choices are still what says the names of the people in force.
  const choices = useLibraryChoices(libraryId, open || people.some(Boolean)).data;
  const inForceNow = narrowedBy(asked);
  const years = choices?.decades.map((decade) => decadeOf(decade.first_year)) ?? [];
  // A year in force that is no ten of them, as one a book's page leads to.
  const otherYears = [...(asked.year ?? []), ...(asked.not_year ?? [])].filter(
    (year) => !years.includes(year),
  );
  const named = (people: NamedPerson[] | undefined) =>
    people?.map((person) => ({ value: person.id, name: person.name })) ?? [];
  // The people asked for have a page of their own, with their books in every library.
  const askedFor = [...new Set([...(asked.writer ?? []), ...(asked.penciller ?? [])])]
    .map((id) =>
      [...(choices?.writers ?? []), ...(choices?.pencillers ?? [])].find((one) => one.id === id),
    )
    .filter((person) => person !== undefined);
  const clear = (
    <Link
      to="/libraries/$libraryId"
      params={{ libraryId }}
      search={unnarrowed(asked)}
      {...choiceProps}
      className={`${textLinkClass} text-metadata`}
    >
      Clear
    </Link>
  );

  return (
    <section aria-label="Order and narrowing" className="mb-6">
      <div className="flex flex-wrap items-center gap-2">
        <Order libraryId={libraryId} asked={asked} />
        <button
          type="button"
          onClick={() => {
            keepOpen(!open);
            setOpen(!open);
          }}
          aria-expanded={open}
          className={buttonClass()}
        >
          <SlidersHorizontal className="size-4 text-fg-muted" aria-hidden="true" />
          Narrow
          {inForceNow > 0 ? (
            <span className="rounded-full bg-accent px-2 py-0.5 text-caption text-on-accent">
              {inForceNow.toLocaleString()}
              <span className="sr-only"> in force</span>
            </span>
          ) : null}
        </button>
        <p className="ml-auto text-fg-muted text-metadata" aria-live="polite">
          {shown === undefined
            ? null
            : inForceNow > 0
              ? `${shown.toLocaleString()} of ${count(of, "series", "series")}`
              : count(shown, "series", "series")}
        </p>
      </div>

      {open ? (
        <div className="mt-4 flex flex-col gap-3 rounded-card border border-divider bg-surface p-4">
          <Kind name="Reading">
            {READ_STATES.map((state) => (
              <Chip key={state} libraryId={libraryId} asked={asked} by="read" value={state}>
                {READ_NAMES[state]}
              </Chip>
            ))}
          </Kind>
          <Many
            libraryId={libraryId}
            asked={asked}
            by="publisher"
            name="Publisher"
            values={choices?.publishers.map(({ name }) => ({ value: name, name })) ?? []}
          />
          {years.length + otherYears.length > 0 ? (
            <Kind name="Years">
              {[...years, ...otherYears].map((year) => (
                <Chip key={year} libraryId={libraryId} asked={asked} by="year" value={year}>
                  {year}
                </Chip>
              ))}
            </Kind>
          ) : null}
          <Many
            libraryId={libraryId}
            asked={asked}
            by="writer"
            name="Writer"
            values={named(choices?.writers)}
          />
          <Many
            libraryId={libraryId}
            asked={asked}
            by="penciller"
            name="Penciller"
            values={named(choices?.pencillers)}
          />
          <p className="text-caption text-fg-muted">
            Click once to show only those, twice to leave them out. {inForceNow > 0 ? clear : null}
          </p>
        </div>
      ) : inForceNow > 0 ? (
        // What is in force stays in sight while the choices are put away.
        <ul aria-label="Narrowed by" className="mt-3 flex flex-wrap items-center gap-2">
          {inForce(asked, choices).map(({ key, name, out, without }) => (
            <li key={key} className="max-w-full">
              <Link
                to="/libraries/$libraryId"
                params={{ libraryId }}
                search={without}
                {...choiceProps}
                aria-label={`${out ? "Not " : ""}${name}. Stop narrowing by this`}
                className={`${chipClass} ${chipLooks[out ? "ruled_out" : "asked"]}`}
              >
                <span className={`truncate ${out ? "line-through" : ""}`}>{name}</span>
                <X className="size-4 shrink-0" aria-hidden="true" />
              </Link>
            </li>
          ))}
          <li>{clear}</li>
        </ul>
      ) : null}
      {askedFor.map((person) => (
        <p key={person.id} className="mt-3 text-metadata">
          <Link to="/people/$personId" params={{ personId: person.id }} className={textLinkClass}>
            Everything by {person.name}
          </Link>
        </p>
      ))}
    </section>
  );
}
