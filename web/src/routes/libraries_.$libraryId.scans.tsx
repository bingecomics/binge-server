import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronLeft, FolderCheck } from "lucide-react";
import { useIsOwner } from "../api/account";
import type { ChangeKind, ScanChange, ScanRecord } from "../api/client";
import { CHANGES_A_PAGE, useLibrary, useScanChanges, useScans } from "../api/libraries";
import { buttonClass } from "../components/Button";
import { Notice, NotOnServer } from "../components/Notice";
import { Pages, pageAsked } from "../components/Pages";
import { rowClass } from "../components/Section";
import { ErrorText, PathText, textLinkClass, upLinkClass } from "../components/text";
import { called, count, moment } from "../format";

const KINDS = ["arrived", "back", "moved", "gone"] as const satisfies readonly ChangeKind[];

type Asked = { scan?: number; kind?: ChangeKind; page?: number };

export const Route = createFileRoute("/libraries_/$libraryId/scans")({
  params: {
    parse: ({ libraryId }) => ({ libraryId: Number(libraryId) }),
    stringify: ({ libraryId }) => ({ libraryId: String(libraryId) }),
  },
  // The scan looked at, the kind of change shown and the page of them.
  validateSearch: (search: Record<string, unknown>): Asked => {
    const scan = Number(search.scan);
    return {
      scan: Number.isSafeInteger(scan) && scan > 0 ? scan : undefined,
      kind: KINDS.find((kind) => kind === search.kind),
      page: pageAsked(search.page),
    };
  },
  component: ScansPage,
});

/** What each kind of change is called: for one book, and after a count of them. */
const KIND_NAMES: Record<ChangeKind, { one: string; many: string }> = {
  arrived: { one: "Arrived", many: "arrived" },
  back: { one: "Came back", many: "came back" },
  moved: { one: "Moved", many: "moved" },
  gone: { one: "Gone", many: "gone" },
};

/** How many changes of a kind a scan found: "12 arrived". */
function counted(scan: ScanRecord, kind: ChangeKind): string {
  return `${scan[kind].toLocaleString()} ${KIND_NAMES[kind].many}`;
}

/** Where the scans of a library are. */
function scansOf(libraryId: number) {
  return { to: "/libraries/$libraryId/scans", params: { libraryId } } as const;
}

/** What a scan changed, in a line: "12 arrived · 1 moved · 3 gone". */
function summed(scan: ScanRecord): string {
  return KINDS.filter((kind) => scan[kind] > 0)
    .map((kind) => counted(scan, kind))
    .join(" · ");
}

/** One file a scan found changed: the book it is, where the file is and where it was. */
function Change({ change }: { change: ScanChange }) {
  const { book } = change;
  return (
    <li className={rowClass}>
      <p className="flex flex-wrap items-baseline gap-x-3">
        <span className="font-semibold text-caption text-fg-muted uppercase tracking-wider">
          {KIND_NAMES[change.kind].one}
        </span>
        {book ? (
          <Link
            to="/books/$bookId"
            params={{ bookId: book.id }}
            className="min-w-0 font-semibold hover:underline"
          >
            {called(book)}
          </Link>
        ) : null}
      </p>
      <PathText path={change.path} className="mt-1" />
      {change.from_path ? <PathText path={`was at ${change.from_path}`} /> : null}
      {change.kind === "gone" && book ? (
        <p className="mt-1 text-caption text-fg-muted">The library still has this book.</p>
      ) : null}
    </li>
  );
}

/** What one scan changed: a kind at a time or all, a page at a time. */
function Changes({ libraryId, scanId }: { libraryId: number; scanId: number }) {
  const asked = Route.useSearch();
  const page = asked.page ?? 1;
  const changes = useScanChanges(scanId, asked.kind, page);
  const link = scansOf(libraryId);
  const scan = changes.data?.scan;

  if (changes.isError) {
    return <ErrorText className="mt-6">{changes.error.message}</ErrorText>;
  }
  if (!scan) {
    return null;
  }
  const chip = (kind: ChangeKind | undefined, name: string) => (
    <Link
      key={kind ?? "all"}
      {...link}
      search={{ scan: scanId, kind }}
      aria-current={asked.kind === kind ? "true" : undefined}
      className={`${buttonClass(asked.kind === kind ? "primary" : "secondary")} min-h-9 px-3`}
    >
      {name}
    </Link>
  );
  return (
    <>
      <h2 className="mt-6 font-semibold text-section-title">Scan of {moment(scan.started_at)}</h2>
      <nav aria-label="Kinds of change" className="mt-3 flex flex-wrap gap-2">
        {chip(
          undefined,
          `All ${(scan.arrived + scan.back + scan.moved + scan.gone).toLocaleString()}`,
        )}
        {KINDS.filter((kind) => scan[kind] > 0).map((kind) => chip(kind, counted(scan, kind)))}
      </nav>
      <ul className={`mt-4 max-w-3xl ${changes.isPlaceholderData ? "opacity-60" : ""}`}>
        {changes.data?.changes.map((change) => (
          <Change key={change.id} change={change} />
        ))}
      </ul>
      <Pages
        label="Pages of changes"
        page={page}
        total={changes.data?.total ?? 0}
        aPage={CHANGES_A_PAGE}
        to={(page) => ({ ...link, search: { ...asked, page: page > 1 ? page : undefined } })}
      />
    </>
  );
}

/**
 * What the scans of a library changed, to look back on: the books that
 * arrived, came back, moved and went. A scan that found the library as it
 * was is not listed. The owner's page.
 */
function ScansPage() {
  const { libraryId } = Route.useParams();
  const { scan } = Route.useSearch();
  const { library, isPending } = useLibrary(libraryId);
  const isOwner = useIsOwner();
  const scans = useScans(libraryId, isOwner);

  if (isPending) {
    return null;
  }
  if (!library || !isOwner) {
    return (
      <NotOnServer title="This page is not here">
        It is the owner's, for a library that is on the server.
      </NotOnServer>
    );
  }
  const link = scansOf(libraryId);
  return (
    <>
      {scan === undefined ? (
        <Link to="/libraries/$libraryId" params={{ libraryId }} className={upLinkClass}>
          <ChevronLeft className="size-4" aria-hidden="true" />
          {library.name}
        </Link>
      ) : (
        <Link {...link} className={upLinkClass}>
          <ChevronLeft className="size-4" aria-hidden="true" />
          All scans of {library.name}
        </Link>
      )}
      <h1 className="mt-2 mb-2 font-bold text-screen-title tracking-tight">What scans changed</h1>
      {scan !== undefined ? (
        <Changes libraryId={libraryId} scanId={scan} />
      ) : scans.isError ? (
        <ErrorText>{scans.error.message}</ErrorText>
      ) : scans.data?.length === 0 ? (
        <Notice icon={FolderCheck} title="No scan has changed anything yet">
          When a scan finds books that arrived, moved or went, it is listed here.
        </Notice>
      ) : scans.data ? (
        <>
          <p className="max-w-prose text-fg-muted">
            {count(scans.data.length, "scan", "scans")} found the folder changed. A scan that found
            everything as it was is not listed, and only the latest fifty are kept.
          </p>
          <ul className="mt-6 max-w-2xl">
            {scans.data.map((one) => (
              <li key={one.id} className={rowClass}>
                <Link
                  {...link}
                  search={{ scan: one.id }}
                  className={`font-semibold ${textLinkClass}`}
                >
                  {moment(one.started_at)}
                </Link>
                <p className="text-fg-muted text-metadata">{summed(one)}</p>
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </>
  );
}
