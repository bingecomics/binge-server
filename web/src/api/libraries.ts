import {
  keepPreviousData,
  type QueryClient,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { type Asked, lettersQuery, seriesQuery } from "../narrowing";
import {
  api,
  type ChangeKind,
  type FromFiles,
  type Library,
  type NewLibrary,
  unwrap,
} from "./client";

export function isScanning(library: Library | undefined): boolean {
  return library !== undefined && library.scan.phase !== "idle";
}

const LIBRARIES = ["libraries"];

/**
 * Start of the key of every list of series or books. A scan changes them, and
 * so does reading, which moves a book's progress.
 */
export const CONTENTS = ["contents"];

/**
 * Start of the key of what scans changed. It is written down once a scan,
 * so it is asked for again when one ends and not while it runs.
 */
const SCANS = ["scans"];

/** How often to look again while a scan runs, in milliseconds. */
const WHILE_SCANNING = 1000;

/** Changes whenever a scan of any library ends. */
function scanTimes(libraries: Library[]): string {
  return libraries.map((library) => `${library.id}:${library.last_scanned_at}`).join();
}

/**
 * Every library with the state of its scan, refreshed while any scan runs.
 *
 * This is also what keeps series and books up to date during a scan: they are
 * asked for again whenever a look at the libraries finds a scan running or
 * just ended.
 */
export function useLibraries() {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: LIBRARIES,
    queryFn: async () => {
      const before = queryClient.getQueryData<Library[]>(LIBRARIES);
      const libraries = unwrap(await api.GET("/api/v1/libraries"));
      const ended = before !== undefined && scanTimes(before) !== scanTimes(libraries);
      if (before?.some(isScanning) || ended) {
        void queryClient.invalidateQueries({ queryKey: CONTENTS });
      }
      if (ended || (before?.some(isScanning) && !libraries.some(isScanning))) {
        void queryClient.invalidateQueries({ queryKey: SCANS });
      }
      return libraries;
    },
    refetchInterval: (query) => (query.state.data?.some(isScanning) ? WHILE_SCANNING : false),
  });
}

/** One library, taken from the list so every view shares one refresh. */
export function useLibrary(id: number | undefined) {
  const libraries = useLibraries();
  return {
    library: libraries.data?.find((library) => library.id === id),
    isPending: libraries.isPending,
    isError: libraries.isError,
  };
}

/**
 * A page of the series of a library: those what is asked leaves in, in the
 * order asked for. The page shown stays up while the next one is fetched.
 */
export function useSeriesPage(libraryId: number, asked: Asked) {
  return useQuery({
    queryKey: [...CONTENTS, "library", libraryId, "series", asked],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/series", {
          params: { path: { id: libraryId }, query: seriesQuery(asked) },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

/** The letters the names of a library's series start with, among those what is asked leaves in. */
export function useSeriesLetters(libraryId: number, asked: Asked, enabled: boolean) {
  const query = lettersQuery(asked);
  return useQuery({
    queryKey: [...CONTENTS, "library", libraryId, "letters", query],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/series/letters", {
          params: { path: { id: libraryId }, query },
        }),
      ),
    enabled,
    placeholderData: keepPreviousData,
  });
}

/**
 * What the series of a library can be narrowed by. It is every publisher
 * and person of the library, so it is asked for only when it shows, and
 * not again each time the window is come back to.
 */
export function useLibraryChoices(libraryId: number, enabled: boolean) {
  return useQuery({
    queryKey: [...CONTENTS, "library", libraryId, "choices"],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/choices", { params: { path: { id: libraryId } } }),
      ),
    enabled,
    staleTime: 60_000,
  });
}

/** The books a library holds more than once, each with its files. For the owner. */
export function useHeldTwice(libraryId: number, enabled: boolean) {
  return useQuery({
    queryKey: [...CONTENTS, "library", libraryId, "duplicates"],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/duplicates", {
          params: { path: { id: libraryId } },
        }),
      ),
    enabled,
  });
}

/**
 * The files of a library that could not be read: the ones still to look
 * at, or the ones the owner set aside. They are in no series.
 */
export function useUnreadableFiles(libraryId: number, setAside = false, enabled = true) {
  return useQuery({
    queryKey: [...CONTENTS, "library", libraryId, "unreadable", setAside],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/unreadable", {
          params: { path: { id: libraryId }, query: { set_aside: setAside } },
        }),
      ),
    enabled,
  });
}

/**
 * Sets a file that could not be read aside, or takes it back. The library
 * counts the ones still to look at.
 */
export function useSetAside() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, aside }: { id: number; aside: boolean }) => {
      const at = { params: { path: { id } } };
      return unwrap(
        aside
          ? await api.PUT("/api/v1/books/{id}/set-aside", at)
          : await api.DELETE("/api/v1/books/{id}/set-aside", at),
      );
    },
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: LIBRARIES }),
        queryClient.invalidateQueries({ queryKey: CONTENTS }),
      ]),
  });
}

/** The scans that changed what a library holds, the latest first. For the owner. */
export function useScans(libraryId: number, enabled: boolean) {
  return useQuery({
    queryKey: [...SCANS, "library", libraryId],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/libraries/{id}/scans", { params: { path: { id: libraryId } } }),
      ),
    enabled,
  });
}

