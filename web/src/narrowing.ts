import { defaultStringifySearch } from "@tanstack/react-router";
import { pageAsked } from "./components/Pages";

/** What a library's series can be ordered by. The first is the order with nothing asked. */
export const SORTS = ["name", "added", "year", "last_read", "unread", "random"] as const;
export type Sort = (typeof SORTS)[number];

/** How far a reader is through a series. */
export const READ_STATES = ["unread", "in_progress", "read"] as const;
export type ReadState = (typeof READ_STATES)[number];

/** How many series a library's page shows at once. A number most widths of the grid fill rows of. */
export const SERIES_A_PAGE = 120;

/**
 * What the page of a library keeps in its address about its series: their
 * order, what they are narrowed by and which page of them is shown. So a
 * narrowed library can be reloaded, bookmarked and shared.
 *
 * Each thing to narrow by has the values asked for and, under `not_`, the
 * values ruled out. A list with nothing in it is left out.
 */
export type Asked = {
  sort?: Exclude<Sort, "name">;
  reverse?: true;
  /** Which shuffle `sort: "random"` is: the same one on every page of it. */
  seed?: number;
  read?: ReadState[];
  not_read?: ReadState[];
  publisher?: string[];
  not_publisher?: string[];
  /** A year as `1993`, or ten of them as `1990s`. */
  year?: string[];
  not_year?: string[];
  /** People, by their ids. */
  writer?: number[];
  not_writer?: number[];
  penciller?: number[];
  not_penciller?: number[];
  /** From 1. Left out for the first. */
  page?: number;
};

/** The things a library is narrowed by. */
const NARROWED_BY = ["read", "publisher", "year", "writer", "penciller"] as const;
export type NarrowedBy = (typeof NARROWED_BY)[number];

/** What a value of each thing is in the address. */
export type ValueOf = {
  read: ReadState;
  publisher: string;
  year: string;
  writer: number;
  penciller: number;
};

/** Where a value stands: not chosen, asked for, or ruled out. */
export type Standing = "off" | "asked" | "ruled_out";

/** One value given by itself, or several: what an address may hold for a list. */
function several(value: unknown): unknown[] {
  return Array.isArray(value) ? value : value == null ? [] : [value];
}

/** The values of a list, each once, or nothing when there are none. */
function some<T>(values: T[]): T[] | undefined {
  return values.length ? [...new Set(values)] : undefined;
}

/** A list of texts from an address. */
function texts(value: unknown): string[] {
  // A name that reads as a number or as yes or no arrives as one.
  return several(value)
    .filter((one) => ["string", "number", "boolean"].includes(typeof one))
    .map(String)
    .filter(Boolean);
}

function ids(value: unknown): number[] | undefined {
  return some(
    several(value)
      .map(Number)
      .filter((id) => Number.isSafeInteger(id) && id > 0),
  );
}

function years(value: unknown): string[] | undefined {
  return some(texts(value).filter((years) => /^\d{4}$|^\d{3}0s$/.test(years)));
}

function readStates(value: unknown): ReadState[] | undefined {
  const known: readonly string[] = READ_STATES;
  return some(texts(value).filter((state): state is ReadState => known.includes(state)));
}

/** Reads `Asked` from what an address asks for, leaving out what makes no sense. */
export function askedIn(search: Record<string, unknown>): Asked {
  const sort = SORTS.find((sort) => sort === search.sort);
  const seed = Number(search.seed);
  // The server takes a seed of 32 bits.
  const seeded = Number.isSafeInteger(seed) && seed >= 0 && seed <= 0xffff_ffff;
  return {
    sort: sort === "name" ? undefined : sort,
    reverse: search.reverse === true ? true : undefined,
    seed: sort === "random" && seeded ? seed : undefined,
    read: readStates(search.read),
    not_read: readStates(search.not_read),
    publisher: some(texts(search.publisher)),
    not_publisher: some(texts(search.not_publisher)),
    year: years(search.year),
    not_year: years(search.not_year),
    writer: ids(search.writer),
    not_writer: ids(search.not_writer),
    penciller: ids(search.penciller),
    not_penciller: ids(search.not_penciller),
    page: pageAsked(search.page),
  };
}

