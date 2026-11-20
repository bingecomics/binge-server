import { createFileRoute } from "@tanstack/react-router";
import { AccountSection } from "../account/AccountSection";

export const Route = createFileRoute("/settings/account")({ component: AccountSection });
