import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ME } from "./api/account";
import { isSignedOut } from "./api/client";
import { showChosenCoverSize } from "./coverSize";
import { stringifySearch } from "./narrowing";
import { routeTree } from "./routeTree.gen";
import "./styles.css";

/**
 * A session can end while the app is open: it was left unused too long, or
 * the password was changed elsewhere. Whatever request finds out leads back
 * to signing in, where the page being looked at is kept.
 */
function noticeSignedOut(error: unknown) {
  if (isSignedOut(error)) {
    queryClient.setQueryData(ME, null);
  }
}

// No retries: the server is close by, and when it says no (a series that is
// gone, say) asking again only delays telling the person.
const queryClient = new QueryClient({
  queryCache: new QueryCache({ onError: noticeSignedOut }),
  mutationCache: new MutationCache({ onError: noticeSignedOut }),
  defaultOptions: { queries: { retry: false } },
});
const router = createRouter({ routeTree, defaultPreload: "intent", stringifySearch });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

showChosenCoverSize();

const root = document.getElementById("root");
if (!root) {
  throw new Error("index.html has no #root element");
}

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