/** How many changes of a scan are shown at once. */
export const CHANGES_A_PAGE = 100;

/** A page of what one scan changed, of one kind or of all. For the owner. */
export function useScanChanges(scanId: number, kind: ChangeKind | undefined, page: number) {
  return useQuery({
    queryKey: [...SCANS, scanId, kind ?? "all", page],
    queryFn: async () =>
      unwrap(
        await api.GET("/api/v1/scans/{id}/changes", {
          params: {
            path: { id: scanId },
            query: { kind, offset: (page - 1) * CHANGES_A_PAGE, limit: CHANGES_A_PAGE },
          },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

/** One series seen whole: what its books say about it, and how far the reader is through it. */
export function useSeriesOverview(id: number) {
  return useQuery({
    queryKey: [...CONTENTS, "series", id, "overview"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/series/{id}/overview", { params: { path: { id } } })),
  });
}

/**
 * One book, for showing what is known about it. Kept apart from what the
 * reader asks for (`useBook`), which must come fresh from the server each
 * time a book is opened: a copy kept here would open the book where it was
 * left when this copy was made.
 */
export function useBookDetails(id: number) {
  return useQuery({
    queryKey: [...CONTENTS, "book", id],
    queryFn: async () => unwrap(await api.GET("/api/v1/books/{id}", { params: { path: { id } } })),
  });
}

/** The books of a series. */
export function useBooks(seriesId: number) {
  return useQuery({
    queryKey: [...CONTENTS, "series", seriesId, "books"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/series/{id}/books", { params: { path: { id: seriesId } } })),
  });
}

/** A group of a series seen whole: its series, and how far the reader is through it. */
export function useGroupOverview(id: number) {
  return useQuery({
    queryKey: [...CONTENTS, "group", id, "overview"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/groups/{id}/overview", { params: { path: { id } } })),
  });
}

/** The books of a group, in the order they are read. */
export function useGroupBooks(groupId: number) {
  return useQuery({
    queryKey: [...CONTENTS, "group", groupId, "books"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/groups/{id}/books", { params: { path: { id: groupId } } })),
  });
}

/** Under what every search is kept. */
const SEARCH = [...CONTENTS, "search"];

/**
 * Asks again for everything kept under `key`, and for what a search found:
 * a search finds reading lists and collections, so a change to one of them
 * changes what it shows.
 */
export function askAgainWithSearch(queryClient: QueryClient, key: string[]) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: key }),
    queryClient.invalidateQueries({ queryKey: SEARCH }),
  ]);
}

/**
 * The series, books, reading lists, collections and people that hold every one of the
 * words: `limit` of each at most, or as many as the server gives. Asks
 * nothing without words.
 */
export function useSearch(words: string, limit?: number) {
  return useQuery({
    queryKey: [...SEARCH, words, limit],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/search", { params: { query: { q: words, limit } } })),
    enabled: words !== "",
    // What was found stays up while the next search runs.
    placeholderData: keepPreviousData,
  });
}

export function useAddLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (library: NewLibrary) =>
      unwrap(await api.POST("/api/v1/libraries", { body: library })),
    // Waits for the list, so the new library's page finds it there.
    onSuccess: () => queryClient.invalidateQueries({ queryKey: LIBRARIES }),
  });
}

/**
 * Says what a library makes from what its files say. What is turned on is
 * made at once, so the reading lists and the collections are asked for
 * again: they are among what a library holds.
 */
export function useMakeFromFiles() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, from }: { id: number; from: FromFiles }) =>
      unwrap(
        await api.PUT("/api/v1/libraries/{id}/from-files", {
          params: { path: { id } },
          body: from,
        }),
      ),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: LIBRARIES }),
        queryClient.invalidateQueries({ queryKey: CONTENTS }),
      ]),
  });
}

/**
 * Removes a library. `onRemoved` runs first and the list is refreshed after,
 * so a page showing the library can be left before the library is gone.
 */
export function useRemoveLibrary(onRemoved: () => unknown) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/libraries/{id}", { params: { path: { id } } })),
    onSuccess: async () => {
      await onRemoved();
      await queryClient.invalidateQueries({ queryKey: LIBRARIES });
    },
  });
}

/**
 * Deletes a series that holds no books. `onDeleted` runs first and the lists
 * are refreshed after, so the series' page can be left before it is gone.
 */
export function useDeleteSeries(onDeleted: () => unknown) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/series/{id}", { params: { path: { id } } })),
    onSuccess: async () => {
      await onDeleted();
      // The library counts its series.
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: LIBRARIES }),
        queryClient.invalidateQueries({ queryKey: CONTENTS }),
      ]);
    },
  });
}

/**
 * Deletes a group that holds no books. `onDeleted` runs first and the lists
 * are refreshed after, so the group's page can be left before it is gone.
 */
export function useDeleteGroup(onDeleted: () => unknown) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/groups/{id}", { params: { path: { id } } })),
    onSuccess: async () => {
      await onDeleted();
      await queryClient.invalidateQueries({ queryKey: CONTENTS });
    },
  });
}

export function useScanLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.POST("/api/v1/libraries/{id}/scan", { params: { path: { id } } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: LIBRARIES }),
  });
}
