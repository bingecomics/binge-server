import { Layers } from "lucide-react";
import { useAddToCollection, useCollections, useCreateCollection } from "../api/collections";
import { type BooksToMark, onlySeries } from "../api/reading";
import { seriesCount } from "../format";
import { tell } from "../problems";
import { store } from "../store";
import { Dialog } from "./Dialog";
import { MenuItem } from "./Menu";
import { PickOrMake } from "./PickOrMake";

/**
 * The series the person asked to add to a collection, until they have said
 * which. Kept outside the components, as what is added to a reading list
 * is: see `AddToList`.
 */
type Asked = { series: number[]; onAdded?: () => void };
const asked = store<Asked | null>(null);

/** Asks which collection to add series to. `onAdded` runs once they are in one. */
export function askToAddToCollection(series: number[], onAdded?: () => void) {
  asked.set({ series, onAdded });
}

/**
 * The choice of a menu that adds what the menu is about to a collection of
 * the reader's. A collection holds whole series, so a menu about anything
 * else has no such choice.
 */
export function AddToCollectionChoice({ what }: { what: BooksToMark }) {
  const series = onlySeries(what);
  if (!series) {
    return null;
  }
  return (
    <MenuItem icon={Layers} onSelect={() => askToAddToCollection(series)}>
      Add to a collection…
    </MenuItem>
  );
}

/**
 * The reader's own collections to choose from, and the way to make a new
 * one with the series in it. A collection someone shared with them is not
 * theirs to add to.
 */
function Collections({ series, onAdded, onClose }: Asked & { onClose: () => void }) {
  const collections = useCollections();
  const add = useAddToCollection();
  const create = useCreateCollection();
  const problem = add.error ?? create.error ?? collections.error;

  function done() {
    onAdded?.();
    onClose();
  }

  return (
    <PickOrMake
      kind="collection"
      choices={collections.data?.filter((collection) => collection.mine) ?? []}
      holds={(collection) => seriesCount(collection.series_count)}
      busy={add.isPending || create.isPending}
      problem={problem?.message}
      onPick={(collection) => add.mutate({ collection, series }, { onSuccess: done })}
      onMake={(name) =>
        create.mutate(
          { name, series },
          {
            onSuccess: (collection) => {
              tell(`Made “${collection.name}” with ${seriesCount(collection.series_count)}.`);
              done();
            },
          },
        )
      }
      onClose={onClose}
    />
  );
}

/**
 * The dialog that asks which collection to add to. There is one, in the
 * frame around every page: see `askToAddToCollection`.
 */
export function AddToCollectionDialog() {
  const now = asked.useValue((value) => value);
  const close = () => asked.set(null);
  return (
    <Dialog open={now !== null} onClose={close} title="Add to a collection">
      {now ? <Collections {...now} onClose={close} /> : null}
    </Dialog>
  );
}
