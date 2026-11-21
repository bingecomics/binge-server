import { useLocation } from "@tanstack/react-router";
import { ChevronLeft, KeyRound, Server, UserRound, Users } from "lucide-react";
import { useEffect, useRef } from "react";
import { useIsOwner } from "../api/account";
import { ComicLibraryIcon } from "./ComicLibraryIcon";
import { NavLink, navHeadingClass, sideMenuClass } from "./NavLink";

/**
 * The menu of the settings, which takes the place of the one with the
 * libraries while a settings page is shown: what is the person's own, then
 * what is the server's. A column beside the page on wide windows, a row
 * above it that scrolls sideways on narrow ones.
 */
export function SettingsMenu() {
  const isOwner = useIsOwner();
  const menu = useRef<HTMLElement>(null);
  const page = useLocation({ select: (location) => location.pathname });

  // In a row too long for the window, the page being shown is brought into sight.
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs again for each page
  useEffect(() => {
    menu.current
      ?.querySelector("[data-status=active]")
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [page]);
  const headingClass = `${navHeadingClass} hidden min-h-9 items-center px-3 md:flex`;
  return (
    <aside className={`${sideMenuClass} border-b md:border-b-0 md:px-3 md:py-6`}>
      <nav
        ref={menu}
        aria-label="Settings"
        className="flex gap-1 overflow-x-auto px-3 py-2 [scrollbar-width:none] md:flex-col md:p-0"
      >
        <div className="hidden md:mb-5 md:block">
          <NavLink to="/" icon={ChevronLeft} label="Home" />
        </div>
        <p className={headingClass}>You</p>
        <NavLink to="/settings/account" icon={UserRound} label="Account" />
        <NavLink to="/settings/tokens" icon={KeyRound} label="App tokens" />
        <p className={`${headingClass} md:mt-5`}>This server</p>
        {isOwner ? <NavLink to="/settings/users" icon={Users} label="Users" /> : null}
        <NavLink to="/settings/libraries" icon={ComicLibraryIcon} label="Libraries" />
        <NavLink to="/settings/server" icon={Server} label="About" />
      </nav>
    </aside>
  );
}
