import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { isShared, seriesCount } from "../format";
import { tell } from "../problems";
import {
  api,
  type Collection,
  type CollectionDetails,
  type NewCollection,
  type Sharing,
  sayProblem,
  unwrap,
} from "./client";
import { askAgainWithSearch, CONTENTS } from "./libraries";

/** Start of the key of everything asked about collections. Reading changes it, as it does the rest. */
const COLLECTIONS = [...CONTENTS, "collections"];

/**
 * The collections the reader sees, by name: their own, and the ones shared
 * with them that hold a series they may read.
 */
export function useCollections() {
  return useQuery({
    queryKey: COLLECTIONS,
    queryFn: async () => unwrap(await api.GET("/api/v1/collections")),
  });
}

/** The collections the reader sees that are rows on Home, by name. */
export function useCollectionsOnHome() {
  return useQuery({
    queryKey: [...COLLECTIONS, "on-home"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/collections", { params: { query: { on_home: true } } })),
  });
}

export function useCollection(id: number) {
  return useQuery({
    queryKey: [...COLLECTIONS, id],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/collections/{id}", { params: { path: { id } } })),
  });
}

/** The series of a collection, by name. */
export function useCollectionSeries(id: number) {
  return useQuery({
    queryKey: [...COLLECTIONS, id, "series"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/collections/{id}/series", { params: { path: { id } } })),
  });
}

/** The first series of a collection, by name, for its row on Home. */
export function useFirstCollectionSeries(id: number, limit: number) {
  return useQuery({
    queryKey: [...COLLECTIONS, id, "series", "first", limit],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/collections/{id}/series", {
          params: { path: { id }, query: { limit } },
        }),
      ),
  });
}

/** The collections a series is in. */
export function useCollectionsOfSeries(seriesId: number) {
  return useQuery({
    queryKey: [...COLLECTIONS, "of-series", seriesId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/series/{id}/collections", { params: { path: { id: seriesId } } }),
      ),
  });
}

/** Makes a collection of the reader's own, with series in it or none. */
export function useCreateCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (collection: NewCollection) =>
      unwrap(await api.POST("/api/v1/collections", { body: collection })),
    onSuccess: () => askAgainWithSearch(queryClient, COLLECTIONS),
  });
}

/** Gives a collection of the reader's another name and summary. */
export function useChangeCollection(id: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (details: CollectionDetails) =>
      unwrap(
        await api.PATCH("/api/v1/collections/{id}", { params: { path: { id } }, body: details }),
      ),
    onSuccess: () => askAgainWithSearch(queryClient, COLLECTIONS),
  });
}

/** Says who a collection of the reader's is shared with from now on. */
export function useShareCollection(id: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (sharing: Sharing) =>
      unwrap(
        await api.PUT("/api/v1/collections/{id}/sharing", {
          params: { path: { id } },
          body: sharing,
        }),
      ),
    onSuccess: () => askAgainWithSearch(queryClient, COLLECTIONS),
  });
}

/**
 * Makes a collection of the reader's a row on Home, or no longer one, and
 * says so over the page: nothing on the collection's own page moves.
 */
export function usePutCollectionOnHome(id: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (onHome: boolean) =>
      unwrap(
        await api.PUT("/api/v1/collections/{id}/on-home", {
          params: { path: { id } },
          body: { on_home: onHome },
        }),
      ),
    onSuccess: (changed) => {
      const shared = isShared(changed);
      tell(
        changed.on_home
          ? `“${changed.name}” is a row on Home${shared ? ", for you and everyone who sees it" : ""}.`
          : `“${changed.name}” is no longer a row on Home.`,
      );
      return askAgainWithSearch(queryClient, COLLECTIONS);
    },
    onError: sayProblem,
  });
}

/**
 * Deletes a collection of the reader's. Its series stay. `onDeleted` runs first and the
 * collections are refreshed after, so the collection's page can be left before it is gone.
 */
export function useDeleteCollection(onDeleted: () => unknown) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/collections/{id}", { params: { path: { id } } })),
    onSuccess: async () => {
      await onDeleted();
      await askAgainWithSearch(queryClient, COLLECTIONS);
    },
  });
}

/**
 * Puts series in a collection, leaving out the ones it holds already, and
 * says over the page how many went in: nothing where the person asked
 * shows the collection. Only the one whose collection it is can.
 */
export function useAddToCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ collection, series }: { collection: Collection; series: number[] }) =>
      unwrap(
        await api.POST("/api/v1/collections/{id}/series", {
          params: { path: { id: collection.id } },
          body: { series },
        }),
      ),
    onSuccess: (added, { collection }) => {
      tell(
        added.series > 0
          ? `Added ${seriesCount(added.series)} to “${collection.name}”.`
          : `“${collection.name}” already holds all of that.`,
      );
      return askAgainWithSearch(queryClient, COLLECTIONS);
    },
  });
}

/** Takes a series out of a collection of the reader's. */
export function useRemoveFromCollection(id: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (seriesId: number) =>
      unwrap(
        await api.DELETE("/api/v1/collections/{id}/series/{series_id}", {
          params: { path: { id, series_id: seriesId } },
        }),
      ),
    onSuccess: () => askAgainWithSearch(queryClient, COLLECTIONS),
    onError: sayProblem,
  });
}
