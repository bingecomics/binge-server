import { type ReactNode, useState } from "react";
import type { FromFiles, Library } from "../api/client";
import { useMakeFromFiles } from "../api/libraries";
import { type Made, sharedWith } from "../format";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Tick } from "./Field";
import { Form } from "./Form";
import { textLinkClass } from "./text";

/** What a library makes when it is added from this app, unless its owner says otherwise. */
export const MAKES_BOTH: FromFiles = { lists: true, collections: true };

/**
 * What a library makes from its files, as a sentence about it: "Makes
 * reading lists and collections from its files".
 */
export function whatItMakes(from: FromFiles): string {
  const made = [from.lists ? "reading lists" : null, from.collections ? "collections" : null]
    .filter((one) => one !== null)
    .join(" and ");
  return `Makes ${made || "nothing"} from its files`;
}

/**
 * The two things a library makes from what its files say, each to tick.
 * `heading` names them where nothing above does.
 */
export function FromFilesTicks({
  from,
  onChange,
  heading = false,
}: {
  from: FromFiles;
  onChange: (from: FromFiles) => void;
  heading?: boolean;
}) {
  return (
    <fieldset>
      <legend className={heading ? "font-semibold text-metadata" : "sr-only"}>
        From what its files say
      </legend>
      <Tick
        name="Reading lists from story arcs"
        under="A list for each story arc the files name, its books in the order they give."
        checked={from.lists}
        onChange={(lists) => onChange({ ...from, lists })}
      />
      <Tick
        name="Collections from groups of series"
        under="A collection for each group of series the files name."
        checked={from.collections}
        onChange={(collections) => onChange({ ...from, collections })}
      />
      <p className="mt-3 text-caption text-fg-muted">
        They are made as yours and shared with everyone on this server. You change, share and delete
        them like any other. A scan adds what new files name and takes nothing out. No file is
        changed.
      </p>
    </fieldset>
  );
}

function MakeFromFiles({ library, onClose }: { library: Library; onClose: () => void }) {
  const [from, setFrom] = useState<FromFiles>(
    library.from_files ?? { lists: false, collections: false },
  );
  const make = useMakeFromFiles();
  return (
    <Form
      className="mt-2"
      problem={make.error?.message}
      onSubmit={() => make.mutate({ id: library.id, from }, { onSuccess: onClose })}
      actions={
        <DialogActions onCancel={onClose}>
          <Button type="submit" variant="primary" disabled={make.isPending}>
            Save
          </Button>
        </DialogActions>
      }
    >
      <FromFilesTicks from={from} onChange={setFrom} />
      <p className="mt-3 text-caption text-fg-muted">
        What you turn on is made now, from what the last scan read. What you turn off makes nothing
        more, and what it made stays.
      </p>
    </Form>
  );
}

/** Asks the owner what a library makes from what its files say. */
export function FromFilesDialog({
  open,
  library,
  onClose,
}: {
  open: boolean;
  library: Library;
  onClose: () => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} title={`What “${library.name}” makes from its files`}>
      <MakeFromFiles library={library} onClose={onClose} />
    </Dialog>
  );
}

/**
 * What the page of a reading list or a collection says under its name to
 * the one whose it is: that the files made it, and who it is shared with,
 * which opens the choice of that. Nothing when there is neither to say:
 * a thing made by hand on a server with nobody to share with.
 */
export function madeAndShared({
  made,
  what,
  canShare,
  onShare,
}: {
  made: Made & { from_files: boolean };
  /** What the files put in it. */
  what: "books" | "series";
  canShare: boolean;
  onShare: () => void;
}): ReactNode {
  if (!made.from_files && !canShare) {
    return null;
  }
  return (
    <>
      {made.from_files ? (
        <span
          title={`A scan adds the ${what} whose files name it. It is yours to change or delete.`}
        >
          Made from your files
        </span>
      ) : null}
      {made.from_files && canShare ? " · " : null}
      {canShare ? (
        <button type="button" className={textLinkClass} onClick={onShare}>
          {sharedWith(made)}
        </button>
      ) : null}
    </>
  );
}
