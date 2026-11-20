import { Link } from "@tanstack/react-router";
import { Activity as ActivityIcon } from "lucide-react";
import { useIsOwner } from "../api/account";
import type { Library } from "../api/client";
import { useReadingNow } from "../api/history";
import { isScanning, useLibraries } from "../api/libraries";
import { called, count, moment, pageOf } from "../format";
import { Menu } from "./Menu";
import { iconButtonClass, navHeadingClass } from "./NavLink";
import { ScanProgress } from "./ScanProgress";
import { textLinkClass } from "./text";

/** The part of a library's address that asks for the files that could not be read. */
export const UNREADABLE_FILES = "unreadable-files";

/** Who is reading now, and what. The owner's to see, asked for while the list is open. */
function ReadingNow() {
  const reading = useReadingNow();
  return (
    <section className="border-divider border-t px-3 pt-3 pb-1">
      <h3 className={navHeadingClass}>Reading now</h3>
      {reading.data?.length === 0 ? (
        <p className="mt-1 text-fg-muted text-metadata">Nobody is reading.</p>
      ) : (
        <ul className="mt-1">
          {reading.data?.map((one) => (
            <li key={one.user_id} className="py-1 text-metadata">
              <p className="truncate">
                <span className="font-semibold">{one.user_name}</span>{" "}
                <Link
                  to="/books/$bookId"
                  params={{ bookId: one.book.id }}
                  className="hover:underline"
                >
                  {called(one.book)}
                </Link>
              </p>
              <p className="text-caption text-fg-muted">{pageOf(one.page, one.book)}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** What the server is doing with one library, and what it could not read of it. */
function OfLibrary({ library }: { library: Library }) {
  // Only the owner is told how many there are.
  const unreadable = library.unreadable_count ?? 0;
  return (
    <li className="px-3 py-2">
      <p className="truncate font-semibold text-metadata">{library.name}</p>
      {isScanning(library) || library.scan.problem ? (
        <div className="mt-1">
          <ScanProgress scan={library.scan} />
        </div>
      ) : (
        <p className="text-caption text-fg-muted">
          {library.last_scanned_at
            ? `Scanned ${moment(library.last_scanned_at)}`
            : "Not scanned yet"}
        </p>
      )}
      {unreadable > 0 ? (
        <Link
          to="/libraries/$libraryId"
          params={{ libraryId: library.id }}
          hash={UNREADABLE_FILES}
          className={`${textLinkClass} text-caption`}
        >
          {count(unreadable, "file", "files")} could not be read
        </Link>
      ) : null}
    </li>
  );
}

/**
 * What the server is doing, behind an icon in the bar at the top: the
 * scans that run, what went wrong in the last ones, and for the owner the
 * files that could not be read and who is reading now. The icon carries a
 * dot while a scan runs or something waits to be looked at.
 */
export function Activity() {
  const libraries = useLibraries().data ?? [];
  const isOwner = useIsOwner();
  const scanning = libraries.some(isScanning);
  const waiting =
    libraries.some((library) => library.scan.problem) ||
    libraries.some((library) => (library.unreadable_count ?? 0) > 0);
  const said = scanning
    ? "Activity: scanning"
    : waiting
      ? "Activity: something to look at"
      : "Activity";

  return (
    <Menu
      label={said}
      title="Activity"
      className={`${iconButtonClass} relative size-11`}
      button={
        <>
          <ActivityIcon className="size-5" aria-hidden="true" />
          {scanning || waiting ? (
            <span
              className={`absolute top-2 right-2 size-2.5 rounded-full ring-2 ring-surface ${
                scanning ? "animate-pulse bg-accent motion-reduce:animate-none" : "bg-danger"
              }`}
              aria-hidden="true"
            />
          ) : null}
        </>
      }
    >
      <h2 className={`${navHeadingClass} px-3 pt-1 pb-1`}>Libraries</h2>
      {libraries.length === 0 ? (
        <p className="px-3 pb-2 text-fg-muted text-metadata">None yet.</p>
      ) : (
        <ul>
          {libraries.map((library) => (
            <OfLibrary key={library.id} library={library} />
          ))}
        </ul>
      )}
      {isOwner ? <ReadingNow /> : null}
    </Menu>
  );
}
