import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { FileUp, ListOrdered, Plus } from "lucide-react";
import { useState } from "react";
import { gotNoAnswer } from "../api/client";
import { useCreateList, useReadingLists } from "../api/lists";
import { Button, buttonClass } from "../components/Button";
import { ListCard } from "../components/ListCard";
import { MyBingeParts } from "../components/MyBinge";
import { NameAndSummaryDialog } from "../components/NameAndSummary";
import { Notice, ServerNotAnswering } from "../components/Notice";
import { ErrorText } from "../components/text";
import { YoursAndShared } from "../components/YoursAndShared";
import { count } from "../format";

export const Route = createFileRoute("/lists/")({ component: ListsPage });

/**
 * The reading lists a person sees: their own, and apart from them the
 * ones other people shared with them. Anyone makes a list or imports one.
 */
function ListsPage() {
  const lists = useReadingLists();
  const create = useCreateList();
  const navigate = useNavigate();
  const [making, setMaking] = useState(false);

  // The choices of My Binge stay while the page comes, and when it does not.
  if (lists.isPending) {
    return <MyBingeParts />;
  }
  if (lists.isError) {
    return (
      <>
        <MyBingeParts />
        {gotNoAnswer(lists.error) ? (
          <ServerNotAnswering />
        ) : (
          <ErrorText>{lists.error.message}</ErrorText>
        )}
      </>
    );
  }

  return (
    <>
      <MyBingeParts />
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <div className="min-w-0">
          <h1 className="sr-only">Reading lists</h1>
          <p className="text-fg-muted text-metadata">
            {count(lists.data.length, "reading list", "reading lists")}
          </p>
        </div>
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
      </div>

      {lists.data.length === 0 ? (
        <div className="mt-8">
          <Notice icon={ListOrdered} title="No reading lists yet">
            A reading list puts books of any series in one order, as a crossover is read. Make one
            here and add books from their menus, or import a CBL file. It is yours alone until you
            share it.
          </Notice>
        </div>
      ) : (
        <YoursAndShared
          things={lists.data}
          card={(list, titleAs) => <ListCard key={list.id} list={list} titleAs={titleAs} />}
        />
      )}

      <NameAndSummaryDialog
        open={making}
        onClose={() => setMaking(false)}
        title="New reading list"
        save={create}
        saveLabel="Make the list"
        onSaved={(list) => navigate({ to: "/lists/$listId", params: { listId: list.id } })}
      />
    </>
  );
}
