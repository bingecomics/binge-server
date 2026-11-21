import { Link, useMatch } from "@tanstack/react-router";
import { Bookmark, House, LoaderCircle, Plus } from "lucide-react";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import { isScanning, useLibraries } from "../api/libraries";
import { askedBy, linkTo, useViews } from "../api/views";
import { sameAsked } from "../narrowing";
import { AddLibraryDialog } from "./AddLibraryDialog";
import { ComicLibraryIcon } from "./ComicLibraryIcon";
import { MyBingeLink } from "./MyBinge";
import { iconButtonClass, NavLink, navHeadingClass, navLinkClass, sideMenuClass } from "./NavLink";
import { ServerStatus } from "./ServerStatus";

function Libraries() {
  const libraries = useLibraries();
  const isOwner = useIsOwner();
  const [adding, setAdding] = useState(false);

  return (
    <section className="flex min-h-0 flex-col" aria-labelledby="sidebar-libraries">
      <div className="flex min-h-9 items-center justify-between pr-1 pl-3">
        <h2 id="sidebar-libraries" className={navHeadingClass}>
          Libraries
        </h2>
        {isOwner ? (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className={`${iconButtonClass} size-9`}
            aria-label="Add a library"
            title="Add a library"
          >
            <Plus className="size-5" aria-hidden="true" />
          </button>
        ) : null}
      </div>

      {libraries.data?.length === 0 ? (
        <p className="mt-1 px-3 text-fg-muted text-metadata">None yet</p>
      ) : (
        <ul className="mt-1 flex flex-col gap-1 overflow-y-auto">
          {libraries.data?.map((library) => (
            <li key={library.id}>
              <Link
                to="/libraries/$libraryId"
                params={{ libraryId: library.id }}
                className={navLinkClass}
              >
                <ComicLibraryIcon className="size-5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{library.name}</span>
                {isScanning(library) ? (
                  <LoaderCircle
                    className="size-4 shrink-0 animate-spin motion-reduce:animate-none"
                    aria-label="Scanning"
                  />
                ) : null}
              </Link>
            </li>
          ))}
        </ul>
      )}

      {isOwner ? <AddLibraryDialog open={adding} onClose={() => setAdding(false)} /> : null}
    </section>
  );
}

/** The views that are entries of the menu, each a library narrowed and ordered. */
function Views() {
  const views = useViews().data?.filter((view) => view.in_menu) ?? [];
  // What the library's page shows now, as the page itself reads its address.
  const shown = useMatch({
    from: "/libraries/$libraryId",
    shouldThrow: false,
    select: (match) => ({ libraryId: match.params.libraryId, asked: match.search }),
  });
  if (views.length === 0) {
    return null;
  }
  return (
    <section className="flex min-h-0 flex-col" aria-labelledby="sidebar-views">
      <h2 id="sidebar-views" className={`${navHeadingClass} flex min-h-9 items-center pl-3`}>
        Views
      </h2>
      <ul className="mt-1 flex flex-col gap-1 overflow-y-auto">
        {views.map((view) => {
          // Marked while the library shows what the view asks, on any page of it.
          const isShown =
            shown?.libraryId === view.library_id && sameAsked(shown.asked, askedBy(view));
          return (
            <li key={view.id}>
              <Link
                {...linkTo(view)}
                // The router compares an address as it was read, where one
                // value is not yet a list, so its own mark is kept to an
                // address that is exactly the view's and the rest is said here.
                activeOptions={{ exact: true, includeSearch: true }}
                data-status={isShown ? "active" : undefined}
                aria-current={isShown ? "page" : undefined}
                className={navLinkClass}
              >
                <Bookmark className="size-5 shrink-0" aria-hidden="true" />
                <span className="min-w-0 flex-1 truncate">{view.name}</span>
              </Link>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/**
 * What there is to read, in a column beside the page: Home, My Binge,
 * the views kept for the menu and the libraries. My Binge is one entry for
 * all that is a person's own: what they want to read, their reading lists,
 * their collections and their favorite pages.
 * What was read is not here: it is looked at now and then, and the menu of
 * who is signed in leads to it. A narrow window has no room for the column:
 * there the name in the bar at the top leads Home, which lists the
 * libraries and those views, and the menu of who is signed in leads to My
 * Binge.
 */
export function Sidebar() {
  return (
    <aside className={`${sideMenuClass} hidden md:flex md:flex-col md:gap-6 md:px-3 md:py-6`}>
      <nav aria-label="Main" className="flex flex-col gap-1">
        <NavLink to="/" icon={House} label="Home" />
        <MyBingeLink />
      </nav>
      <Views />
      <Libraries />
      <div className="mt-auto pl-3">
        <ServerStatus />
      </div>
    </aside>
  );
}
