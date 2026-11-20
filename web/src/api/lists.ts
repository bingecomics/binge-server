import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { books as bookCount } from "../format";
import { tell } from "../problems";
import {
  api,
  type Book,
  type CblEntry,
  fileBody,
  type NewReadingList,
  type ReadingList,
  type ReadingListDetails,
  type Sharing,
  sayProblem,
  unwrap,
} from "./client";
import { askAgainWithSearch, CONTENTS } from "./libraries";
import type { BooksToMark } from "./reading";

/** Start of the key of everything asked about reading lists. Reading changes it, as it does the rest. */
const LISTS = [...CONTENTS, "lists"];

function booksKey(listId: number) {
  return [...LISTS, listId, "books"];
}

/**
 * The reading lists the reader sees, by name: their own, and the ones
 * shared with them that hold a book they may read.
 */
export function useReadingLists({ enabled = true }: { enabled?: boolean } = {}) {
  return useQuery({
    queryKey: LISTS,
    queryFn: async () => unwrap(await api.GET("/api/v1/reading-lists")),
    enabled,
  });
}

/** One reading list seen whole: what its books say together, and how far the reader is through it. */
export function useReadingList(id: number) {
  return useQuery({
    queryKey: [...LISTS, id, "overview"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/reading-lists/{id}", { params: { path: { id } } })),
  });
}

async function listBooks(listId: number) {
  return unwrap(
    await api.GET("/api/v1/reading-lists/{id}/books", { params: { path: { id: listId } } }),
  );
}

/** The books of a reading list, in its order. */
export function useListBooks(listId: number) {
  return useQuery({ queryKey: booksKey(listId), queryFn: () => listBooks(listId) });
}

/** The reading lists a book is in. */
export function useListsOfBook(bookId: number) {
  return useQuery({
    queryKey: [...LISTS, "of-book", bookId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/books/{id}/reading-lists", { params: { path: { id: bookId } } }),
      ),
  });
}

/** The reading lists that hold a book of a series. */
export function useListsOfSeries(seriesId: number) {
  return useQuery({
    queryKey: [...LISTS, "of-series", seriesId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/series/{id}/reading-lists", { params: { path: { id: seriesId } } }),
      ),
  });
}

/** Makes a reading list of the reader's own, with books in it or none. */
export function useCreateList() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (list: NewReadingList) =>
      unwrap(await api.POST("/api/v1/reading-lists", { body: list })),
    onSuccess: () => askAgainWithSearch(queryClient, LISTS),
  });
}

/** Gives a reading list of the reader's another name and summary. */
export function useChangeList(id: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (details: ReadingListDetails) =>
      unwrap(
        await api.PATCH("/api/v1/reading-lists/{id}", { params: { path: { id } }, body: details }),
      ),
    onSuccess: () => askAgainWithSearch(queryClient, LISTS),
  });
}

/** Says who a reading list of the reader's is shared with from now on. */
export function useShareList(id: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (sharing: Sharing) =>
      unwrap(
        await api.PUT("/api/v1/reading-lists/{id}/sharing", {
          params: { path: { id } },
          body: sharing,
        }),
      ),
    onSuccess: () => askAgainWithSearch(queryClient, LISTS),
  });
}

/**
 * Whether a reading list is the reader's own, by what the lists already
 * asked for say: for a part of the page that is given only the list's id.
 * Without an id nothing is asked.
 */
export function useIsOwnList(id: number | undefined): boolean {
  const lists = useReadingLists({ enabled: id !== undefined }).data;
  return id !== undefined && lists?.find((list) => list.id === id)?.mine === true;
}

/**
 * Deletes a reading list of the reader's. Its books stay. `onDeleted` runs first and the
 * lists are refreshed after, so the list's page can be left before it is gone.
 */
export function useDeleteList(onDeleted: () => unknown) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/reading-lists/{id}", { params: { path: { id } } })),
    onSuccess: async () => {
      await onDeleted();
      await askAgainWithSearch(queryClient, LISTS);
    },
  });
}

/**
 * Adds books to the end of a reading list, leaving out the ones it holds
 * already, and says over the page how many went in: nothing where the
 * person asked shows the list. Only the one whose list it is can.
 */
export function useAddToList() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ list, what }: { list: ReadingList; what: BooksToMark }) =>
      unwrap(
        await api.POST("/api/v1/reading-lists/{id}/books", {
          params: { path: { id: list.id } },
          body: what,
        }),
      ),
    onSuccess: (added, { list }) => {
      tell(
        added.books > 0
          ? `Added ${bookCount(added.books)} to “${list.name}”.`
          : `“${list.name}” already holds all of that.`,
      );
      return askAgainWithSearch(queryClient, LISTS);
    },
  });
}

/** Takes a book out of a reading list of the reader's. */
export function useRemoveFromList(listId: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (bookId: number) =>
      unwrap(
        await api.DELETE("/api/v1/reading-lists/{id}/books/{book_id}", {
          params: { path: { id: listId, book_id: bookId } },
        }),
      ),
    onSuccess: () => askAgainWithSearch(queryClient, LISTS),
    onError: sayProblem,
  });
}

/**
 * Moves a book of a reading list to a position among the books shown, from
 * 0. The page shows the new order at once and the server's when it answers.
 * Only the one whose list it is can.
 */
export function useMoveInList(listId: number) {
  const queryClient = useQueryClient();
  const key = booksKey(listId);
  return useMutation({
    mutationFn: async ({ bookId, position }: { bookId: number; position: number }) =>
      unwrap(
        await api.PUT("/api/v1/reading-lists/{id}/books/{book_id}/position", {
          params: { path: { id: listId, book_id: bookId } },
          body: { position },
        }),
      ),
    onMutate: async ({ bookId, position }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<Book[]>(key);
      const moved = before?.find((book) => book.id === bookId);
      if (before && moved) {
        const after = before.filter((book) => book !== moved);
        after.splice(position, 0, moved);
        queryClient.setQueryData(key, after);
      }
      return before;
    },
    onError: (error, _, before) => {
      queryClient.setQueryData(key, before);
      sayProblem(error);
    },
    // The list's first covers and what to read next follow its order.
    onSettled: () => askAgainWithSearch(queryClient, LISTS),
  });
}

/** The biggest CBL file the server takes, in megabytes. */
export const BIGGEST_CBL = 5;

/**
 * Asks the server what a CBL file would make: for each of its entries, the
 * book it found and how. Nothing is made.
 */
export function usePlanImport() {
  return useMutation({
    mutationFn: async (file: File) =>
      unwrap(
        await api.POST(
          "/api/v1/reading-lists/import",
          fileBody(file, "file", BIGGEST_CBL, "application/xml"),
        ),
      ),
  });
}

/** What the reader says an entry of a CBL file is: every issue of a series, or one book. */
export type RuleFor = { entry: CblEntry } & ({ series_id: number } | { book_id: number });

/** Keeps what an entry of a CBL file means, for this import of the reader's and the ones after. */
export function useKeepRule() {
  return useMutation({
    mutationFn: async (rule: RuleFor) =>
      unwrap(await api.POST("/api/v1/reading-lists/import/rules", { body: rule })),
  });
}

/** Forgets a rule the reader kept for their imports. */
export function useForgetRule() {
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(
        await api.DELETE("/api/v1/reading-lists/import/rules/{id}", { params: { path: { id } } }),
      ),
  });
}
