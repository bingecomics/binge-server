import { createFileRoute, Navigate } from "@tanstack/react-router";
import { UsersSection } from "../account/UsersSection";
import { useIsOwner } from "../api/account";

export const Route = createFileRoute("/settings/users")({ component: UsersPage });

/** The people of the server, who are the owner's to see. Anyone else is led to their own account. */
function UsersPage() {
  return useIsOwner() ? <UsersSection /> : <Navigate to="/settings/account" replace />;
}
