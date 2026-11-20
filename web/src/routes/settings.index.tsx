import { createFileRoute, redirect } from "@tanstack/react-router";

/** The settings open on the person's own account. */
export const Route = createFileRoute("/settings/")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/account", replace: true });
  },
});