/** Whether two ask for the same series in the same order, whichever page of them. */
export function sameAsked(one: Asked, other: Asked): boolean {
  const same = (a: readonly unknown[] = [], b: readonly unknown[] = []) =>
    a.length === b.length && a.every((value) => b.includes(value));
  return (
    one.sort === other.sort &&
    one.reverse === other.reverse &&
    one.seed === other.seed &&
    NARROWED_BY.every(
      (by) => same(one[by], other[by]) && same(one[`not_${by}`], other[`not_${by}`]),
    )
  );
}

/** Where a value of one thing stands in what is asked. */
export function standingOf<By extends NarrowedBy>(
  asked: Asked,
  by: By,
  value: ValueOf[By],
): Standing {
  const has = (values: readonly unknown[] | undefined) => values?.includes(value) ?? false;
  return has(asked[by]) ? "asked" : has(asked[`not_${by}`]) ? "ruled_out" : "off";
}

/** What is asked once a value of one thing stands another way. It starts again from the first page. */
export function withStanding<By extends NarrowedBy>(
  asked: Asked,
  by: By,
  value: ValueOf[By],
  standing: Standing,
): Asked {
  const without = (values: readonly ValueOf[By][] | undefined) => {
    const rest = (values ?? []).filter((one) => one !== value);
    return rest.length ? rest : undefined;
  };
  const wanted = without(asked[by] as ValueOf[By][] | undefined);
  const unwanted = without(asked[`not_${by}`] as ValueOf[By][] | undefined);
  return {
    ...asked,
    page: undefined,
    [by]: standing === "asked" ? [...(wanted ?? []), value] : wanted,
    [`not_${by}`]: standing === "ruled_out" ? [...(unwanted ?? []), value] : unwanted,
  };
}

/** The standing a click on a value leads to: asked for, then ruled out, then neither. */
export function nextStanding(standing: Standing): Standing {
  return standing === "off" ? "asked" : standing === "asked" ? "ruled_out" : "off";
}

/** How many values the series are narrowed by. */
export function narrowedBy(asked: Asked): number {
  return NARROWED_BY.reduce(
    (count, by) => count + (asked[by]?.length ?? 0) + (asked[`not_${by}`]?.length ?? 0),
    0,
  );
}

/** What is asked with nothing to narrow by: the order stays. */
export function unnarrowed(asked: Asked): Asked {
  return { sort: asked.sort, reverse: asked.reverse, seed: asked.seed };
}

/** The ten years a year is one of, as the address names them: `1990s`. */
export function decadeOf(year: number): string {
  return `${Math.floor(year / 10) * 10}s`;
}

/** What is asked on another page of the same series, from 1. */
export function onPage(asked: Asked, page: number): Asked {
  return { ...asked, page: page > 1 ? page : undefined };
}

/** What the server is asked for a page of a library's series. */
export function seriesQuery(asked: Asked) {
  const { page = 1, ...rest } = asked;
  return { ...rest, offset: (page - 1) * SERIES_A_PAGE, limit: SERIES_A_PAGE };
}

/** What the server is asked for the letters of a library's series: the same narrowing. */
export function lettersQuery(asked: Asked) {
  const { page, sort, seed, ...rest } = asked;
  return rest;
}

/**
 * Whether a text alone under its name would be read back from an address
 * as something else: the router reads a lone value as JSON, so `1.0` comes
 * back as `1`, `null` as nothing and `"x"` without its quotes.
 */
function misread(text: string): boolean {
  try {
    const read: unknown = JSON.parse(text);
    return !(["number", "boolean"].includes(typeof read) && String(read) === text);
  } catch {
    return false;
  }
}

/**
 * Writes what a page keeps in its address. A list goes in as its values one
 * after another, each under the list's name, so that an address reads
 * `?publisher=Vertigo&year=1990s` and not as brackets and quotes. The rest
 * goes in as the router writes it, and so does a lone text the router
 * would not read back as itself.
 */
export function stringifySearch(search: Record<string, unknown>): string {
  const plain: Record<string, unknown> = {};
  const lists = new URLSearchParams();
  for (const [key, value] of Object.entries(search)) {
    if (Array.isArray(value) && value.every((one) => ["string", "number"].includes(typeof one))) {
      const alone = value.length === 1;
      for (const one of value) {
        const quoted = alone && typeof one === "string" && misread(one);
        lists.append(key, quoted ? JSON.stringify(one) : String(one));
      }
    } else {
      plain[key] = value;
    }
  }
  const first = defaultStringifySearch(plain);
  const rest = lists.toString();
  return rest ? `${first ? `${first}&` : "?"}${rest}` : first;
}
