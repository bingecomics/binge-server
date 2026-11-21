import { createFileRoute, Link } from "@tanstack/react-router";
import { Info } from "lucide-react";
import { type KeptPage, smallPageUrl } from "../api/client";
import { KEPT_A_PAGE, useKeepPage, useKeptPages } from "../api/kept";
import { Card } from "../components/Card";
import { coverGridClass } from "../components/Cover";
import { MenuItem, MenuLink, MoreMenu } from "../components/Menu";
import { FavoriteIcon, MyBingeParts, NoFavoriteIcon } from "../components/MyBinge";
import { Notice } from "../components/Notice";
import { Pages, pageAsked } from "../components/Pages";
import { shelfTitleClass } from "../components/Shelf";
import { ErrorText, quietLinkClass } from "../components/text";
import { called, count, day, issueOf, nameOf } from "../format";

type Asked = { page?: number };

export const Route = createFileRoute("/favorites")({
  validateSearch: (search: Record<string, unknown>): Asked => ({ page: pageAsked(search.page) }),
  component: FavoritesPage,
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
 * A favorite page as a card: the page itself, the book it is from as it is
 * called under its series, and its number. It opens the book on that page
 * for a look, which moves no progress.
 */
function FavoriteCard({ kept }: { kept: KeptPage }) {
  const { book, page } = kept;
  const { mutate: keepPage } = useKeepPage();
  const identity = book.identity;
  if (!identity) {
    return null;
  }
  const name = `${issueOf(book) ?? nameOf(book)}, page ${(page + 1).toLocaleString()}`;
  return (
    <Card
      name={name}
      about={`${called(book)}, page ${(page + 1).toLocaleString()}`}
      link={{
        to: "/read/$bookId",
        params: { bookId: book.id },
        search: { page: page + 1, unrecorded: true },
      }}
      picture={smallPageUrl(identity, page)}
      standing={<p className="text-caption text-fg-muted">A favorite since {day(kept.kept_at)}</p>}
      menu={
        <MoreMenu corner about={name}>
          <MenuLink to="/books/$bookId" params={{ bookId: book.id }} icon={Info}>
            Details of the book
          </MenuLink>
          <MenuItem
            icon={NoFavoriteIcon}
            onSelect={() => keepPage({ bookId: book.id, identity, number: page, keep: false })}
          >
            Remove from favorites
          </MenuItem>
        </MoreMenu>
      }
    />
  );
}

/**
 * The pages the reader made favorites while reading, series by series. A
 * page is made one with the heart in the bar at the top of the reader. The
 * server calls them kept pages.
 */
function FavoritesPage() {
  const page = Route.useSearch().page ?? 1;
  const kept = useKeptPages(page);

  return (
    <>
      <MyBingeParts />
      <h1 className="sr-only">Favorites</h1>
      {kept.data?.total ? (
        <p className="text-fg-muted text-metadata">
          {count(kept.data.total, "favorite page", "favorite pages")}
        </p>
      ) : null}

      {kept.isError ? (
        <ErrorText className="mt-6">{kept.error.message}</ErrorText>
      ) : !kept.data ? null : kept.data.total === 0 ? (
        <div className="mt-8">
          <Notice icon={FavoriteIcon} title="No favorites yet">
            While you read, the heart in the bar at the top of the reader makes the page on screen a
            favorite. Every favorite page is here, with the book it is from.
          </Notice>
        </div>
      ) : (
        <>
          {bySeries(kept.data.pages).map((series) => (
            <section
              key={series.id ?? "none"}
              className="mt-10"
              aria-labelledby={`favorites-${series.id ?? "none"}`}
            >
              <h2 id={`favorites-${series.id ?? "none"}`} className={shelfTitleClass}>
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
                  <FavoriteCard key={`${one.book.id}-${one.page}`} kept={one} />
                ))}
              </ul>
            </section>
          ))}
          <Pages
            label="Pages of favorites"
            page={page}
            total={kept.data.total}
            aPage={KEPT_A_PAGE}
            to={(page) => ({ to: "/favorites", search: { page: page > 1 ? page : undefined } })}
          />
        </>
      )}
    </>
  );
}
