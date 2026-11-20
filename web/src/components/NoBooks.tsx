import { useNavigate } from "@tanstack/react-router";
import { Trash2 } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useIsOwner } from "../api/account";
import type { Group, Series } from "../api/client";
import { useDeleteGroup, useDeleteSeries } from "../api/libraries";
import { groupName } from "../format";
import { Button } from "./Button";
import { ConfirmDialog } from "./ConfirmDialog";

/**
 * What a series or a group with no books shows in their place. The server
 * keeps it until it is deleted, which is the owner's to do.
 */
function NoBooks({
  what,
  name,
  remove,
  onDelete,
  children,
}: {
  what: "series" | "group";
  name: string;
  /** The state of deleting it. */
  remove: { isPending: boolean; error: Error | null; reset: () => void };
  onDelete: () => void;
  /** What deleting it does, in a sentence or two. */
  children: ReactNode;
}) {
  const isOwner = useIsOwner();
  const [deleting, setDeleting] = useState(false);

  function close() {
    setDeleting(false);
    remove.reset();
  }

  return (
    <div className="mb-8">
      <p className="text-fg-muted text-metadata">This {what} has no books.</p>
      {isOwner ? (
        <Button variant="quiet" className="-ml-4 mt-2" onClick={() => setDeleting(true)}>
          <Trash2 className="size-4" aria-hidden="true" />
          Delete {what}
        </Button>
      ) : null}
      <ConfirmDialog
        open={deleting}
        onClose={close}
        title={`Delete “${name}”?`}
        confirm={`Delete ${what}`}
        confirming="Deleting…"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={onDelete}
      >
        {children}
      </ConfirmDialog>
    </div>
  );
}

/** A series with no books, and the way to delete it. Leaves for its library once it is gone. */
export function EmptySeries({ series }: { series: Series }) {
  const navigate = useNavigate();
  const remove = useDeleteSeries(() =>
    navigate({ to: "/libraries/$libraryId", params: { libraryId: series.library_id } }),
  );
  return (
    <NoBooks
      what="series"
      name={series.name}
      remove={remove}
      onDelete={() => remove.mutate(series.id)}
    >
      The server forgets this series and its groups. No file is touched. If its comics show up
      again, they are put in a series as new ones are.
    </NoBooks>
  );
}

/** A group with no books, and the way to delete it. Leaves for its series once it is gone. */
export function EmptyGroup({ group }: { group: Group }) {
  const navigate = useNavigate();
  const remove = useDeleteGroup(() =>
    navigate({ to: "/series/$seriesId", params: { seriesId: group.series_id } }),
  );
  return (
    <NoBooks
      what="group"
      name={groupName(group)}
      remove={remove}
      onDelete={() => remove.mutate(group.id)}
    >
      The server forgets this group. Its series stays, and no file is touched.
    </NoBooks>
  );
}
