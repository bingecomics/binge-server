import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BringStatus, unwrap } from "./client";

const BRING = ["bring"];

/** How often the server is asked how far it is while it reads the other one, in milliseconds. */
const WHILE_READING = 1_500;

/**
 * Where the owner stands with the server they are leaving: whether it is being read, why the last
 * reading stopped, and what the last one that was done found. Asked again by itself while it is
 * being read.
 */
export function useBringing() {
  return useQuery({
    queryKey: BRING,
    queryFn: async () => unwrap(await api.GET("/api/v1/bring")),
    refetchInterval: (query) => (query.state.data?.reading ? WHILE_READING : false),
  });
}

/** Starts reading a Kavita server with the owner's key there. The key is not kept. */
export function useStartBringing() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ address, key }: { address: string; key: string }) =>
      unwrap(await api.POST("/api/v1/bring", { body: { from: "kavita", address, key } })),
    onSuccess: (status) => queryClient.setQueryData<BringStatus>(BRING, status),
  });
}

/** Says which library here holds the folder of a library of the other server, or that none does. */
export function useSayBroughtLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ theirId, libraryId }: { theirId: number; libraryId: number | null }) =>
      unwrap(
        await api.PUT("/api/v1/bring/libraries/{their_id}", {
          params: { path: { their_id: theirId } },
          body: { library_id: libraryId },
        }),
      ),
    onSuccess: (status) => queryClient.setQueryData<BringStatus>(BRING, status),
  });
}

/** Forgets what was read from the other server. */
export function useForgetBrought() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => unwrap(await api.DELETE("/api/v1/bring")),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: BRING }),
  });
}
