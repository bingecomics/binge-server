import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type DatabaseCopies, unwrap } from "./client";

const COPIES = ["database-copies"];

/** The copies of the database the server keeps, and whether it makes one a day. For the owner. */
export function useDatabaseCopies() {
  return useQuery({
    queryKey: COPIES,
    queryFn: async () => unwrap(await api.GET("/api/v1/server/backups")),
  });
}

/** Makes a copy of the database now. */
export function useMakeCopy() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => unwrap(await api.POST("/api/v1/server/backups")),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: COPIES }),
  });
}

/** Turns the daily copy on or off. */
export function useScheduleCopies() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (scheduled: boolean) =>
      unwrap(await api.PATCH("/api/v1/server/backups", { body: { scheduled } })),
    onSuccess: (copies) => queryClient.setQueryData<DatabaseCopies>(COPIES, copies),
  });
}
