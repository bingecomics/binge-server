import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, unwrap } from "./client";

const SETTINGS = ["settings"];

type Settings = Record<string, string>;

/**
 * What the person signed in chose, kept by the server so that it follows
 * them to another browser: each value under its key. Asked for once; what
 * is chosen here afterwards is put in by `useKeepSetting`.
 */
export function useSettings() {
  return useQuery({
    queryKey: SETTINGS,
    queryFn: async (): Promise<Settings> => unwrap(await api.GET("/api/v1/me/settings")).settings,
    staleTime: Number.POSITIVE_INFINITY,
  });
}

/**
 * Keeps a value under a key. What the page shows follows at once, and goes
 * back to what the server has when the server refuses.
 */
export function useKeepSetting() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ key, value }: { key: string; value: string }) =>
      unwrap(
        await api.PUT("/api/v1/me/settings/{key}", { params: { path: { key } }, body: { value } }),
      ),
    onMutate: ({ key, value }) => {
      queryClient.setQueryData<Settings>(SETTINGS, (kept) => ({ ...kept, [key]: value }));
    },
    onError: () => queryClient.invalidateQueries({ queryKey: SETTINGS }),
  });
}
