import { createFileRoute, Link } from "@tanstack/react-router";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import type { ReadingYear } from "../api/client";
import { useReadingYear } from "../api/history";
import { buttonClass } from "../components/Button";
import { Notice } from "../components/Notice";
import { rowClass, rowSidesClass } from "../components/Section";
import { shelfTitleClass } from "../components/Shelf";
import { ErrorText, quietLinkClass } from "../components/text";
import { books, count, pages, timeSpent } from "../format";

type Asked = { year?: number };

export const Route = createFileRoute("/history")({
  // The year looked back on. Left out for this one.
  validateSearch: (search: Record<string, unknown>): Asked => {
    const year = Number(search.year);
    return { year: Number.isSafeInteger(year) && year >= 1970 && year <= 9999 ? year : undefined };
  },
  component: HistoryPage,
});

/** One number that sums a year up, with what it counts under it. */
function Sum({ amount, of, className = "" }: { amount: string; of: string; className?: string }) {
  return (
    <div className={`rounded-card border border-divider bg-surface px-5 py-4 ${className}`}>
      <dd className="font-bold text-screen-title tracking-tight">{amount}</dd>
      <dt className="text-fg-muted text-metadata">{of}</dt>
    </div>
  );
}

/** How much ink a day gets, from none to the most: one colour, fuller for more pages. */
const INK = ["bg-raised", "bg-accent/25", "bg-accent/50", "bg-accent/75", "bg-accent"] as const;

const dayName = new Intl.DateTimeFormat(undefined, {
  weekday: "long",
  month: "long",
  day: "numeric",
  timeZone: "UTC",
});
const monthName = new Intl.DateTimeFormat(undefined, { month: "short", timeZone: "UTC" });
const longMonth = new Intl.DateTimeFormat(undefined, { month: "long", timeZone: "UTC" });
const monthLetter = new Intl.DateTimeFormat(undefined, { month: "narrow", timeZone: "UTC" });

/** What was read on a day or in a month, as a sentence under the pointer. */
function said(
  when: string,
  reading: { books: number; pages: number; seconds: number } | undefined,
) {
  return reading
    ? `${when}: ${pages(reading.pages)} of ${books(reading.books)}, ${timeSpent(reading.seconds)}`
    : `${when}: nothing read`;
}

/** A day of a year as the server names it: `2026-03-05`. */
function named(day: Date): string {
  return day.toISOString().slice(0, 10);
}

/**
 * The year as a calendar: a square for every day, a column for every week
 * from Monday down, fuller the more pages were read that day. What a
 * square stands for is said under the pointer and read out.
 */
