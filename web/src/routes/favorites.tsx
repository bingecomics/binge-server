import { createFileRoute, Link } from "@tanstack/react-router";
import { Info } from "lucide-react";
import { type FavoritePage, smallPageUrl } from "../api/client";
import { FAVORITES_A_PAGE, useFavoritePages, useMarkFavorite } from "../api/favorites";
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
type OfSeries = { id: number | null; name: string; pages: FavoritePage[] };

/** Pages in the server's order, which has those of a series together, as one lot for each series. */
function bySeries(pages: FavoritePage[]): OfSeries[] {
  const lots: OfSeries[] = [];
  for (const favorite of pages) {
    const id = favorite.book.series_id ?? null;
    const last = lots[lots.length - 1];
    if (last && last.id === id) {
      last.pages.push(favorite);
    } else {
      lots.push({ id, name: favorite.book.series_name ?? "In no series", pages: [favorite] });
    }
  }
  return lots;
}

/**
 * A favorite page as a card: the page itself, the book it is from as it is
 * called under its series, and its number. It opens the book on that page
 * for a look, which moves no progress.
 */
function FavoriteCard({ favorite }: { favorite: FavoritePage }) {
  const { book, page } = favorite;
  const { mutate: markFavorite } = useMarkFavorite();
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
      standing={
        <p className="text-caption text-fg-muted">A favorite since {day(favorite.added_at)}</p>
      }
      menu={
        <MoreMenu corner about={name}>
          <MenuLink to="/books/$bookId" params={{ bookId: book.id }} icon={Info}>
            Details of the book
          </MenuLink>
          <MenuItem
            icon={NoFavoriteIcon}
            onSelect={() =>
              markFavorite({ bookId: book.id, identity, number: page, favorite: false })
            }
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
 * page is made one with the heart in the bar at the top of the reader.
 */
function FavoritesPage() {
  const page = Route.useSearch().page ?? 1;
  const favorites = useFavoritePages(page);

  return (
    <>
      <MyBingeParts />
      <h1 className="sr-only">Favorites</h1>
      {favorites.data?.total ? (
        <p className="text-fg-muted text-metadata">
          {count(favorites.data.total, "favorite page", "favorite pages")}
        </p>
      ) : null}

      {favorites.isError ? (
        <ErrorText className="mt-6">{favorites.error.message}</ErrorText>
      ) : !favorites.data ? null : favorites.data.total === 0 ? (
        <div className="mt-8">
          <Notice icon={FavoriteIcon} title="No favorites yet">
            While you read, the heart in the bar at the top of the reader makes the page on screen a
            favorite. Every favorite page is here, with the book it is from.
          </Notice>
        </div>
      ) : (
        <>
          {bySeries(favorites.data.pages).map((series) => (
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
                  <FavoriteCard key={`${one.book.id}-${one.page}`} favorite={one} />
                ))}
              </ul>
            </section>
          ))}
          <Pages
            label="Pages of favorites"
            page={page}
            total={favorites.data.total}
            aPage={FAVORITES_A_PAGE}
            to={(page) => ({ to: "/favorites", search: { page: page > 1 ? page : undefined } })}
          />
        </>
      )}
    </>
  );
}
