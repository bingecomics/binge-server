import { ArrowDown, ArrowUp } from "lucide-react";
import { useState } from "react";
import { useKeepSetting } from "../api/settings";
import { HOME_ROWS_KEPT_AS, keptAs } from "../homeRows";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Form } from "./Form";
import { iconButtonClass } from "./NavLink";
import { rowClass } from "./Section";

type Row = { id: string; name: string };

/** The rows, to put in order and to show or put away. */
function Arrange({
  rows,
  hidden,
  onClose,
}: {
  rows: Row[];
  hidden: string[];
  onClose: () => void;
}) {
  const keep = useKeepSetting();
  const [order, setOrder] = useState(rows);
  const [away, setAway] = useState(hidden);

  function move(place: number, by: -1 | 1) {
    const moved = [...order];
    const [row] = moved.splice(place, 1);
    if (row) {
      moved.splice(place + by, 0, row);
      setOrder(moved);
    }
  }

  return (
    <Form
      className="mt-2"
      problem={keep.error?.message}
      onSubmit={() =>
        keep.mutate(
          {
            key: HOME_ROWS_KEPT_AS,
            value: keptAs({
              order: order.map((row) => row.id),
              // Only rows there are: one put away and gone since is forgotten.
              hidden: away.filter((id) => order.some((row) => row.id === id)),
            }),
          },
          { onSuccess: onClose },
        )
      }
      actions={
        <DialogActions onCancel={onClose}>
          <Button type="submit" variant="primary" disabled={keep.isPending}>
            Save
          </Button>
        </DialogActions>
      }
    >
      <p className="text-fg-muted text-metadata">
        Home is yours: the order and the rows you choose here follow you to every browser you sign
        in from. A row with nothing in it does not show.
      </p>
      <ul className="mt-3">
        {order.map((row, place) => (
          <li key={row.id} className={`${rowClass} flex items-center gap-2 py-1`}>
            <label className="flex min-h-11 min-w-0 flex-1 items-center gap-3">
              <input
                type="checkbox"
                className="size-5 shrink-0 accent-accent"
                checked={!away.includes(row.id)}
                onChange={(event) =>
                  setAway(
                    event.target.checked ? away.filter((id) => id !== row.id) : [...away, row.id],
                  )
                }
              />
              <span className="min-w-0 truncate">{row.name}</span>
            </label>
            <button
              type="button"
              disabled={place === 0}
              onClick={() => move(place, -1)}
              className={`${iconButtonClass} size-11 disabled:opacity-30`}
              aria-label={`Move ${row.name} up`}
              title="Move up"
            >
              <ArrowUp className="size-5" aria-hidden="true" />
            </button>
            <button
              type="button"
              disabled={place === order.length - 1}
              onClick={() => move(place, 1)}
              className={`${iconButtonClass} size-11 disabled:opacity-30`}
              aria-label={`Move ${row.name} down`}
              title="Move down"
            >
              <ArrowDown className="size-5" aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
    </Form>
  );
}

/**
 * Asks which rows Home shows and in what order. `rows` are all of them in
 * the order they have now, `hidden` the ids of the ones put away.
 */
export function ArrangeHomeDialog({
  open,
  onClose,
  rows,
  hidden,
}: {
  open: boolean;
  onClose: () => void;
  rows: Row[];
  hidden: string[];
}) {
  return (
    <Dialog open={open} onClose={onClose} title="Arrange Home">
      <Arrange rows={rows} hidden={hidden} onClose={onClose} />
    </Dialog>
  );
}
