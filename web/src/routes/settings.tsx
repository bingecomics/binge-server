import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/settings")({ component: SettingsPages });

/**
 * The settings, a page for each part of them. Their menu takes the place of
 * the one beside the page: see `SettingsMenu`.
 */
function SettingsPages() {
  return (
    <>
      {/* Their menu already says these are the settings, and which page of them. */}
      <h1 className="sr-only">Settings</h1>
      <Outlet />
    </>
  );
}
