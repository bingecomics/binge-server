import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ChevronLeft, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import { gotNoAnswer } from "../api/client";
import { useDeleteList, useListBooks, useReadingList } from "../api/lists";
import { BooksHeader } from "../components/BooksHeader";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { CoverChoices } from "../components/CoverChoices";
import { ListBooks } from "../components/ListBooks";
import { ListDetailsDialog } from "../components/ListDetailsDialog";
import { MenuItem } from "../components/Menu";
import { NotOnServer, ServerNotAnswering } from "../components/Notice";
import { BooksMenu } from "../components/ReadMarks";
import { ErrorText, upLinkClass } from "../components/text";

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
 */
function ListPage() {
  const { listId } = Route.useParams();
  const overview = useReadingList(listId);
  const books = useListBooks(listId);
  const owner = useIsOwner();
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const remove = useDeleteList(() => navigate({ to: "/lists", replace: true }));

  if (overview.isPending) {
    return null;
  }
  if (overview.isError) {
    return gotNoAnswer(overview.error) ? (
      <ServerNotAnswering />
    ) : (
      <NotOnServer title="This reading list is not on the server">
        It may have been deleted, or it holds no book you can read.
      </NotOnServer>
    );
  }

  const { list } = overview.data;
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
        menu={
          <BooksMenu about={list.name} what={{ lists: [list.id] }} unreadCount={list.unread_count}>
            {owner ? (
              <MenuItem icon={Pencil} onSelect={() => setEditing(true)}>
                Edit name and summary…
              </MenuItem>
            ) : null}
            <CoverChoices of={{ kind: "lists", id: list.id }} chosen={list.cover_chosen} />
            {owner ? (
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
            {owner
              ? " Add some from the menu of a book, a series or a group, or select several and add them together."
              : null}
          </p>
          {owner ? (
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
        <ListBooks listId={list.id} books={books.data} />
      ) : null}

      <ListDetailsDialog open={editing} onClose={() => setEditing(false)} list={list} />
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
