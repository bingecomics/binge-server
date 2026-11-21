import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { seriesCount } from "../format";
import { tell } from "../problems";
import { api, sayProblem, unwrap } from "./client";
import { CONTENTS } from "./libraries";

/** How many series a page of want to read shows. */
export const WANTED_A_PAGE = 120;

/**
 * A page of the series the reader wants to read, the ones put there last
 * first, with how many there are in all. It is theirs alone. A series they
 * have read to the end is left out, until it has a book they have not
 * read. `aPage` is how many a page holds: Home asks for its row's worth.
 */
export function useWantToRead(page = 1, aPage = WANTED_A_PAGE) {
  return useQuery({
    queryKey: [...CONTENTS, "want-to-read", page, aPage],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/want-to-read", {
          params: { query: { offset: (page - 1) * aPage, limit: aPage } },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

/**
 * Puts series on the reader's want to read and says over the page how many
 * went on: nothing where the person asked shows it. Every series says
 * whether it is on it, so every list of series is asked for again. `onPut`
 * runs once the server has them.
 */
export function useWant(onPut?: () => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (series: number[]) =>
      unwrap(await api.POST("/api/v1/want-to-read", { body: { series } })),
    onSuccess: (added) => {
      onPut?.();
      tell(
        added.series > 0
          ? `Added ${seriesCount(added.series)} to want to read.`
          : "Your want to read already holds all of that.",
      );
      return queryClient.invalidateQueries({ queryKey: CONTENTS });
    },
    onError: sayProblem,
  });
}

/** Takes a series off the reader's want to read, and says so over the page. */
export function useWantNoMore() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (seriesId: number) =>
      unwrap(
        await api.DELETE("/api/v1/want-to-read/{series_id}", {
          params: { path: { series_id: seriesId } },
        }),
      ),
    onSuccess: () => {
      tell("Removed from want to read.");
      return queryClient.invalidateQueries({ queryKey: CONTENTS });
    },
    onError: sayProblem,
  });
}
