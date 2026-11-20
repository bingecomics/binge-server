import { Circle, CircleCheck } from "lucide-react";
import type { ReactNode } from "react";
import { type BooksToMark, useMarkRead } from "../api/reading";
import { AddToCollectionChoice } from "./AddToCollection";
import { AddToListChoice } from "./AddToList";
import { MenuItem, MoreMenu } from "./Menu";

/** The two marks: what each does, and what it is called for one book and for all of several. */
export const MARKS = [
  { read: true, icon: CircleCheck, one: "Mark as read", all: "Mark all as read" },
  { read: false, icon: Circle, one: "Mark as unread", all: "Mark all as unread" },
] as const;

/**
 * The two choices of a menu that mark books as read and as unread. `all`
 * is for a menu of something that holds books: a series, a group of one, or
 * a reading list.
 */
export function ReadMarks({
  what,
  all = false,
  canRead = true,
  canUnread = true,
}: {
  what: BooksToMark;
  all?: boolean;
  /** Whether there is anything left to mark as read. */
  canRead?: boolean;
  /** Whether there is anything to mark as unread. */
  canUnread?: boolean;
}) {
  const mark = useMarkRead();
  return MARKS.filter(({ read }) => (read ? canRead : canUnread)).map(
    ({ read, icon, one, all: every }) => (
      <MenuItem key={one} icon={icon} onSelect={() => mark.mutate({ ...what, read })}>
        {all ? every : one}
      </MenuItem>
    ),
  );
}

/**
 * The menu of something that holds books (a series, a group of one, a
 * reading list), in the `corner` of its card or on its page: it marks every
 * book of it, adds them all to a reading list, and adds whole series to a
 * collection. `children` are what else the menu of a page offers.
 */
export function BooksMenu({
  about,
  what,
  unreadCount,
  corner = false,
  children,
}: {
  about: string;
  what: BooksToMark;
  unreadCount: number;
  corner?: boolean;
  children?: ReactNode;
}) {
  return (
    <MoreMenu about={about} corner={corner}>
      <ReadMarks all what={what} canRead={unreadCount > 0} />
      <AddToListChoice what={what} />
      <AddToCollectionChoice what={what} />
      {children}
    </MoreMenu>
  );
}
