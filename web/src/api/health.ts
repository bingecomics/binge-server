import { useQuery } from "@tanstack/react-query";
import { api, unwrap } from "./client";

export const HEALTH = ["health"];

/** How often the server is asked how it is, in milliseconds. */
const REFRESH = 15_000;

/** The server's health, refreshed in the background so the UI notices outages. */
export function useHealth() {
  return useQuery({
    queryKey: HEALTH,
    queryFn: async () => unwrap(await api.GET("/api/v1/health")),
    refetchInterval: REFRESH,
    // The parts of the page that show it share one answer.
    staleTime: REFRESH,
  });
}
