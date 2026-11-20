import { type QueryClient, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  type Credentials,
  type Health,
  type NewUser,
  type User,
  type UserChange,
  unwrap,
} from "./client";
import { HEALTH } from "./health";

export const ME = ["me"];
const TOKENS = ["tokens"];
const USERS = ["users"];

/**
 * Who is signed in in this browser: `null` when nobody is. Asked once, and
 * again each time the window is looked at again, so a session that ended
 * elsewhere is noticed; not by every part of the page that wants to know.
 */
export function useMe() {
  return useQuery({
    queryKey: ME,
    queryFn: async (): Promise<User | null> => {
      const result = await api.GET("/api/v1/me");
      return result.response.status === 401 ? null : unwrap(result);
    },
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: "always",
  });
}

/** Whether the person signed in is the server's owner, who manages libraries and users. */
export function useIsOwner(): boolean {
  return useMe().data?.role === "owner";
}

/** Lets a user who has just signed in into the app. */
function welcome(queryClient: QueryClient, user: User) {
  // What someone else looked at in this tab before is not this user's to see.
  queryClient.removeQueries({
    predicate: (query) => query.queryKey[0] !== ME[0] && query.queryKey[0] !== HEALTH[0],
  });
  queryClient.setQueryData<Health>(HEALTH, (health) => health && { ...health, needs_setup: false });
  queryClient.setQueryData(ME, user);
}

/** First-run setup: makes the server's owner and signs them in. */
export function useSetUp() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (owner: Credentials) =>
      unwrap(await api.POST("/api/v1/setup", { body: owner })),
    onSuccess: (user) => welcome(queryClient, user),
  });
}

export function useSignIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (shown: Credentials) =>
      unwrap(await api.POST("/api/v1/sign-in", { body: shown })),
    onSuccess: (user) => welcome(queryClient, user),
  });
}

/**
 * Signs out and loads the app again from the start, so that nothing the user
 * looked at stays in this tab's memory.
 */
export function useSignOut() {
  return useMutation({
    mutationFn: async () => {
      const result = await api.POST("/api/v1/sign-out");
      // A session that had already ended is signed out all the same.
      if (result.response.status !== 401) {
        unwrap(result);
      }
      window.location.assign("/");
    },
  });
}

export function useChangePassword() {
  return useMutation({
    mutationFn: async (change: { current: string; new: string }) =>
      unwrap(await api.PUT("/api/v1/me/password", { body: change })),
  });
}

/** The tokens the signed-in user's apps use. */
export function useTokens() {
  return useQuery({
    queryKey: TOKENS,
    queryFn: async () => unwrap(await api.GET("/api/v1/tokens")),
  });
}

/** Makes a token. What comes back holds its secret, which is shown this once. */
export function useCreateToken() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (name: string) =>
      unwrap(await api.POST("/api/v1/tokens", { body: { name } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: TOKENS }),
  });
}

export function useRevokeToken() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/tokens/{id}", { params: { path: { id } } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: TOKENS }),
  });
}

/** Everyone who may use the server. For the owner. */
export function useUsers() {
  return useQuery({
    queryKey: USERS,
    queryFn: async () => unwrap(await api.GET("/api/v1/users")),
  });
}

export function useAddUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (user: NewUser) => unwrap(await api.POST("/api/v1/users", { body: user })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: USERS }),
  });
}

export function useChangeUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, change }: { id: number; change: UserChange }) =>
      unwrap(await api.PATCH("/api/v1/users/{id}", { params: { path: { id } }, body: change })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: USERS }),
  });
}

export function useRemoveUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: number) =>
      unwrap(await api.DELETE("/api/v1/users/{id}", { params: { path: { id } } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: USERS }),
  });
}
