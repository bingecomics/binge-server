import { createFileRoute, Link } from "@tanstack/react-router";
import { BookMarked, BookmarkX, Info } from "lucide-react";
import { type KeptPage, smallPageUrl } from "../api/client";
import { KEPT_A_PAGE, useKeepPage, useKeptPages } from "../api/kept";
import { Card } from "../components/Card";
import { coverGridClass } from "../components/Cover";
import { MenuItem, MenuLink, MoreMenu } from "../components/Menu";
import { Notice } from "../components/Notice";
import { Pages, pageAsked } from "../components/Pages";
import { shelfTitleClass } from "../components/Shelf";
import { ErrorText, quietLinkClass } from "../components/text";
import { called, count, day, issueOf, nameOf } from "../format";

type Asked = { page?: number };

export const Route = createFileRoute("/kept")({
  validateSearch: (search: Record<string, unknown>): Asked => ({ page: pageAsked(search.page) }),
  component: KeptPagesPage,
});

/** The pages of one series, or of the books in no series, together under its name. */
type OfSeries = { id: number | null; name: string; pages: KeptPage[] };

/** Pages in the server's order, which has those of a series together, as one lot for each series. */
function bySeries(pages: KeptPage[]): OfSeries[] {
  const lots: OfSeries[] = [];
  for (const kept of pages) {
    const id = kept.book.series_id ?? null;
    const last = lots[lots.length - 1];
    if (last && last.id === id) {
      last.pages.push(kept);
    } else {
      lots.push({ id, name: kept.book.series_name ?? "In no series", pages: [kept] });
    }
  }
  return lots;
}

/**
 * A kept page as a card: the page itself, the book it is from as it is
 * called under its series, and its number. It opens the book on that page
 * for a look, which moves no progress.
 */
function KeptCard({ kept }: { kept: KeptPage }) {
  const { book, page } = kept;
  const { mutate: keepPage } = useKeepPage();
  const identity = book.identity;
  if (!identity) {
    return null;
  }
  const name = `${issueOf(book) ?? nameOf(book)}, page ${(page + 1).toLocaleString()}`;
  return (
    <Card
      what={{ kind: "books", id: book.id }}
      pickable={false}
      name={name}
      about={`${called(book)}, page ${(page + 1).toLocaleString()}`}
      link={{
        to: "/read/$bookId",
        params: { bookId: book.id },
        search: { page: page + 1, unrecorded: true },
      }}
      picture={smallPageUrl(identity, page)}
      standing={<p className="text-caption text-fg-muted">Kept on {day(kept.kept_at)}</p>}
      menu={
        <MoreMenu corner about={name}>
          <MenuLink to="/books/$bookId" params={{ bookId: book.id }} icon={Info}>
            Details of the book
          </MenuLink>
          <MenuItem
            icon={BookmarkX}
            onSelect={() => keepPage({ bookId: book.id, identity, number: page, keep: false })}
          >
            Let this page go
          </MenuItem>
        </MoreMenu>
      }
    />
  );
}

/**
 * The pages the reader kept while reading, series by series. A page is
 * kept with the mark in the bar at the top of the reader.
 */
function KeptPagesPage() {
  const page = Route.useSearch().page ?? 1;
  const kept = useKeptPages(page);

  return (
    <>
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h1 className="font-bold text-screen-title tracking-tight">Kept pages</h1>
        {kept.data?.total ? (
          <p className="text-fg-muted text-metadata">{count(kept.data.total, "page", "pages")}</p>
        ) : null}
      </div>

      {kept.isError ? (
        <ErrorText className="mt-6">{kept.error.message}</ErrorText>
      ) : !kept.data ? null : kept.data.total === 0 ? (
        <div className="mt-8">
          <Notice icon={BookMarked} title="No page kept yet">
            While you read, the mark in the bar at the top of the reader keeps the page on screen.
            Every page you keep is here, with the book it is from.
          </Notice>
        </div>
      ) : (
        <>
          {bySeries(kept.data.pages).map((series) => (
            <section
              key={series.id ?? "none"}
              className="mt-10"
              aria-labelledby={`kept-${series.id ?? "none"}`}
            >
              <h2 id={`kept-${series.id ?? "none"}`} className={shelfTitleClass}>
                {series.id != null ? (
                  <Link
                    to="/series/$seriesId"
                    params={{ seriesId: series.id }}
                    className={quietLinkClass}
                  >
                    {series.name}
                  </Link>
                ) : (
                  series.name
                )}
              </h2>
              <ul className={`mt-4 ${coverGridClass}`}>
                {series.pages.map((one) => (
                  <KeptCard key={`${one.book.id}-${one.page}`} kept={one} />
                ))}
              </ul>
            </section>
          ))}
          <Pages
            label="Pages of kept pages"
            page={page}
            total={kept.data.total}
            aPage={KEPT_A_PAGE}
            to={(page) => ({ to: "/kept", search: { page: page > 1 ? page : undefined } })}
          />
        </>
      )}
    </>
  );
}
