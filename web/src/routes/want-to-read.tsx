import { createFileRoute, Link } from "@tanstack/react-router";
import { gotNoAnswer } from "../api/client";
import { useWantToRead, WANTED_A_PAGE } from "../api/wanted";
import { coverGridClass } from "../components/Cover";
import { MyBingeParts } from "../components/MyBinge";
import { Notice, ServerNotAnswering } from "../components/Notice";
import { Pages, pageAsked } from "../components/Pages";
import { SeriesCard } from "../components/SeriesCard";
import { ErrorText, textLinkClass } from "../components/text";
import { WantToReadIcon } from "../components/WantToRead";
import { seriesCount } from "../format";

type Asked = { page?: number };

export const Route = createFileRoute("/want-to-read")({
  validateSearch: (search: Record<string, unknown>): Asked => ({ page: pageAsked(search.page) }),
  component: WantToReadPage,
});

/**
 * The series a person wants to read, the ones put there last first. It is
 * theirs alone: there is nothing to share. A series leaves it from its
 * menu, and is left out while all of it is read.
 */
function WantToReadPage() {
  const page = Route.useSearch().page ?? 1;
  const wanted = useWantToRead(page);

  // The choices of My Binge stay while the page comes, and when it does not.
  if (wanted.isPending) {
    return <MyBingeParts />;
  }
  if (wanted.isError) {
    return (
      <>
        <MyBingeParts />
        {gotNoAnswer(wanted.error) ? (
          <ServerNotAnswering />
        ) : (
          <ErrorText>{wanted.error.message}</ErrorText>
        )}
      </>
    );
  }

  const { series, total } = wanted.data;
  return (
    <>
      <MyBingeParts />
      <h1 className="sr-only">Want to read</h1>
      <p className="text-fg-muted text-metadata">{seriesCount(total)}</p>

      {total === 0 ? (
        <div className="mt-8">
          <Notice icon={WantToReadIcon} title="Nothing here yet">
            Put a series here from its menu, or select several in a library and add them together.
            Only you see what you want to read. A series you have read to the end is left out, and
            comes back when it has a new book.
          </Notice>
        </div>
      ) : series.length === 0 ? (
        // An address kept from when there were more, or the last of a page taken off.
        <p className="mt-6 text-fg-muted">
          There is no page {page.toLocaleString()}.{" "}
          <Link to="/want-to-read" className={textLinkClass}>
            Go to the first page
          </Link>
        </p>
      ) : (
        <>
          <ul className={`${coverGridClass} mt-6`}>
            {series.map((one) => (
              <SeriesCard key={one.id} series={one} />
            ))}
          </ul>
          <Pages
            label="Pages of want to read"
            page={page}
            total={total}
            aPage={WANTED_A_PAGE}
            to={(page) => ({ to: "/want-to-read", search: { page: page > 1 ? page : undefined } })}
          />
        </>
      )}
    </>
  );
}
