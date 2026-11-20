import { BookImage, ImageOff, ImagePlus, Images, ListOrdered, type LucideIcon } from "lucide-react";
import { useIsOwner } from "../api/account";
import type { Book } from "../api/client";
import { type CoverOf, PICTURES, useChooseCover, useForgetCover } from "../api/covers";
import { askForFile } from "../askForFile";
import { MenuItem } from "./Menu";

/**
 * The choices of a menu that change the cover of a book, of a series or of
 * a reading list: a picture in its place, and back to the usual one once
 * one was `chosen`.
 * They are the owner's; nobody else has them.
 */
export function CoverChoices({ of, chosen }: { of: CoverOf; chosen: boolean }) {
  const owner = useIsOwner();
  const choose = useChooseCover();
  const forget = useForgetCover();
  if (!owner) {
    return null;
  }
  return (
    <>
      <MenuItem
        icon={ImagePlus}
        onSelect={() => askForFile(PICTURES, (picture) => choose.mutate({ of, from: picture }))}
      >
        Choose a cover…
      </MenuItem>
      {chosen ? (
        <MenuItem icon={ImageOff} onSelect={() => forget.mutate(of)}>
          Use the usual cover
        </MenuItem>
      ) : null}
    </>
  );
}

/**
 * The choices of the reader's menu that make a page on screen the cover of
 * the book being read, of its series, or of the reading list it is read
 * from. `pages` are the pages on screen, from 0: with two of them, each
 * choice names its page.
 */
export function PageAsCover({
  book,
  pages,
  listId,
}: {
  book: Book;
  pages: number[];
  listId?: number;
}) {
  const choose = useChooseCover();
  const covers: { of: CoverOf; name: string; icon: LucideIcon }[] = [
    { of: { kind: "books", id: book.id }, name: "the book's cover", icon: BookImage },
  ];
  if (book.series_id != null) {
    const of = { kind: "series", id: book.series_id } as const;
    covers.push({ of, name: "the series' cover", icon: Images });
  }
  if (listId !== undefined) {
    const of = { kind: "lists", id: listId } as const;
    covers.push({ of, name: "the reading list's cover", icon: ListOrdered });
  }
  return pages.flatMap((page) =>
    covers.map(({ of, name, icon }) => (
      <MenuItem
        key={`${page}:${of.kind}`}
        icon={icon}
        onSelect={() =>
          choose.mutate({
            of,
            from: { bookId: book.id, page },
            done: `Page ${page + 1} is now ${name}.`,
          })
        }
      >
        {pages.length > 1 ? `Page ${page + 1} as ${name}` : `Use as ${name}`}
      </MenuItem>
    )),
  );
}
