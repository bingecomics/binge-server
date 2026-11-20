import { useQuery } from "@tanstack/react-query";
import { api, unwrap } from "./client";

/**
 * What the reader read in a year: the days, the months and the writers.
 * The days are the ones of this browser's clock.
 */
export function useReadingYear(year: number) {
  // How far this clock is ahead of UTC today, so that what was read tonight
  // is on today. The server takes one distance for the whole year: where
  // clocks change with the season, a sitting begun within an hour of
  // midnight in the other season is told on the day beside its own.
  const minutesAhead = -new Date().getTimezoneOffset();
  return useQuery({
    queryKey: ["reading-year", year, minutesAhead],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/me/reading", {
          params: { query: { year, minutes_ahead: minutesAhead } },
        }),
      ),
  });
}

/** How often to ask again who is reading, in milliseconds. */
const WHILE_LOOKING = 30_000;

/** Who is reading now, and what. For the owner, while they look. */
export function useReadingNow() {
  return useQuery({
    queryKey: ["reading-now"],
    queryFn: async () => unwrap(await api.GET("/api/v1/reading-now")),
    refetchInterval: WHILE_LOOKING,
  });
}
