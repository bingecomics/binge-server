import { useQuery } from "@tanstack/react-query";
import { api, type Book, type Followed, unwrap } from "../api/client";

export type { Followed };

/**
 * What the reader follows from a book, and the book that comes next there.
 *
 * It follows where the book was opened from: a reading list (`listId`),
 * the book's group of its series (`inGroup`), or else its series, which
 * goes on from one group to the next. The server says which and what
 * follows, so every reader goes on to the same book. A book in no series,
 * opened from no list, is followed by nothing.
 *
 * Asked once for each book opened, and kept apart from what the pages
 * show, which is asked for again at every page turned. Until the answer
 * has come, and when it does not come, nothing is followed and the book is
 * read all the same.
 */
export function useFollowing(bookId: number, listId: number | undefined, inGroup: boolean) {
  const after = useQuery({
    queryKey: ["reading", "following", bookId, listId ?? null, inGroup],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/books/{id}/next", {
          params: { path: { id: bookId }, query: { list: listId, group: inGroup || undefined } },
        }),
      ),
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: 0,
  }).data;
  const next: Book | undefined = after?.next ?? undefined;
  return { followed: after?.followed ?? null, next };
}
