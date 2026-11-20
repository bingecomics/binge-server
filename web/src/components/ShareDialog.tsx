import { type ReactNode, useId, useState } from "react";
import { useOthers } from "../api/account";
import type { Sharing as Shared } from "../api/client";
import type { Made } from "../format";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Form } from "./Form";

/** Who a thing is shared with: nobody, everyone, or the people picked. */
type With = "nobody" | "everyone" | "chosen";

/** What says who a thing is shared with from now on: see `useShareList`. */
type Share = {
  mutate: (sharing: Shared, options: { onSuccess: () => void }) => void;
  isPending: boolean;
  error: Error | null;
  /** Forgets what went wrong the last time. */
  reset: () => void;
};

const rowClass = "flex min-h-11 items-center gap-3";
const boxClass = "size-5 shrink-0 accent-accent";

/** The three ways a thing is shared, and under the last the people to pick. */
function Sharing({
  made,
  share,
  onClose,
  children,
}: {
  made: Made;
  share: Share;
  onClose: () => void;
  children: ReactNode;
}) {
  const others = useOthers();
  const [shared, setShared] = useState<With>(
    made.shared_with_all ? "everyone" : made.shared_with.length > 0 ? "chosen" : "nobody",
  );
  const [picked, setPicked] = useState(() => made.shared_with.map((person) => person.id));
  // Someone taken off the server since it was read has no box to untick,
  // so they are not named again.
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
        <legend className="sr-only">Who sees it</legend>
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
      <p className="mt-3 text-caption text-fg-muted">{children}</p>
    </Form>
  );
}

/**
 * Asks who a reading list or a collection of the reader's is shared with.
 * `children` say what someone it is shared with sees of it.
 */
export function ShareDialog({
  open,
  onClose,
  made,
  share,
  children,
}: {
  open: boolean;
  onClose: () => void;
  made: Made;
  share: Share;
  children: ReactNode;
}) {
  // What went wrong is not there again when the dialog is opened again.
  function close() {
    share.reset();
    onClose();
  }
  return (
    <Dialog open={open} onClose={close} title={`Share “${made.name}”`}>
      <Sharing made={made} share={share} onClose={close}>
        {children}
      </Sharing>
    </Dialog>
  );
}
