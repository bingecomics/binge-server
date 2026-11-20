import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, sayProblem, unwrap } from "./client";

/** Start of the key of everything asked about kept pages. */
const KEPT = ["kept-pages"];

function keptInKey(bookId: number) {
  return [...KEPT, "book", bookId];
}

/** How many kept pages a page of their list shows. */
export const KEPT_A_PAGE = 60;

/** The pages of a book the reader kept, by their numbers from 0. */
export function useKeptIn(bookId: number) {
  return useQuery({
    queryKey: keptInKey(bookId),
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/books/{id}/kept-pages", { params: { path: { id: bookId } } })),
  });
}

/** A page of the list of everything the reader kept, series by series. */
export function useKeptPages(page: number) {
  return useQuery({
    queryKey: [...KEPT, "all", page],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/me/kept-pages", {
          params: { query: { offset: (page - 1) * KEPT_A_PAGE, limit: KEPT_A_PAGE } },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

/** A page of a book to keep, or to let go. */
type Keeping = { bookId: number; identity: string; number: number; keep: boolean };

/**
 * Keeps a page of a book, or lets it go. The book's own list of kept pages
 * follows at once, and goes back to what the server has when it refuses.
 */
export function useKeepPage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ identity, number, keep }: Keeping) => {
      const page = { params: { path: { identity, number } } };
      return unwrap(
        keep
          ? await api.PUT("/api/v1/pages/{identity}/{number}/kept", page)
          : await api.DELETE("/api/v1/pages/{identity}/{number}/kept", page),
      );
    },
    onMutate: async ({ bookId, number, keep }) => {
      // An answer on its way would put back what this changes.
      await queryClient.cancelQueries({ queryKey: keptInKey(bookId) });
      queryClient.setQueryData<number[]>(keptInKey(bookId), (kept = []) =>
        keep
          ? [...new Set([...kept, number])].sort((a, b) => a - b)
          : kept.filter((one) => one !== number),
      );
    },
    onError: sayProblem,
    onSettled: () => queryClient.invalidateQueries({ queryKey: KEPT }),
  });
}
