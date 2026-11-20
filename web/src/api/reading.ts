import { type QueryClient, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { api, gotNoAnswer, type ReadMark, reason, sayProblem, unwrap } from "./client";
import { CONTENTS } from "./libraries";

/** How many covers each shelf of the home page asks for. */
const SHELF = 24;

/**
 * What the reader needs is asked for each time a book is opened and not kept
 * after it is closed, so the book always opens where the server says it was
 * left, also when it was last read somewhere else.
 */
const whileOpen = { gcTime: 0, refetchOnWindowFocus: false } as const;

/** The key of what `useUpNext` asks for. */
const UP_NEXT = [...CONTENTS, "up-next"];

/** One book, for reading it. */
export function useBook(id: number) {
  return useQuery({
    queryKey: ["reading", "book", id],
    queryFn: async () => {
      // A book closed and opened again at once must not be asked for before
      // its last page turn has reached the server.
      await savers.get(id)?.running;
      return unwrap(await api.GET("/api/v1/books/{id}", { params: { path: { id } } }));
    },
    ...whileOpen,
  });
}

/** The pages of a book, with their sizes. */
export function usePages(bookId: number) {
  return useQuery({
    queryKey: ["reading", "pages", bookId],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/books/{id}/pages", { params: { path: { id: bookId } } })),
    ...whileOpen,
  });
}

/** Books that were started and not finished, the one read last first. */
export function useContinueReading() {
  return useQuery({
    queryKey: [...CONTENTS, "continue-reading"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/continue-reading", { params: { query: { limit: SHELF } } })),
  });
}

/** The books the server saw last, newest first. */
export function useRecentlyAdded() {
  return useQuery({
    queryKey: [...CONTENTS, "recently-added"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/recently-added", { params: { query: { limit: SHELF } } })),
  });
}

/**
 * The book that follows in each series where one was finished and none
 * opened since, the series read last first.
 */
export function useUpNext() {
  return useQuery({
    queryKey: UP_NEXT,
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/up-next", { params: { query: { limit: SHELF } } })),
  });
}

/** Takes a series off "Up next", until the reader reads from it again. */
export function useTakeOffUpNext() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (seriesId: number) =>
      unwrap(
        await api.DELETE("/api/v1/up-next/{series_id}", {
          params: { path: { series_id: seriesId } },
        }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: UP_NEXT }),
    onError: sayProblem,
  });
}

/** The series that hold books, the one given a book last first. */
export function useRecentlyUpdated() {
  return useQuery({
    queryKey: [...CONTENTS, "recently-updated"],
    queryFn: async () =>
      unwrap(await api.GET("/api/v1/recently-updated", { params: { query: { limit: SHELF } } })),
  });
}

/**
 * Which books to mark, or to add to a reading list: these books, and every
 * book of these groups, series and reading lists.
 */
export type BooksToMark = Omit<ReadMark, "read">;

/**
 * Marks books as read, or as unread. Every list that shows them is asked for
 * again, and what goes wrong is said over the page: a menu that asked for
 * the mark is closed by then. `onMarked` runs once the server has it.
 */
export function useMarkRead(onMarked?: () => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (mark: ReadMark) =>
      unwrap(await api.POST("/api/v1/read-marks", { body: mark })),
    onSuccess: () => {
      onMarked?.();
      return queryClient.invalidateQueries({ queryKey: CONTENTS });
    },
    onError: sayProblem,
  });
}

/**
 * Why a page image did not load, in the server's words. An image element
 * only says that it failed, so the page is asked for once more to hear why.
 * `null` when the server has nothing against the page.
 */
export function usePageProblem(identity: string, number: number) {
  return useQuery({
    queryKey: ["reading", "page-problem", identity, number],
    queryFn: async () => {
      try {
        const result = await api.GET("/api/v1/pages/{identity}/{number}", {
          params: { path: { identity, number } },
          parseAs: "stream",
        });
        unwrap(result);
        // The image itself is not wanted here.
        void result.response.body?.cancel();
        return null;
      } catch (error) {
        return reason(error);
      }
    },
    ...whileOpen,
  });
}

/** How often a save that got no answer is tried again, and how long after, in milliseconds. */
const RETRIES = 5;
const RETRY_AFTER = 3000;

/**
 * The saving of one book's progress. There is one for each book and not one
 * for each time it is opened, so a book closed and opened again goes on where
 * the saving was, and what an earlier reader left unsent cannot land after
 * what a later one saved.
 */
type Saver = {
  /** The page to save, until the server has it. */
  waiting?: number;
  /** The save that is on its way. */
  running?: Promise<void>;
  retries: number;
  /** Tells the reader that is open what went wrong, or that nothing did. */
  report?: (problem: string | null) => void;
};

const savers = new Map<number, Saver>();

function saverOf(bookId: number): Saver {
  let saver = savers.get(bookId);
  if (!saver) {
    saver = { retries: 0 };
    savers.set(bookId, saver);
  }
  return saver;
}

async function send(bookId: number, page: number) {
  unwrap(
    await api.PUT("/api/v1/books/{id}/progress", {
      params: { path: { id: bookId } },
      body: { page },
      // Lets the request finish when the tab is closed right after.
      keepalive: true,
    }),
  );
}

/** Sends what waits, one save at a time, until the server has the latest page. */
function run(bookId: number, saver: Saver, queryClient: QueryClient) {
  if (saver.running || saver.waiting === undefined) {
    return;
  }
  saver.running = (async () => {
    try {
      while (saver.waiting !== undefined) {
        const page = saver.waiting;
        await send(bookId, page);
        // A page turned to meanwhile still waits.
        if (saver.waiting === page) {
          saver.waiting = undefined;
        }
      }
      saver.report?.(null);
      // Every list that shows this book now shows old progress.
      void queryClient.invalidateQueries({ queryKey: CONTENTS });
    } catch (error) {
      saver.report?.(reason(error));
      // A server that said nothing may be back in a moment. One that said no
      // will say no again, so the page is let go.
      if (!gotNoAnswer(error)) {
        saver.waiting = undefined;
      } else if (saver.retries < RETRIES) {
        saver.retries += 1;
        setTimeout(() => run(bookId, saver, queryClient), RETRY_AFTER);
      }
    } finally {
      saver.running = undefined;
    }
  })();
}

/**
 * Saves how far a book has been read.
 *
 * Returns `save`, to call with the last page shown, and what went wrong with
 * the last save, if anything. Saves go out one at a time and only the latest
 * page matters, so turning pages quickly cannot leave an older page saved
 * last. A save is finished even when the reader is closed, sent at once when
 * the tab is, and tried again for a while when the server does not answer.
 */
export function useSaveProgress(bookId: number) {
  const queryClient = useQueryClient();
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    const saver = saverOf(bookId);
    saver.report = setProblem;
    // A page that waits behind a save in flight, or for the next try, would
    // be lost with the tab.
    function sendNow() {
      if (saver.waiting !== undefined) {
        void send(bookId, saver.waiting).catch(() => {});
      }
    }
    window.addEventListener("pagehide", sendNow);
    return () => {
      saver.report = undefined;
      window.removeEventListener("pagehide", sendNow);
    };
  }, [bookId]);

  const save = useCallback(
    (page: number) => {
      const saver = saverOf(bookId);
      saver.waiting = page;
      saver.retries = 0;
      run(bookId, saver, queryClient);
    },
    [bookId, queryClient],
  );

  return { save, problem };
}
