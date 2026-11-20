import { createRootRoute, Outlet, useMatchRoute } from "@tanstack/react-router";
import { SetupPage } from "../account/SetupPage";
import { SignInPage } from "../account/SignInPage";
import { useMe } from "../api/account";
import { useHealth } from "../api/health";
import { AddToListDialog } from "../components/AddToList";
import { ServerNotAnswering } from "../components/Notice";
import { Problem } from "../components/Problem";
import { SelectionBar } from "../components/SelectionBar";
import { SettingsMenu } from "../components/SettingsMenu";
import { Sidebar } from "../components/Sidebar";
import { TopBar } from "../components/TopBar";

export const Route = createRootRoute({ component: Door });

/**
 * Decides what a visitor gets: first-run setup on a server without an owner,
 * the sign-in page when nobody is signed in, and the app otherwise. The
 * address stays what it was, so signing in leads to the page that was asked
 * for.
 */
function Door() {
  const health = useHealth();
  const me = useMe();

  if (health.data?.needs_setup) {
    return <SetupPage />;
  }
  if (me.data) {
    return <Shell />;
  }
  // Setup or signing in: that takes both answers.
  if (me.isPending || health.isPending) {
    return null;
  }
  if (me.isError) {
    return (
      <main className="grid min-h-dvh place-items-center px-4 py-8">
        <ServerNotAnswering />
      </main>
    );
  }
  return <SignInPage />;
}

function Shell() {
  const matches = useMatchRoute();
  // The reader fills the window by itself.
  if (matches({ to: "/read/$bookId" })) {
    return (
      <>
        <Outlet />
        <Problem />
      </>
    );
  }
  // The settings have a menu of their own, in place of the libraries'.
  const inSettings = matches({ to: "/settings", fuzzy: true });
  return (
    <div className="min-h-dvh">
      <TopBar withCoverSize={!inSettings} />
      <div className="md:grid md:grid-cols-[15rem_minmax(0,1fr)]">
        {inSettings ? <SettingsMenu /> : <Sidebar />}
        <main className="px-gutter py-8 md:py-12">
          <Outlet />
        </main>
      </div>
      <SelectionBar />
      <AddToListDialog />
      <Problem />
    </div>
  );
}
