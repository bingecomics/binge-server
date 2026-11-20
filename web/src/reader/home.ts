import { linkOptions } from "@tanstack/react-router";
import type { Book } from "../api/client";

/**
 * Where to go from a book that was opened by its address, so that there is no
 * page to go back to: its series, or its library for a book that is in no
 * series.
 */
export function homeOf(book: Book) {
  return book.series_id != null
    ? linkOptions({ to: "/series/$seriesId", params: { seriesId: book.series_id } })
    : linkOptions({ to: "/libraries/$libraryId", params: { libraryId: book.library_id } });
}

/**
 * Where leaving a book leads when there is no page to go back to: the
 * reading list it is read from (`listId`), or else its home.
 */
export function backFrom(book: Book, listId?: number) {
  return listId !== undefined
    ? linkOptions({ to: "/lists/$listId", params: { listId } })
    : homeOf(book);
}
