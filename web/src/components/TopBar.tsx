import { Link } from "@tanstack/react-router";
import { Settings } from "lucide-react";
import { Activity } from "./Activity";
import { BingeMark } from "./BingeMark";
import { Brand } from "./Brand";
import { CoverSize } from "./CoverSize";
import { iconButtonClass } from "./NavLink";
import { ProfileMenu } from "./ProfileMenu";
import { SearchBox } from "./SearchBox";

/**
 * The bar across the top of every page but the reader: the name, which
 * leads Home, the search box, the size of the covers where the caller
 * wants it, what the server is doing, the settings, and who is signed in. The name is as wide as
 * the menu under it, so the box starts where the page does.
 */
export function TopBar({ withCoverSize }: { withCoverSize: boolean }) {
  return (
    <header className="sticky top-0 z-20 flex h-14 items-center gap-2 border-divider border-b bg-surface px-3 md:gap-4">
      <Link
        to="/"
        aria-label="Binge Server: Home"
        className="flex shrink-0 items-center rounded-control px-1 text-section-title md:w-65 md:px-3 md:text-wordmark"
      >
        {/* A narrow window has room for the mark only. */}
        <BingeMark className="size-8 sm:hidden" />
        <span className="max-sm:hidden">
          <Brand />
        </span>
      </Link>

      <SearchBox />

      <div className="ml-auto flex shrink-0 items-center gap-1">
        {withCoverSize ? <CoverSize /> : null}
        <Activity />
        <Link
          to="/settings"
          className={`${iconButtonClass} size-11 data-[status=active]:bg-raised data-[status=active]:text-fg`}
          aria-label="Settings"
          title="Settings"
        >
          <Settings className="size-5" aria-hidden="true" />
        </Link>
        <ProfileMenu />
      </div>
    </header>
  );
}
