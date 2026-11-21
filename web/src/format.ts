import type { Book, Group, Library, Person, ReadingList } from "./api/client";

/** "1 file", "1,240 files". */
export function count(amount: number, one: string, many: string): string {
  return `${amount.toLocaleString()} ${amount === 1 ? one : many}`;
}

export function books(amount: number): string {
  return count(amount, "book", "books");
}

export function pages(amount: number): string {
  return count(amount, "page", "pages");
}

/** "12 series · 340 books", or "340 books" before a scan has made any series. */
export function contents(library: Library): string {
  const all = books(library.book_count);
  return library.series_count ? `${seriesCount(library.series_count)} · ${all}` : all;
}

/** What a person did, in a line: "Writer of 40 books · Penciller of 3 books · 6 series". */
export function workOf(person: Person): string {
  const parts = [
    person.written_count ? `Writer of ${books(person.written_count)}` : null,
    person.pencilled_count ? `Penciller of ${books(person.pencilled_count)}` : null,
    count(person.series_count, "series", "series"),
  ];
  return parts.filter(Boolean).join(" · ");
}

const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/** A moment given in seconds since 1970, in the reader's own format. */
export function moment(seconds: number): string {
  return dateTime.format(new Date(seconds * 1000));
}

const date = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

/** The day of a moment given in seconds since 1970, in the reader's own format. */
export function day(seconds: number): string {
  return date.format(new Date(seconds * 1000));
}

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

/** How long ago a moment given in seconds since 1970 was: "yesterday", "3 months ago", "2 years ago". */
export function ago(seconds: number): string {
  const days = Math.round((Date.now() / 1000 - seconds) / (24 * 60 * 60));
  if (days < 60) {
    return relative.format(-days, "day");
  }
  if (days < 730) {
    return relative.format(-Math.round(days / 30.44), "month");
  }
  return relative.format(-Math.round(days / 365.25), "year");
}

/**
 * What tells one build of the server from another of the same version: when
 * it was built and from which commit. `null` when the server does not say.
 */
export function build(server: { built_at?: number | null; commit?: string | null }): string | null {
  const parts = [server.built_at == null ? null : moment(server.built_at), server.commit];
  return parts.filter(Boolean).join(" · ") || null;
}

/** What each kind of group is called with no name of its own: by itself, and with its number. */
const GROUP_NAMES: Record<Group["kind"], { plain: string; numbered: (number: number) => string }> =
  {
    volume: { plain: "Volume", numbered: (number) => `Volume ${number}` },
    limited: { plain: "Limited series", numbered: (number) => `Limited series ${number}` },
    annual: { plain: "Annuals", numbered: (number) => `Volume ${number} annuals` },
    collected: {
      plain: "Collected editions",
      numbered: (number) => `Collected editions ${number}`,
    },
    one_shot: { plain: "One-shots", numbered: (number) => `One-shots ${number}` },
    appearance: { plain: "Appearances", numbered: (number) => `Appearances ${number}` },
  };

/**
 * What a group of a series is called: its own name when it has one, else
 * what its kind is called. Its number is said too when it has one. "Year
 * One", "Volume 2", "Appearances", "Volume 2: Rebirth".
 */
export function groupName(group: Group): string {
  const names = GROUP_NAMES[group.kind];
  if (group.number == null) {
    return group.name ?? names.plain;
  }
  const numbered = names.numbered(group.number);
  return group.name == null ? numbered : `${numbered}: ${group.name}`;
}

/** Whether two names are one, whatever their capitals. */
export function same(one: string, other: string | null | undefined): boolean {
  return one.toLowerCase() === other?.toLowerCase();
}

/**
 * The series and the number a book goes by, when it has a number where it
 * sits. The number is as comics give it: "#12". One that is not a number
 * stays as written: "Annual 1". A book of the annuals says so, since its
 * number counts the annuals and not the issues: "Annual #1".
 *
 * Every way of naming a book starts here, so that they all agree on which
 * books go by number. A file that could not be read sits nowhere.
 */
function numbered(book: Book): { series: string; number: string } | null {
  const { number, series_name: series } = book;
  if (!number || !series) {
    return null;
  }
  if (!/^\d/.test(number)) {
    return { series, number };
  }
  return { series, number: book.group_kind === "annual" ? `Annual #${number}` : `#${number}` };
}

/** A book's issue number as comics give it, when it goes by one. */
export function issueOf(book: Book): string | null {
  return numbered(book)?.number ?? null;
}

/** A book's own title, unless it only repeats the book's name. */
export function titleOf(book: Book): string | null {
  const title = book.info?.title;
  return title && title !== book.name ? title : null;
}

/**
 * The title to say with a book that goes by its number: its own, unless it
 * only repeats the series' name. Nothing for a book that goes by its title.
 */
export function titleUnder(book: Book): string | null {
  const by = numbered(book);
  if (!by) {
    return null;
  }
  const title = titleOf(book);
  return title && !same(title, by.series) ? title : null;
}

/** A name without what is in brackets, the ones inside other brackets first. */
function unbracketed(name: string): string {
  const without = name.replace(/\s*[([][^()[\]]*[)\]]/g, "");
  return without === name ? name : unbracketed(without);
}

/**
 * A file's name as a reader would say it: without what is in brackets, and
 * with the number it ends in said plainly, without the zeros that pad it or
 * the `SP05` some libraries put after it to mark a special. "Superman 020
 * SP05 (2019) (Digital)" is "Superman 20", and "Akira v01" is "Akira v1".
 *
 * A marker that follows no number stays, since it is all that tells such
 * files apart: "Odd Squad v2 SP01". This is only for showing a name. What a
 * file's name means for where the book sits is the server's to read.
 */
