import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type Asked, askedIn } from "../narrowing";
import { api, type NewView, unwrap, type View, type ViewChange } from "./client";
import { CONTENTS } from "./libraries";

const VIEWS = ["views"];

/** How many series a view's row on Home shows. The rest are a click away. */
export const SERIES_IN_A_ROW = 24;

/**
 * The views of the libraries the reader may read: each a library narrowed
 * and ordered, kept under a name.
 */
export function useViews() {
  return useQuery({
    queryKey: VIEWS,
    queryFn: async () => unwrap(await api.GET("/api/v1/views")),
  });
}

/** What a view asks of its library, as the library's page keeps it in its address. */
export function askedBy(view: View): Asked {
  return askedIn(view.asked);
}

/** Where a view leads: its library, showing what the view asks. */
export function linkTo(view: View) {
  return {
    to: "/libraries/$libraryId",
    params: { libraryId: view.library_id },
    search: askedBy(view),
  } as const;
}

/** What a view is asked to keep of what a library's page shows: all but the page of it. */
export function keptOf(asked: Asked): NonNullable<NewView["asked"]> {
  const { page, ...kept } = asked;
  return kept;
}

/** The first series of a view, for its row on Home. */
export function useViewSeries(view: View) {
  return useQuery({
    queryKey: [...CONTENTS, "view", view.id, view.asked],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/series", {
          params: {
            path: { id: view.library_id },
            query: { ...keptOf(askedBy(view)), limit: SERIES_IN_A_ROW },
          },
        }),
      ),
  });
}

/** Keeps a view. The owner's to do. */
export function useAddView() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (view: NewView) => unwrap(await api.POST("/api/v1/views", { body: view })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: VIEWS }),
  });
}

/** Changes a view: its name, what it asks, or where it shows. The owner's to do. */
export function useChangeView() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, change }: { id: number; change: ViewChange }) =>
      unwrap(await api.PATCH("/api/v1/views/{id}", { params: { path: { id } }, body: change })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: VIEWS }),
  });
}

/** Deletes a view. Its library is not touched. The owner's to do. */
export function useDeleteView() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/views/{id}", { params: { path: { id } } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: VIEWS }),
  });
}
