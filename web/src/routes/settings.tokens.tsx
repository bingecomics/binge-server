import { createFileRoute } from "@tanstack/react-router";
import { TokensSection } from "../account/TokensSection";

export const Route = createFileRoute("/settings/tokens")({ component: TokensSection });