function plainName(name: string): string {
  const plain = unbracketed(name.replace(/_/g, " "))
    .trim()
    .replace(/(\s)0*(\d+)(\s+sp\d+)?$/i, "$1$2")
    // A volume it ends in, which no marker follows.
    .replace(/(\sv)0+(\d+)$/i, "$1$2");
  return plain || name;
}

/**
 * What a book is called by name: its own title, or else its file's name as
 * a reader would say it.
 */
export function nameOf(book: Book): string {
  return titleOf(book) ?? plainName(book.name);
}

/**
 * What a book is called wherever its series is not already said: by its
 * series and number as comics are, "The Flash #12". One with no number goes
 * by its own title, or else its file's name as a reader would say it. A file
 * that could not be read is in no series and keeps its file's name, which is
 * what there is to look for.
 */
export function called(book: Book): string {
  if (book.problem) {
    return book.name;
  }
  const by = numbered(book);
  return by ? `${by.series} ${by.number}` : nameOf(book);
}

/** The year a book came out: where it sits says, or else its file. */
export function yearOf(book: Book): string | null {
  const year = book.published?.slice(0, 4) ?? book.info?.year;
  return year ? String(year) : null;
}

/** Where a reader is in a book: "Page 12 of 40". `page` counts from 0. */
export function pageOf(page: number, book: Book): string {
  return `Page ${(page + 1).toLocaleString()} of ${book.page_count.toLocaleString()}`;
}

/** When a book came out, as exactly as its file says: "1988", "March 1988", "March 12, 1988". */
export function published(info: NonNullable<Book["info"]>): string | null {
  const { year, month, day } = info;
  if (!year) {
    return null;
  }
  if (!month) {
    return String(year);
  }
  // Set this way because a year under 100 given to `Date.UTC` means 19xx.
  const date = new Date(0);
  date.setUTCFullYear(year, month - 1, day ?? 1);
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "long",
    day: day ? "numeric" : undefined,
    timeZone: "UTC",
  }).format(date);
}

const namesTogether = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** What a person makes and shares: a reading list, a collection. */
export type Made = Pick<
  ReadingList,
  "name" | "mine" | "made_by" | "shared_with_all" | "shared_with"
>;

/**
 * Who sees a reading list or a collection, as its page tells the one whose
 * it is: "Only you", "Shared with Ana and Luis", "Shared with everyone".
 */
export function sharedWith(made: Made): string {
  if (made.shared_with_all) {
    return "Shared with everyone";
  }
  if (made.shared_with.length === 0) {
    return "Only you";
  }
  return `Shared with ${namesTogether.format(made.shared_with.map((person) => person.name))}`;
}

/** Whether anyone but the one who made it sees a reading list or a collection. */
export function isShared(made: Pick<Made, "shared_with_all" | "shared_with">): boolean {
  return made.shared_with_all || made.shared_with.length > 0;
}

/** Whose a reading list or a collection is, for someone else's: "By Ana". Nothing for one's own. */
export function madeBy(made: Made): string | null {
  return made.mine ? null : `By ${made.made_by.name}`;
}

/** "12 series". */
export function seriesCount(amount: number): string {
  return count(amount, "series", "series");
}

/** The years a series ran: "1968", or "1968–1988". */
export function years(first: number | null | undefined, last: number | null | undefined) {
  if (!first || !last) {
    return null;
  }
  return first === last ? String(first) : `${first}–${last}`;
}

/** Seconds a page of a comic takes to read, near enough to say how long a series is. */
const SECONDS_A_PAGE = 20;

/** How long that many pages take to read: "about 35 minutes", "about 13 hours". */
export function readingTime(pageCount: number): string | null {
  return about(pageCount * SECONDS_A_PAGE);
}

/** A time to come, from seconds, said roundly: "about 35 minutes", "about 13 hours". Nothing under a minute. */
export function about(seconds: number): string | null {
  const minutes = seconds / 60;
  if (minutes < 1) {
    return null;
  }
  if (minutes < 50) {
    return `about ${count(Math.max(5, Math.round(minutes / 5) * 5), "minute", "minutes")}`;
  }
  return `about ${count(Math.round(minutes / 60), "hour", "hours")}`;
}

/** Time spent, from seconds: "0 min", "under a minute", "45 min", "3 h 20 min", "12 h". */
export function timeSpent(seconds: number): string {
  if (seconds <= 0) {
    return "0 min";
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 1) {
    return "under a minute";
  }
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) {
    return `${minutes} min`;
  }
  return rest === 0 || hours >= 10 ? `${hours.toLocaleString()} h` : `${hours} h ${rest} min`;
}

/** A language by its name, from a code such as `en`. The code itself when it is not known. */
export function language(code: string): string {
  try {
    return new Intl.DisplayNames(undefined, { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** A file's size: "640 kB", "34.5 MB", "1.2 GB". */
export function size(bytes: number): string {
  const units = ["kilobyte", "megabyte", "gigabyte"];
  let amount = bytes / 1000;
  let unit = 0;
  // Rounded first, so 999,950 bytes is 1 MB and not 1,000 kB.
  while (Math.round(amount) >= 1000 && unit < units.length - 1) {
    amount /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(undefined, {
    style: "unit",
    unit: units[unit],
    maximumFractionDigits: amount < 100 ? 1 : 0,
  }).format(amount);
}
