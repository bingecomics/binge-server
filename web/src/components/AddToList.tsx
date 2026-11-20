import { ListPlus } from "lucide-react";
import { useAddToList, useCreateList, useReadingLists } from "../api/lists";
import type { BooksToMark } from "../api/reading";
import { books as bookCount } from "../format";
import { tell } from "../problems";
import { store } from "../store";
import { Dialog } from "./Dialog";
import { MenuItem } from "./Menu";
import { PickOrMake } from "./PickOrMake";

/**
 * What the person asked to add to a reading list, until they have said
 * which. Kept outside the components: the menu that asks is gone before
 * the dialog that answers is up, and there is one dialog for every menu.
 */
type Asked = { what: BooksToMark; onAdded?: () => void };
const asked = store<Asked | null>(null);

/** Asks which reading list to add books to. `onAdded` runs once they are in one. */
export function askToAddToList(what: BooksToMark, onAdded?: () => void) {
  asked.set({ what, onAdded });
}

/** The choice of a menu that adds books to a reading list of the reader's. */
export function AddToListChoice({ what }: { what: BooksToMark }) {
  return (
    <MenuItem icon={ListPlus} onSelect={() => askToAddToList(what)}>
      Add to a reading list…
    </MenuItem>
  );
}

/**
 * The reader's own reading lists to choose from, and the way to make a new
 * one with the books in it. A list someone shared with them is not theirs
 * to add to.
 */
function Lists({ what, onAdded, onClose }: Asked & { onClose: () => void }) {
  const lists = useReadingLists();
  const add = useAddToList();
  const create = useCreateList();
  const problem = add.error ?? create.error ?? lists.error;

  function done() {
    onAdded?.();
    onClose();
  }

  return (
    <PickOrMake
      kind="list"
      // A list is not added to itself.
      choices={lists.data?.filter((list) => list.mine && !what.lists?.includes(list.id)) ?? []}
      holds={(list) => bookCount(list.book_count)}
      busy={add.isPending || create.isPending}
      problem={problem?.message}
      onPick={(list) => add.mutate({ list, what }, { onSuccess: done })}
      onMake={(name) =>
        create.mutate(
          { name, ...what },
          {
            onSuccess: (list) => {
              tell(`Made “${list.name}” with ${bookCount(list.book_count)}.`);
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
 * The dialog that asks which reading list to add to. There is one, in the
 * frame around every page: see `askToAddToList`.
 */
export function AddToListDialog() {
  const now = asked.useValue((value) => value);
  const close = () => asked.set(null);
  return (
    <Dialog open={now !== null} onClose={close} title="Add to a reading list">
      {now ? <Lists {...now} onClose={close} /> : null}
    </Dialog>
  );
}