function Calendar({ year }: { year: ReadingYear }) {
  const read = new Map(year.days.map((day) => [day.day, day]));
  const most = Math.max(1, ...year.days.map((day) => day.pages));
  const first = new Date(Date.UTC(year.year, 0, 1));
  // How many squares of the first week come before the first of January.
  const before = (first.getUTCDay() + 6) % 7;
  const days: Date[] = [];
  for (let day = new Date(first); day.getUTCFullYear() === year.year; ) {
    days.push(new Date(day));
    day.setUTCDate(day.getUTCDate() + 1);
  }
  const weeks = Math.ceil((before + days.length) / 7);
  const columns = { gridTemplateColumns: `repeat(${weeks}, 0.75rem)` };
  // Where the year is too wide for the window, it opens on today.
  const now = new Date();
  const today = named(new Date(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())));
  const box = useRef<HTMLDivElement>(null);
  const todaySquare = useRef<HTMLLIElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the year shown decides where today is
  useEffect(() => {
    if (box.current && todaySquare.current) {
      box.current.scrollLeft = todaySquare.current.offsetLeft - box.current.clientWidth / 2;
    }
  }, [year.year]);
  return (
    <div ref={box} className="relative overflow-x-auto pb-2">
      <div className="w-max">
        <div className="grid gap-1 text-caption text-fg-muted" style={columns} aria-hidden="true">
          {days
            .filter((day) => day.getUTCDate() === 1)
            .map((day) => (
              <span
                key={day.getUTCMonth()}
                // The column of the week the month starts in.
                style={{
                  gridColumnStart: Math.floor((before + days.indexOf(day)) / 7) + 1,
                  gridRowStart: 1,
                }}
              >
                {monthName.format(day)}
              </span>
            ))}
        </div>
        <ul
          aria-label={`Days of ${year.year} with reading`}
          className="mt-1 grid grid-flow-col grid-rows-[repeat(7,0.75rem)] gap-1"
          style={columns}
        >
          {days.map((day, place) => {
            const reading = read.get(named(day));
            const ink = reading ? Math.max(1, Math.ceil((reading.pages / most) * 4)) : 0;
            const told = said(dayName.format(day), reading);
            return (
              <li
                key={named(day)}
                ref={named(day) === today ? todaySquare : undefined}
                title={told}
                // A day without reading is only there to be seen.
                aria-hidden={reading ? undefined : "true"}
                style={place === 0 ? { gridRowStart: before + 1 } : undefined}
                className={`size-3 rounded-[3px] ${INK[ink]}`}
              >
                {reading ? <span className="sr-only">{told}</span> : null}
              </li>
            );
          })}
        </ul>
        <p className="mt-2 flex items-center justify-end gap-1 text-caption text-fg-muted">
          Fewer pages
          {INK.map((ink) => (
            <span key={ink} className={`size-3 rounded-[3px] ${ink}`} aria-hidden="true" />
          ))}
          More
        </p>
      </div>
    </div>
  );
}

/**
 * The pages of each month as bars, the fullest month the tallest. The
 * numbers are under the pointer, and in a table for whoever has it read.
 */
