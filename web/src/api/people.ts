import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api, unwrap } from "./client";
import { CONTENTS } from "./libraries";

/** How many of a person's books are asked for at once. */
const BOOKS_AT_ONCE = 60;

/**
 * The people one of whose names holds the given words, the ones most books
 * name first: `limit` of them at most. Asks nothing without words.
 */
export function usePeopleNamed(words: string, limit: number) {
  return useQuery({
    queryKey: [...CONTENTS, "people", words, limit],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/people", { params: { query: { q: words, limit } } })),
    enabled: words !== "",
    // Who was found stays up while the next ones are looked for.
    placeholderData: keepPreviousData,
  });
}

/** One person with the names they go by. */
export function usePerson(id: number) {
  return useQuery({
    queryKey: [...CONTENTS, "person", id],
    queryFn: async () => unwrap(await api.GET("/api/v1/people/{id}", { params: { path: { id } } })),
  });
}

/** The series that hold a book that names the person. */
export function useSeriesOfPerson(id: number) {
  return useQuery({
    queryKey: [...CONTENTS, "person", id, "series"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/people/{id}/series", { params: { path: { id } } })),
  });
}

/** The books that name the person, from the first that came out, a part at a time. */
export function useBooksOfPerson(id: number) {
  return useInfiniteQuery({
    queryKey: [...CONTENTS, "person", id, "books"],
    queryFn: async ({ pageParam }) =>
      unwrap(
        await api.GET("/api/v1/people/{id}/books", {
          params: { path: { id }, query: { offset: pageParam, limit: BOOKS_AT_ONCE } },
        }),
      ),
    initialPageParam: 0,
    getNextPageParam: (last, parts) => {
      const shown = parts.reduce((count, part) => count + part.books.length, 0);
      return last.books.length > 0 && shown < last.total ? shown : undefined;
    },
  });
}

/** Who a book names, as people. */
export function useBookCredits(bookId: number) {
  return useQuery({
    queryKey: [...CONTENTS, "book", bookId, "credits"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/books/{id}/credits", { params: { path: { id: bookId } } })),
  });
}

/**
 * What changes who a person is: every page that names people is asked for
 * again after it.
 */
function usePersonChange<Change, Answer>(change: (change: Change) => Promise<Answer>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: change,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: CONTENTS }),
  });
}

/** Makes other people one with this one: their names become names of this one. */
export function useMergePeople(id: number) {
  return usePersonChange(async (people: number[]) =>
    unwrap(
      await api.POST("/api/v1/people/{id}/merge", { params: { path: { id } }, body: { people } }),
    ),
  );
}

/** Gives a person one more name. */
export function useAddPersonName(id: number) {
  return usePersonChange(async (name: string) =>
    unwrap(
      await api.POST("/api/v1/people/{id}/names", { params: { path: { id } }, body: { name } }),
    ),
  );
}

/** Shows a person by another of their names, given by its id. */
export function useShowPersonBy(id: number) {
  return usePersonChange(async (nameId: number) =>
    unwrap(
      await api.PATCH("/api/v1/people/{id}", {
        params: { path: { id } },
        body: { shown_by: nameId },
      }),
    ),
  );
}

/** Makes one of a person's names, given by its id, a person of its own again. */
export function useSplitPerson(id: number) {
  return usePersonChange(async (nameId: number) =>
    unwrap(
      await api.DELETE("/api/v1/people/{id}/names/{name}", {
        params: { path: { id, name: nameId } },
      }),
    ),
  );
}
