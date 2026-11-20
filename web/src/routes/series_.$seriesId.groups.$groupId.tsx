import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";
import { useGroupBooks, useGroupOverview } from "../api/libraries";
import { BookGrid, shownIn } from "../components/BookGrid";
import { BooksHeader } from "../components/BooksHeader";
import { EmptyGroup } from "../components/NoBooks";
import { NotOnServer } from "../components/Notice";
import { BooksMenu } from "../components/ReadMarks";
import { ErrorText, upLinkClass } from "../components/text";
import { groupName } from "../format";

export const Route = createFileRoute("/series_/$seriesId/groups/$groupId")({
  params: {
    parse: ({ seriesId, groupId }) => ({ seriesId: Number(seriesId), groupId: Number(groupId) }),
    stringify: ({ seriesId, groupId }) => ({
      seriesId: String(seriesId),
      groupId: String(groupId),
    }),
  },
  validateSearch: shownIn,
  component: GroupPage,
});

/** The page of one group of a series, the same for every kind of group. */
function GroupPage() {
  const { groupId } = Route.useParams();
  const { show } = Route.useSearch();
  const overview = useGroupOverview(groupId);
  const books = useGroupBooks(groupId);

  if (overview.isPending) {
    return null;
  }
  if (overview.isError) {
    return (
      <NotOnServer title="This part of the series is not on the server">
        It may have been deleted, or its series may be gone.
      </NotOnServer>
    );
  }

  const { group, series } = overview.data;
  const name = groupName(group);
  return (
    <>
      <BooksHeader
        // A new header for each group: what was opened in one stays there.
        key={groupId}
        cover={group.cover}
        title={name}
        bookCount={group.book_count}
        overview={overview.data}
        libraryId={series.library_id}
        inGroup
        menu={
          <BooksMenu about={name} what={{ groups: [group.id] }} unreadCount={group.unread_count} />
        }
        up={
          <Link to="/series/$seriesId" params={{ seriesId: series.id }} className={upLinkClass}>
            <ChevronLeft className="size-4" aria-hidden="true" />
            {series.name}
          </Link>
        }
      />
      {group.book_count === 0 ? (
        <EmptyGroup group={group} />
      ) : books.isError ? (
        <ErrorText>{books.error.message}</ErrorText>
      ) : books.data ? (
        <BookGrid title="Books" titleHidden books={books.data} unread={show === "unread"} inGroup />
      ) : null}
    </>
  );
}