function Months({ year }: { year: ReadingYear }) {
  const read = new Map(year.months.map((month) => [month.month, month]));
  const most = Math.max(1, ...year.months.map((month) => month.pages));
  const months = Array.from({ length: 12 }, (_, month) => ({
    date: new Date(Date.UTC(year.year, month, 1)),
    reading: read.get(month + 1),
  }));
  return (
    <>
      <div className="flex h-40 max-w-2xl items-end gap-2" aria-hidden="true">
        {months.map(({ date, reading }) => (
          <div
            key={date.getUTCMonth()}
            className="flex h-full min-w-0 flex-1 flex-col justify-end gap-1"
            title={said(longMonth.format(date), reading)}
          >
            {/* Only the fullest month says its number: the rest are read against it. */}
            <span className="text-center text-caption text-fg-muted">
              {reading && reading.pages === most ? reading.pages.toLocaleString() : " "}
            </span>
            <div
              className={`rounded-t-[4px] ${reading ? "bg-accent" : "bg-raised"}`}
              // A month with nothing keeps a line, so the year reads whole.
              style={{ height: reading ? `${Math.max(3, (reading.pages / most) * 80)}%` : "2px" }}
            />
            {/* A narrow window has room for a letter under each bar. */}
            <span className="text-center text-caption text-fg-muted">
              <span className="sm:hidden">{monthLetter.format(date)}</span>
              <span className="hidden sm:inline">{monthName.format(date)}</span>
            </span>
          </div>
        ))}
      </div>
      <table className="sr-only">
        <caption>What was read each month of {year.year}</caption>
        <thead>
          <tr>
            <th scope="col">Month</th>
            <th scope="col">Books</th>
            <th scope="col">Pages</th>
            <th scope="col">Time</th>
          </tr>
        </thead>
        <tbody>
          {months.map(({ date, reading }) => (
            <tr key={date.getUTCMonth()}>
              <th scope="row">{longMonth.format(date)}</th>
              <td>{reading?.books ?? 0}</td>
              <td>{reading?.pages ?? 0}</td>
              <td>{timeSpent(reading?.seconds ?? 0)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

/** A way to another year, or the same thing greyed when there is none that way. */
function Turn({
  year,
  label,
  children,
}: {
  year: number | null;
  label: string;
  children: ReactNode;
}) {
  const className = `${buttonClass()} w-11 px-0`;
  if (year === null) {
    return (
      <span aria-disabled="true" className={`${className} opacity-40`}>
        {children}
      </span>
    );
  }
  return (
    <Link to="/history" search={{ year }} aria-label={label} title={label} className={className}>
      {children}
    </Link>
  );
}

/**
 * What the reader read, a year at a time: the days they read on, the pages
 * of each month and the writers they read most. It is told from the pages
 * turned in the reader, so a book marked read adds nothing to it.
 */
function HistoryPage() {
  const thisYear = new Date().getFullYear();
  const asked = Route.useSearch().year ?? thisYear;
  const reading = useReadingYear(asked);
  const year = reading.data;
  const earliest = year?.first_year ?? thisYear;

  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <h1 className="font-bold text-screen-title tracking-tight">What you read</h1>
        <nav aria-label="Years" className="flex items-center gap-2">
          <Turn year={asked > earliest ? asked - 1 : null} label={`${asked - 1}`}>
            <ChevronLeft className="size-5" aria-hidden="true" />
          </Turn>
          <p className="min-w-16 text-center font-semibold" aria-current="page">
            {asked}
          </p>
          <Turn year={asked < thisYear ? asked + 1 : null} label={`${asked + 1}`}>
            <ChevronRight className="size-5" aria-hidden="true" />
          </Turn>
        </nav>
      </div>

      {reading.isError ? (
        <ErrorText className="mt-6">{reading.error.message}</ErrorText>
      ) : !year ? null : year.days.length === 0 ? (
        <div className="mt-8">
          <Notice icon={CalendarDays} title={`Nothing read in ${asked}`}>
            This page fills as you read: every day you turn pages here is a day on it. A book only
            marked as read adds nothing.
          </Notice>
        </div>
      ) : (
        <>
          <dl className="mt-6 grid max-w-2xl grid-cols-2 gap-4 sm:grid-cols-3">
            <Sum amount={year.books.toLocaleString()} of={year.books === 1 ? "book" : "books"} />
            <Sum amount={year.pages.toLocaleString()} of={year.pages === 1 ? "page" : "pages"} />
            <Sum
              amount={timeSpent(year.seconds)}
              of="reading"
              // A time is longer than a count, so a narrow window gives it a line.
              className="col-span-2 sm:col-span-1"
            />
          </dl>

          <section className="mt-12" aria-labelledby="reading-days">
            <h2 id="reading-days" className={shelfTitleClass}>
              {count(year.days.length, "day", "days")} with reading
            </h2>
            <div className="mt-4">
              <Calendar year={year} />
            </div>
          </section>

          <section className="mt-12" aria-labelledby="reading-months">
            <h2 id="reading-months" className={shelfTitleClass}>
              Pages by month
            </h2>
            <div className="mt-4">
              <Months year={year} />
            </div>
          </section>

          {year.writers.length > 0 ? (
            <section className="mt-12 max-w-2xl" aria-labelledby="reading-writers">
              <h2 id="reading-writers" className={shelfTitleClass}>
                Writers you read most
              </h2>
              <ol className="mt-2">
                {year.writers.map((writer) => (
                  <li key={writer.id} className={`${rowClass} ${rowSidesClass}`}>
                    <Link
                      to="/people/$personId"
                      params={{ personId: writer.id }}
                      className={`min-w-0 truncate ${quietLinkClass}`}
                    >
                      {writer.name}
                    </Link>
                    <span className="shrink-0 text-fg-muted text-metadata">
                      {books(writer.books)} · {pages(writer.pages)}
                    </span>
                  </li>
                ))}
              </ol>
            </section>
          ) : null}
        </>
      )}
    </>
  );
}
