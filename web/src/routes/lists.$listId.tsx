import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ChevronLeft, Download, ListX, Pencil, Trash2, Users } from "lucide-react";
import { useState } from "react";
import { useOthers } from "../api/account";
import { cblUrl, gotNoAnswer } from "../api/client";
import {
  useChangeList,
  useDeleteList,
  useListBooks,
  useReadingList,
  useRemoveRead,
  useShareList,
} from "../api/lists";
import { BooksHeader } from "../components/BooksHeader";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { CoverChoices } from "../components/CoverChoices";
import { madeAndShared } from "../components/FromFiles";
import { ListBooks } from "../components/ListBooks";
import { MenuDownload, MenuItem } from "../components/Menu";
import { NameAndSummaryDialog } from "../components/NameAndSummary";
import { NotOnServer, ServerNotAnswering } from "../components/Notice";
import { BooksMenu } from "../components/ReadMarks";
import { ShareDialog } from "../components/ShareDialog";
import { ErrorText, upLinkClass } from "../components/text";
import { books as bookCount, isShared } from "../format";

export const Route = createFileRoute("/lists/$listId")({
  params: {
    parse: ({ listId }) => ({ listId: Number(listId) }),
    stringify: ({ listId }) => ({ listId: String(listId) }),
  },
  component: ListPage,
});

/**
 * One reading list: what its books say together, the way to go on where
 * the reader stopped, and its books in their order. Reading from here
 * follows the list from one series to the next.
 *
 * The one whose list it is changes it and says who else sees it. Anyone
 * else reads it and is told whose it is. On a server of one person there
 * is nobody to share with, and nothing about sharing is shown. Anyone who
 * sees the list saves it as a CBL file.
 */
function ListPage() {
  const { listId } = Route.useParams();
  const overview = useReadingList(listId);
  const books = useListBooks(listId);
  // Only the one whose list it is shares it, so only they need the names.
  const others = useOthers({ enabled: overview.data?.list.mine === true });
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [sharing, setSharing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [removingRead, setRemovingRead] = useState(false);
  const change = useChangeList(listId);
  const share = useShareList(listId);
  const remove = useDeleteList(() => navigate({ to: "/lists", replace: true }));
  const removeRead = useRemoveRead(() => setRemovingRead(false));

  if (overview.isPending) {
    return null;
  }
  if (overview.isError) {
    return gotNoAnswer(overview.error) ? (
      <ServerNotAnswering />
    ) : (
      <NotOnServer title="This reading list is not on the server">
        It may have been deleted, it may not be shared with you, or it holds no book you can read.
      </NotOnServer>
    );
  }

  const { list } = overview.data;
  const mine = list.mine;
  const canShare = mine && others.length > 0;
  const readCount = list.book_count - list.unread_count;
  return (
    <>
      <BooksHeader
        // A new header for each list: what was opened in one stays there.
        key={listId}
        cover={list.cover}
        covers={list.covers}
        title={list.name}
        bookCount={list.book_count}
        overview={overview.data}
        listId={list.id}
        whose={
          mine
            ? madeAndShared({
                made: list,
                what: "books",
                canShare,
                onShare: () => setSharing(true),
              })
            : `${list.made_by.name}’s list`
        }
        menu={
          <BooksMenu about={list.name} what={{ lists: [list.id] }} unreadCount={list.unread_count}>
            {mine ? (
              <>
                <MenuItem icon={Pencil} onSelect={() => setEditing(true)}>
                  Edit name and summary…
                </MenuItem>
                {canShare ? (
                  <MenuItem icon={Users} onSelect={() => setSharing(true)}>
                    Share…
                  </MenuItem>
                ) : null}
                <CoverChoices of={{ kind: "lists", id: list.id }} chosen={list.cover_chosen} />
                {readCount > 0 ? (
                  <MenuItem icon={ListX} onSelect={() => setRemovingRead(true)}>
                    Remove read books…
                  </MenuItem>
                ) : null}
              </>
            ) : null}
            <MenuDownload icon={Download} href={cblUrl(list.id)}>
              Export as CBL
            </MenuDownload>
            {mine ? (
              <MenuItem icon={Trash2} onSelect={() => setDeleting(true)}>
                Delete list…
              </MenuItem>
            ) : null}
          </BooksMenu>
        }
        up={
          <Link to="/lists" className={upLinkClass}>
            <ChevronLeft className="size-4" aria-hidden="true" />
            Reading lists
          </Link>
        }
      />

      {books.isError ? (
        <ErrorText>{books.error.message}</ErrorText>
      ) : books.data?.length === 0 ? (
        // The header has no menu without a book to go on with.
        <div className="mb-8">
          <p className="max-w-prose text-fg-muted text-metadata">
            This list has no books yet.
            {mine
              ? " Add some from the menu of a book, a series or a group, or select several and add them together."
              : null}
          </p>
          {mine ? (
            <div className="-ml-4 mt-2 flex flex-wrap gap-2">
              <Button variant="quiet" onClick={() => setEditing(true)}>
                <Pencil className="size-4" aria-hidden="true" />
                Edit name and summary
              </Button>
              <Button variant="quiet" onClick={() => setDeleting(true)}>
                <Trash2 className="size-4" aria-hidden="true" />
                Delete list
              </Button>
            </div>
          ) : null}
        </div>
      ) : books.data ? (
        <ListBooks listId={list.id} books={books.data} mine={mine} />
      ) : null}

      <NameAndSummaryDialog
        open={editing}
        onClose={() => setEditing(false)}
        title="Name and summary"
        of={list}
        save={change}
        saveLabel="Save"
      />
      <ShareDialog open={sharing} onClose={() => setSharing(false)} made={list} share={share}>
        Someone you share it with sees the books in it that they can read, and does not see the list
        while they can read none of them. What you have read stays yours, and only you can change
        the list.
      </ShareDialog>
      <ConfirmDialog
        open={removingRead}
        onClose={() => {
          setRemovingRead(false);
          removeRead.reset();
        }}
        title={`Remove ${bookCount(readCount)} you have read?`}
        confirm="Remove read books"
        confirming="Removing…"
        isPending={removeRead.isPending}
        error={removeRead.error}
        onConfirm={() => removeRead.mutate(list)}
      >
        The list keeps only what you have not finished. The books stay in the library, and what you
        read of them stays read.
        {isShared(list)
          ? " The list is shared: the books leave it for everyone who sees it."
          : null}
      </ConfirmDialog>
      <ConfirmDialog
        open={deleting}
        onClose={() => {
          setDeleting(false);
          remove.reset();
        }}
        title={`Delete “${list.name}”?`}
        confirm="Delete list"
        confirming="Deleting…"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => remove.mutate(list.id)}
      >
        The server forgets this list. Its books stay where they are, and what was read of them stays
        read.
      </ConfirmDialog>
    </>
  );
}
