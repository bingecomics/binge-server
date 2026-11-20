import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { FileUp, ListOrdered, Plus } from "lucide-react";
import { useState } from "react";
import { gotNoAnswer, type ReadingList } from "../api/client";
import { useReadingLists } from "../api/lists";
import { Button, buttonClass } from "../components/Button";
import { coverGridClass } from "../components/Cover";
import { ListCard } from "../components/ListCard";
import { ListDetailsDialog } from "../components/ListDetailsDialog";
import { navHeadingClass, saidByTheMenuClass } from "../components/NavLink";
import { Notice, ServerNotAnswering } from "../components/Notice";
import { ErrorText } from "../components/text";
import { count } from "../format";

export const Route = createFileRoute("/lists/")({ component: ListsPage });

/** Some of the lists as cards, under a heading when there are two kinds to tell apart. */
function Lists({ heading, lists }: { heading: string | null; lists: ReadingList[] }) {
  if (lists.length === 0) {
    return null;
  }
  return (
    <section className="mt-8">
      {heading ? <h2 className={`mb-3 ${navHeadingClass}`}>{heading}</h2> : null}
      <ul className={coverGridClass}>
        {lists.map((list) => (
          <ListCard key={list.id} list={list} titleAs={heading ? "h3" : "h2"} />
        ))}
      </ul>
    </section>
  );
}

/**
 * The reading lists a person sees: their own, and apart from them the
 * ones other people shared with them. Anyone makes a list or imports one.
 */
function ListsPage() {
  const lists = useReadingLists();
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

  const mine = lists.data.filter((list) => list.mine);
  const shared = lists.data.filter((list) => !list.mine);
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
        <>
          <Lists heading={shared.length > 0 ? "Yours" : null} lists={mine} />
          <Lists heading="Shared with you" lists={shared} />
        </>
      )}

      <ListDetailsDialog
        open={making}
        onClose={() => setMaking(false)}
        onSaved={(list) => navigate({ to: "/lists/$listId", params: { listId: list.id } })}
      />
    </>
  );
}
