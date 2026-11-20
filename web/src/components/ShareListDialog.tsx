import { useId, useState } from "react";
import { useOthers } from "../api/account";
import type { ReadingList } from "../api/client";
import { useShareList } from "../api/lists";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Form } from "./Form";

/** Who a list is shared with: nobody, everyone, or the people picked. */
type With = "nobody" | "everyone" | "chosen";

const rowClass = "flex min-h-11 items-center gap-3";
const boxClass = "size-5 shrink-0 accent-accent";

/** The three ways a list is shared, and under the last the people to pick. */
function Sharing({ list, onClose }: { list: ReadingList; onClose: () => void }) {
  const others = useOthers();
  const share = useShareList(list.id);
  const [shared, setShared] = useState<With>(
    list.shared_with_all ? "everyone" : list.shared_with.length > 0 ? "chosen" : "nobody",
  );
  const [picked, setPicked] = useState(() => list.shared_with.map((person) => person.id));
  // Someone taken off the server since the list was read has no box to
  // untick, so they are not named again.
  const people = picked.filter((id) => others.some((person) => person.id === id));
  const name = useId();

  const ways: { value: With; label: string }[] = [
    { value: "nobody", label: "Only you" },
    { value: "everyone", label: "Everyone on this server" },
    { value: "chosen", label: "The people you choose" },
  ];
  return (
    <Form
      className="mt-4"
      problem={share.error?.message}
      onSubmit={() =>
        share.mutate(
          { everyone: shared === "everyone", people: shared === "chosen" ? people : [] },
          { onSuccess: onClose },
        )
      }
      actions={
        <DialogActions onCancel={onClose}>
          <Button
            type="submit"
            variant="primary"
            disabled={share.isPending || (shared === "chosen" && people.length === 0)}
          >
            Save
          </Button>
        </DialogActions>
      }
    >
      <fieldset>
        <legend className="sr-only">Who sees this list</legend>
        {ways.map((way) => (
          <label key={way.value} className={rowClass}>
            <input
              type="radio"
              name={name}
              className={boxClass}
              checked={shared === way.value}
              onChange={() => setShared(way.value)}
            />
            <span className="font-semibold text-metadata">{way.label}</span>
          </label>
        ))}
      </fieldset>
      {shared === "chosen" ? (
        <ul className="ml-8 max-h-64 overflow-y-auto" aria-label="The people to share with">
          {others.map((person) => (
            <li key={person.id}>
              <label className={rowClass}>
                <input
                  type="checkbox"
                  className={boxClass}
                  checked={people.includes(person.id)}
                  onChange={(event) =>
                    setPicked(
                      event.target.checked
                        ? [...people, person.id]
                        : people.filter((id) => id !== person.id),
                    )
                  }
                />
                <span className="min-w-0 truncate">{person.name}</span>
              </label>
            </li>
          ))}
        </ul>
      ) : null}
      <p className="mt-3 text-caption text-fg-muted">
        Someone you share it with sees the books in it that they can read, and does not see the list
        while they can read none of them. What you have read stays yours, and only you can change
        the list.
      </p>
    </Form>
  );
}

/** Asks who a reading list of the reader's is shared with. */
export function ShareListDialog({
  open,
  onClose,
  list,
}: {
  open: boolean;
  onClose: () => void;
  list: ReadingList;
}) {
  return (
    <Dialog open={open} onClose={onClose} title={`Share “${list.name}”`}>
      <Sharing list={list} onClose={onClose} />
    </Dialog>
  );
}
