import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ChevronLeft, House, Pencil, Trash2, Users, X } from "lucide-react";
import { memo, useState } from "react";
import { useOthers } from "../api/account";
import { gotNoAnswer, type Series } from "../api/client";
import {
  useChangeCollection,
  useCollection,
  useCollectionSeries,
  useDeleteCollection,
  usePutCollectionOnHome,
  useRemoveFromCollection,
  useShareCollection,
} from "../api/collections";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { coverGridClass } from "../components/Cover";
import { madeAndShared } from "../components/FromFiles";
import { MenuItem, MoreMenu } from "../components/Menu";
import { NameAndSummaryDialog } from "../components/NameAndSummary";
import { NotOnServer, ServerNotAnswering } from "../components/Notice";
import { BooksMenu } from "../components/ReadMarks";
import { SeriesCard } from "../components/SeriesCard";
import { ShareDialog } from "../components/ShareDialog";
import { ErrorText, upLinkClass } from "../components/text";
import { books as bookCount, seriesCount } from "../format";

export const Route = createFileRoute("/collections/$collectionId")({
  params: {
    parse: ({ collectionId }) => ({ collectionId: Number(collectionId) }),
    stringify: ({ collectionId }) => ({ collectionId: String(collectionId) }),
  },
  component: CollectionPage,
});

/**
 * The series of a collection as cards. The one whose collection it is takes
 * a series out from its card's menu. Apart from the page, so that a dialog
 * opened on the page does not draw every card again.
 */
const SeriesOf = memo(function SeriesOf({
  collectionId,
  series,
  mine,
}: {
  collectionId: number;
  series: Series[];
  mine: boolean;
}) {
  // What goes wrong taking one out is said over the page by the hook.
  const take = useRemoveFromCollection(collectionId);
  return (
    <ul className={coverGridClass}>
      {series.map((one) => (
        <SeriesCard
          key={one.id}
          series={one}
          more={
            mine ? (
              <MenuItem icon={X} onSelect={() => take.mutate(one.id)}>
                Remove from collection
              </MenuItem>
            ) : null
          }
        />
      ))}
    </ul>
  );
});

/**
 * One collection: what it is, and its series by name.
 *
 * The one whose collection it is changes it and says who else sees it.
 * Anyone else looks through it and is told whose it is. On a server of one
 * person there is nobody to share with, and nothing about sharing is shown.
 */
function CollectionPage() {
  const { collectionId } = Route.useParams();
  const found = useCollection(collectionId);
  const series = useCollectionSeries(collectionId);
  // Only the one whose collection it is shares it, so only they need the names.
  const others = useOthers({ enabled: found.data?.mine === true });
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [sharing, setSharing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const change = useChangeCollection(collectionId);
  const share = useShareCollection(collectionId);
  // What goes wrong, and what was done, is said over the page by the hook.
  const onHome = usePutCollectionOnHome(collectionId);
  const remove = useDeleteCollection(() => navigate({ to: "/collections", replace: true }));

  if (found.isPending) {
    return null;
  }
  if (found.isError) {
    return gotNoAnswer(found.error) ? (
      <ServerNotAnswering />
    ) : (
      <NotOnServer title="This collection is not on the server">
        It may have been deleted, it may not be shared with you, or it holds no series you can read.
      </NotOnServer>
    );
  }

  const collection = found.data;
  const mine = collection.mine;
  const canShare = mine && others.length > 0;
  const own = mine ? (
    <>
      <MenuItem icon={House} onSelect={() => onHome.mutate(!collection.on_home)}>
        {collection.on_home ? "Take off Home" : "Show on Home"}
      </MenuItem>
      <MenuItem icon={Pencil} onSelect={() => setEditing(true)}>
        Edit name and summary…
      </MenuItem>
      {canShare ? (
        <MenuItem icon={Users} onSelect={() => setSharing(true)}>
          Share…
        </MenuItem>
      ) : null}
      <MenuItem icon={Trash2} onSelect={() => setDeleting(true)}>
        Delete collection…
      </MenuItem>
    </>
  ) : null;
  const whose = madeAndShared({
    made: collection,
    what: "series",
    canShare,
    onShare: () => setSharing(true),
  });
  return (
    <>
      <header className="mb-8">
        <Link to="/collections" className={upLinkClass}>
          <ChevronLeft className="size-4" aria-hidden="true" />
          Collections
        </Link>
        <div className="mt-2 flex flex-wrap items-start justify-between gap-x-4 gap-y-3">
          <div className="min-w-0">
            <h1 className="font-bold text-screen-title leading-tight tracking-tight">
              {collection.name}
            </h1>
            <p className="mt-2 text-fg-muted text-metadata">
              {seriesCount(collection.series_count)} · {bookCount(collection.book_count)}
            </p>
            {mine ? (
              whose ? (
                <p className="mt-1 text-fg-muted text-metadata">{whose}</p>
              ) : null
            ) : (
              <p className="mt-1 text-fg-muted text-metadata">
                {collection.made_by.name}’s collection
              </p>
            )}
          </div>
          {series.data?.length ? (
            <BooksMenu
              about={collection.name}
              what={{ series: series.data.map((one) => one.id) }}
              unreadCount={collection.unread_count}
            >
              {own}
            </BooksMenu>
          ) : own ? (
            <MoreMenu about={collection.name}>{own}</MoreMenu>
          ) : null}
        </div>
        {collection.summary ? (
          <p className="mt-4 max-w-prose whitespace-pre-line">{collection.summary}</p>
        ) : null}
      </header>

      {series.isError ? (
        <ErrorText>{series.error.message}</ErrorText>
      ) : series.data?.length === 0 ? (
        <p className="max-w-prose text-fg-muted text-metadata">
          This collection has no series yet.
          {mine
            ? " Add some from the menu of a series, or select several in a library and add them together."
            : null}
        </p>
      ) : series.data ? (
        <SeriesOf collectionId={collection.id} series={series.data} mine={mine} />
      ) : null}

      <NameAndSummaryDialog
        open={editing}
        onClose={() => setEditing(false)}
        title="Name and summary"
        of={collection}
        save={change}
        saveLabel="Save"
      />
      <ShareDialog open={sharing} onClose={() => setSharing(false)} made={collection} share={share}>
        Someone you share it with sees the series in it that they can read, and does not see the
        collection while they can read none of them. Only you can change the collection.
      </ShareDialog>
      <ConfirmDialog
        open={deleting}
        onClose={() => {
          setDeleting(false);
          remove.reset();
        }}
        title={`Delete “${collection.name}”?`}
        confirm="Delete collection"
        confirming="Deleting…"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => remove.mutate(collection.id)}
      >
        The server forgets this collection. Its series stay where they are.
      </ConfirmDialog>
    </>
  );
}
