import { useMutation, useQueryClient } from "@tanstack/react-query";
import { tell } from "../problems";
import { api, fileBody, sayProblem, unwrap } from "./client";
import { CONTENTS } from "./libraries";

/** What a cover is chosen for. */
export type CoverOf = { kind: "books" | "series" | "lists"; id: number };

/** What a cover is made from: a page of a book, from 0, or a picture's file. */
export type CoverFrom = { bookId: number; page: number } | File;

/** The kinds of picture the server makes a cover from, for a file field. */
export const PICTURES = "image/jpeg,image/png,image/webp,image/gif";

/** The biggest picture the server takes, in megabytes. */
const BIGGEST = 20;

const PATHS = {
  books: { cover: "/api/v1/books/{id}/cover", picture: "/api/v1/books/{id}/cover/picture" },
  series: { cover: "/api/v1/series/{id}/cover", picture: "/api/v1/series/{id}/cover/picture" },
  lists: {
    cover: "/api/v1/reading-lists/{id}/cover",
    picture: "/api/v1/reading-lists/{id}/cover/picture",
  },
} as const;

type Choice = {
  of: CoverOf;
  from: CoverFrom;
  /** What to say once the server has it, where nothing shows the new cover. */
  done?: string;
};

async function choose({ of, from }: Choice) {
  const params = { path: { id: of.id } };
  if (!(from instanceof File)) {
    const body = { book_id: from.bookId, page: from.page };
    return unwrap(await api.PUT(PATHS[of.kind].cover, { params, body }));
  }
  return unwrap(
    await api.PUT(PATHS[of.kind].picture, {
      params,
      ...fileBody(from, "picture", BIGGEST, from.type || "application/octet-stream"),
    }),
  );
}

/**
 * Gives a book, a series or a reading list the cover chosen for it. Only the
 * owner can, and for a reading list only one of their own.
 * Every list that shows the cover is asked for again, and what goes wrong
 * is said over the page: the menu that asked is closed by then.
 */
export function useChooseCover() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: choose,
    onSuccess: (_, { done }) => {
      if (done) {
        tell(done);
      }
      return queryClient.invalidateQueries({ queryKey: CONTENTS });
    },
    onError: sayProblem,
  });
}

/**
 * Takes back the cover chosen for a book, a series or a reading list. Only the
 * owner can, and for a reading list only one of their own.
 */
export function useForgetCover() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (of: CoverOf) =>
      unwrap(await api.DELETE(PATHS[of.kind].cover, { params: { path: { id: of.id } } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: CONTENTS }),
    onError: sayProblem,
  });
}
