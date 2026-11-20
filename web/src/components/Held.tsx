import { CircleCheck } from "lucide-react";
import type { ReactNode } from "react";
import { books } from "../format";

/** A line under a card's name that starts with the tick of what is read. */
export function ReadLine({ children }: { children: ReactNode }) {
  return (
    <p className="flex items-center gap-1 text-caption text-fg-muted">
      <CircleCheck className="size-3.5 text-success" aria-hidden="true" />
      {children}
    </p>
  );
}

/**
 * What a card says under the name of a series or of a group of one: how many
 * books it holds, and how many of them are still to be read once some are
 * read. A tick when all of them are.
 */
export function Held({ bookCount, unreadCount }: { bookCount: number; unreadCount: number }) {
  const all = books(bookCount);
  if (bookCount > 0 && unreadCount === 0) {
    return (
      <ReadLine>
        <span className="sr-only">Read:</span>
        {all}
      </ReadLine>
    );
  }
  return (
    <p className="text-caption text-fg-muted">
      {unreadCount < bookCount ? `${all} · ${unreadCount.toLocaleString()} unread` : all}
    </p>
  );
}
