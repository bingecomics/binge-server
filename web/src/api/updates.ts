import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api, gotNoAnswer, type UpdateStatus, unwrap } from "./client";

const UPDATE = ["server-update"];

/** The question both hooks ask, so that they share one answer. */
const STATUS = {
  queryKey: UPDATE,
  queryFn: async () => unwrap(await api.GET("/api/v1/server/update")),
};

/** How often the server is asked how far an update is while one is under way, in milliseconds. */
const WHILE_UPDATING = 1_500;

/** How long an answer about updates is good for otherwise, in milliseconds. */
const GOOD_FOR = 5 * 60_000;

/**
 * Where the server stands with its own updates: the version that runs and the newer one its last
 * look found. For the owner; `asked` is whether to ask at all.
 */
export function useUpdateStatus(asked = true) {
  return useQuery({ ...STATUS, enabled: asked, staleTime: GOOD_FOR });
}

/** A version the server was brought to, and the page that says what changed in it. */
export type Updated = { version: string; notes: string };

/** Where the version an update was asked for is kept while the page loads again, in this tab. */
const ASKED_FOR = "binge.update-asked-for";

function remember(asked: Updated) {
  try {
    sessionStorage.setItem(ASKED_FOR, JSON.stringify(asked));
  } catch {
    // Storage is blocked: the page will not say that it updated. The version it shows still does.
  }
}

/** The version this tab asked the server to update to before the page loaded again. */
function remembered(): Updated | null {
  try {
    const kept: unknown = JSON.parse(sessionStorage.getItem(ASKED_FOR) ?? "null");
    if (kept && typeof kept === "object" && "version" in kept && "notes" in kept) {
      const { version, notes } = kept;
      return typeof version === "string" && typeof notes === "string" ? { version, notes } : null;
    }
  } catch {
    // Blocked, or not what was kept: nothing to say.
  }
  return null;
}

function forget() {
  try {
    sessionStorage.removeItem(ASKED_FOR);
  } catch {
    // Nothing was kept then.
  }
}

/**
 * The server's updates, for the page that manages them: where they stand, looking for a newer
 * version, having the server look every day, and bringing the server to the newer one.
 *
 * An update is followed to its end here. The server fetches the version, then starts again as it,
 * and does not answer for a moment while it does. When the version that answers is another than
 * the one the update was asked of, the page is loaded again, because the web app is part of the
 * server and is the old one until then. The page that loads then says once that the server was
 * updated. When the same version answers, the server says why in `problem` or `not_started`.
 */
export function useServerUpdate() {
  const queryClient = useQueryClient();
  // The version that ran when the update was asked for, while it is followed.
  const [from, setFrom] = useState<string | null>(null);
  // What this tab asked for before the page loaded again. It is said once: gone on the next load.
  const [askedFor] = useState(remembered);
  useEffect(forget, []);

  const status = useQuery({
    ...STATUS,
    staleTime: from == null ? GOOD_FOR : 0,
    // An answer that fails while the server starts again leaves the last good one in place.
    refetchInterval: from == null ? false : WHILE_UPDATING,
  });
  const keep = (answer: UpdateStatus) => queryClient.setQueryData<UpdateStatus>(UPDATE, answer);

  const look = useMutation({
    mutationFn: async () => unwrap(await api.POST("/api/v1/server/update/look")),
    onSuccess: keep,
  });
  const setLooks = useMutation({
    mutationFn: async (looks: boolean) =>
      unwrap(await api.PATCH("/api/v1/server/update", { body: { looks } })),
    onSuccess: keep,
  });
  const start = useMutation({
    // The version is the one the owner was shown, so that no other is fetched in its name.
    mutationFn: async (version: string) =>
      unwrap(await api.POST("/api/v1/server/update", { body: { version } })),
    onSuccess: async (answer) => {
      // An answer asked for before this one would say that nothing is under way.
      await queryClient.cancelQueries({ queryKey: UPDATE });
      if (answer.newer) {
        remember(answer.newer);
      }
      setFrom(answer.version);
      keep(answer);
    },
  });

  const now = status.data;
  useEffect(() => {
    if (from == null || !now) {
      return;
    }
    if (now.version !== from) {
      window.location.reload();
    } else if (now.stage === "idle") {
      // It stopped, or started again as what it was. The server says why.
      setFrom(null);
    }
  }, [from, now]);

  let updating: "fetching" | "restarting" | null = null;
  if (from != null) {
    // No answer while an update is followed is the server starting again.
    updating = now?.stage === "restarting" || gotNoAnswer(status.error) ? "restarting" : "fetching";
  }
  return {
    status: now,
    /** What went wrong asking the server, as distinct from what the server says went wrong. */
    error: from == null ? (status.error ?? look.error ?? setLooks.error ?? start.error) : null,
    updating,
    /** The version the server was just brought to from this tab, when it is the one that runs. */
    updated: askedFor && askedFor.version === now?.version ? askedFor : null,
    look,
    setLooks,
    start,
  };
}
