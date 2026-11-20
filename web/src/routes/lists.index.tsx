import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { FileUp, ListOrdered, Plus } from "lucide-react";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import { gotNoAnswer } from "../api/client";
import { useReadingLists } from "../api/lists";
import { Button, buttonClass } from "../components/Button";
import { coverGridClass } from "../components/Cover";
import { ListCard } from "../components/ListCard";
import { ListDetailsDialog } from "../components/ListDetailsDialog";
import { saidByTheMenuClass } from "../components/NavLink";
import { Notice, ServerNotAnswering } from "../components/Notice";
import { ErrorText } from "../components/text";
import { count } from "../format";

export const Route = createFileRoute("/lists/")({ component: ListsPage });

/**
 * The reading lists of the server. Everyone sees the ones that hold a book
 * they may read; making one and importing one are the owner's.
 */
function ListsPage() {
  const lists = useReadingLists();
  const owner = useIsOwner();
  const navigate = useNavigate();
  const [making, setMaking] = useState(false);

  if (lists.isPending) {
    return null;
  }
  if (lists.isError) {
    return gotNoAnswer(lists.error) ? (
      <ServerNotAnswering />
    ) : (
      <ErrorText>{lists.error.message}</ErrorText>
    );
  }

  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <div className="min-w-0">
          <h1 className={`mb-1 font-bold text-screen-title tracking-tight ${saidByTheMenuClass}`}>
            Reading lists
          </h1>
          <p className="text-fg-muted text-metadata">
            {count(lists.data.length, "reading list", "reading lists")}
          </p>
        </div>
        {owner ? (
          <div className="flex flex-wrap gap-2">
            <Link to="/lists/import" className={buttonClass()}>
              <FileUp className="size-5" aria-hidden="true" />
              Import a CBL file
            </Link>
            <Button variant="primary" onClick={() => setMaking(true)}>
              <Plus className="size-5" aria-hidden="true" />
              New list
            </Button>
          </div>
        ) : null}
      </div>

      <div className="mt-8">
        {lists.data.length === 0 ? (
          <Notice icon={ListOrdered} title="No reading lists yet">
            {owner
              ? "A reading list puts books of any series in one order, as a crossover is read. " +
                "Make one here and add books from their menus, or import a CBL file."
              : "The owner of this server has not made one with books you can read."}
          </Notice>
        ) : (
          <ul className={coverGridClass}>
            {lists.data.map((list) => (
              <ListCard key={list.id} list={list} />
            ))}
          </ul>
        )}
      </div>

      <ListDetailsDialog
        open={making}
        onClose={() => setMaking(false)}
        onSaved={(list) => navigate({ to: "/lists/$listId", params: { listId: list.id } })}
      />
    </>
  );
}
