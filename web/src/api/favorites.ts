import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, sayProblem, unwrap } from "./client";

/** Start of the key of everything asked about favorite pages. */
const FAVORITES = ["favorite-pages"];

function favoritesInKey(bookId: number) {
  return [...FAVORITES, "book", bookId];
}

/** How many favorite pages a page of their list shows. */
export const FAVORITES_A_PAGE = 60;

/** The pages of a book that are the reader's favorites, by their numbers from 0. */
export function useFavoritesIn(bookId: number) {
  return useQuery({
    queryKey: favoritesInKey(bookId),
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/books/{id}/favorite-pages", { params: { path: { id: bookId } } }),
      ),
  });
}

/** A page of the list of all the reader's favorite pages, series by series. */
export function useFavoritePages(page: number) {
  return useQuery({
    queryKey: [...FAVORITES, "all", page],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/me/favorite-pages", {
          params: { query: { offset: (page - 1) * FAVORITES_A_PAGE, limit: FAVORITES_A_PAGE } },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

/** A page of a book to make a favorite, or no longer one. */
type Marking = { bookId: number; identity: string; number: number; favorite: boolean };

/**
 * Makes a page of a book a favorite, or no longer one. The book's own list
 * of favorites follows at once, and goes back to what the server has when
 * it refuses.
 */
export function useMarkFavorite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ identity, number, favorite }: Marking) => {
      const page = { params: { path: { identity, number } } };
      return unwrap(
        favorite
          ? await api.PUT("/api/v1/pages/{identity}/{number}/favorite", page)
          : await api.DELETE("/api/v1/pages/{identity}/{number}/favorite", page),
      );
    },
    onMutate: async ({ bookId, number, favorite }) => {
      // An answer on its way would put back what this changes.
      await queryClient.cancelQueries({ queryKey: favoritesInKey(bookId) });
      queryClient.setQueryData<number[]>(favoritesInKey(bookId), (favorites = []) =>
        favorite
          ? [...new Set([...favorites, number])].sort((a, b) => a - b)
          : favorites.filter((one) => one !== number),
      );
    },
    onError: sayProblem,
    onSettled: () => queryClient.invalidateQueries({ queryKey: FAVORITES }),
  });
}
