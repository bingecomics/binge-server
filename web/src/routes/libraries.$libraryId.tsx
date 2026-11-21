import { createFileRoute, Link, useLocation, useNavigate } from "@tanstack/react-router";
import { Copy, FolderSearch, History, RefreshCw, Tags, Trash2 } from "lucide-react";
import { memo, useEffect, useRef, useState } from "react";
import { useIsOwner } from "../api/account";
import type { Book } from "../api/client";
import {
  isScanning,
  useLibrary,
  useScanLibrary,
  useSeriesLetters,
  useSeriesPage,
  useSetAside,
  useUnreadableFiles,
} from "../api/libraries";
import { UNREADABLE_FILES } from "../components/Activity";
import { choiceProps } from "../components/BookGrid";
import { Button } from "../components/Button";
import { coverGridClass } from "../components/Cover";
import { FromFilesDialog } from "../components/FromFiles";
import { LetterBar, letterIn } from "../components/LetterBar";
import { MenuItem, MenuLink, MoreMenu } from "../components/Menu";
import { Narrowing } from "../components/Narrowing";
import { saidByTheMenuClass } from "../components/NavLink";
import { Notice, NotOnServer, ServerNotAnswering } from "../components/Notice";
import { Pages } from "../components/Pages";
import { RemoveLibraryDialog } from "../components/RemoveLibraryDialog";
import { ScanProgress } from "../components/ScanProgress";
import { rowClass } from "../components/Section";
import { SeriesCard } from "../components/SeriesCard";
import { shelfTitleClass } from "../components/Shelf";
import { ErrorText, textLinkClass } from "../components/text";
import { contents, count, moment } from "../format";
import { askedIn, narrowedBy, onPage, SERIES_A_PAGE, unnarrowed } from "../narrowing";

export const Route = createFileRoute("/libraries/$libraryId")({
  params: {
    parse: ({ libraryId }) => ({ libraryId: Number(libraryId) }),
    stringify: ({ libraryId }) => ({ libraryId: String(libraryId) }),
  },
  // The order of the series, what narrows them and the page of them: in
  // the address, so a narrowed library can be reloaded and shared.
  validateSearch: askedIn,
  component: LibraryPage,
});

/** A library with fewer series than this is seen whole: letters to jump by would be in the way. */
const ENOUGH_FOR_LETTERS = 24;

/**
 * The library's series: how they are ordered and narrowed, the letters to
 * jump by, a page of them, and the way to the other pages.
 */
const LibraryContents = memo(function LibraryContents({
  libraryId,
  seriesCount,
  scanning,
}: {
  libraryId: number;
  /** How many series the library has, with nothing narrowing them. */
  seriesCount: number;
  scanning: boolean;
}) {
  const asked = Route.useSearch();
  const series = useSeriesPage(libraryId, asked);
  // The letters are those of the names, so they are for the order of the names.
  const byName = asked.sort === undefined;
  const letters = useSeriesLetters(libraryId, asked, byName);
  const hash = useLocation({ select: (location) => location.hash });
  const at = byName ? letterIn(hash) : undefined;
  const grid = useRef<HTMLUListElement>(null);
  // The letter the page last went down to.
  const went = useRef<string>(null);

  const page = asked.page ?? 1;
  const total = series.data?.total;
  // Where the first series of the letter asked for is among the ones shown.
  const place = letters.data?.find((letter) => letter.letter === at)?.offset;
  const shownAt = place === undefined ? undefined : place - (page - 1) * SERIES_A_PAGE;
  const settled = series.data !== undefined && !series.isPlaceholderData;

  // Goes down to the letter asked for, once, when its page has arrived.
  useEffect(() => {
    if (at === undefined) {
      went.current = null;
      return;
    }
    const card = shownAt === undefined ? undefined : grid.current?.children[shownAt];
    if (!settled || !card || went.current === at) {
      return;
    }
    went.current = at;
    // It stops under the bar at the top, as the page says all scrolling does.
    card.scrollIntoView();
  }, [at, shownAt, settled]);

  if (series.error) {
    return <ErrorText>{series.error.message}</ErrorText>;
  }
  if (total === 0 && narrowedBy(asked) === 0) {
    return scanning ? (
      <p className="text-fg-muted">Nothing found yet.</p>
    ) : (
      <Notice icon={FolderSearch} title="No comics found here">
        The server looks for .cbz, .cbr and .pdf files in this folder and the folders inside it. Add
        some, then scan again.
      </Notice>
    );
  }
  const link = { to: "/libraries/$libraryId", params: { libraryId } } as const;
  return (
    <>
      <Narrowing libraryId={libraryId} asked={asked} shown={total} of={seriesCount} />
      {byName && letters.data && (total ?? 0) >= ENOUGH_FOR_LETTERS ? (
        <LetterBar
          libraryId={libraryId}
          asked={asked}
          letters={letters.data}
          // The letter already asked for changes nothing in the address: the
          // page goes down to it again from here.
          onChosen={(letter) => {
            if (letter === at && shownAt !== undefined) {
              grid.current?.children[shownAt]?.scrollIntoView();
            }
          }}
        />
      ) : null}
      {total === 0 ? (
        <p className="text-fg-muted">
          No series is like that.{" "}
          {narrowedBy(asked) > 0 ? (
            <Link {...link} search={unnarrowed(asked)} {...choiceProps} className={textLinkClass}>
              Show them all
            </Link>
          ) : null}
        </p>
      ) : series.data?.series.length === 0 ? (
        // An address kept from when the library had more, or typed by hand.
        <p className="text-fg-muted">
          There is no page {page.toLocaleString()}.{" "}
          <Link {...link} search={{ ...asked, page: undefined }} className={textLinkClass}>
            Go to the first page
          </Link>
        </p>
      ) : (
        <ul
          ref={grid}
          // Dimmed while the page shown is still the one before.
          className={`${coverGridClass} ${series.isPlaceholderData ? "opacity-60" : ""}`}
        >
          {series.data?.series.map((one) => (
            <SeriesCard key={one.id} series={one} />
          ))}
        </ul>
      )}
      {total ? (
        <Pages
          label="Pages of series"
          page={page}
          total={total}
          aPage={SERIES_A_PAGE}
          to={(page) => ({ ...link, search: onPage(asked, page) })}
        />
      ) : null}
    </>
  );
});

/**
 * The library's files that could not be read, with why. They are in no
 * series. The owner sets aside the ones they have seen, so that what is
 * listed is what is still to look at; the ones set aside can be shown and
 * taken back.
 */
const UnreadableFiles = memo(function UnreadableFiles({
  libraryId,
  asideCount,
  isOwner,
}: {
  libraryId: number;
  /** How many were set aside. None for anyone but the owner, who alone is told. */
  asideCount: number;
  isOwner: boolean;
}) {
  const [showingAside, setShowingAside] = useState(false);
  const files = useUnreadableFiles(libraryId);
  // The ones set aside are asked for when they are to be shown.
  const aside = useUnreadableFiles(libraryId, true, showingAside && asideCount > 0);
  const setAside = useSetAside();
  const any = Boolean(files.data?.length) || asideCount > 0;
  // An address that asks for these files is followed before they are
  // known, so the page goes to them once they are, and once the series
  // over them have arrived and pushed them down.
  const askedFor = useLocation({ select: (location) => location.hash === UNREADABLE_FILES });
  const over = useSeriesPage(libraryId, Route.useSearch());
  const inPlace = any && !over.isPending;
  const section = useRef<HTMLElement>(null);
  useEffect(() => {
    if (askedFor && inPlace) {
      section.current?.scrollIntoView();
    }
  }, [askedFor, inPlace]);

  if (!any) {
    return null;
  }
  const list = (which: Book[], putAside: boolean) => (
    <ul className="mt-2">
      {which.map((file) => (
        <li key={file.id} className={`${rowClass} flex items-center gap-4`}>
          <div className="min-w-0 flex-1">
            <p className="break-words">{file.name}</p>
            <p className="text-fg-muted text-metadata">{file.problem}</p>
          </div>
          {isOwner ? (
            <Button
              variant="quiet"
              disabled={setAside.isPending}
              onClick={() => setAside.mutate({ id: file.id, aside: putAside })}
              className="shrink-0"
            >
              {putAside ? "Set aside" : "Take back"}
            </Button>
          ) : null}
        </li>
      ))}
    </ul>
  );
  return (
    <section
      ref={section}
      id={UNREADABLE_FILES}
      aria-labelledby="unreadable-title"
      className="mt-12 max-w-2xl scroll-mt-20"
    >
      <h2 id="unreadable-title" className={shelfTitleClass}>
        Files that could not be read
      </h2>
      {setAside.isError ? <ErrorText className="mt-2">{setAside.error.message}</ErrorText> : null}
      {files.data?.length ? (
        list(files.data, true)
      ) : (
        <p className="mt-2 text-fg-muted text-metadata">None left to look at.</p>
      )}
      {asideCount > 0 ? (
        <>
          <button
            type="button"
            onClick={() => setShowingAside(!showingAside)}
            aria-expanded={showingAside}
            className={`${textLinkClass} mt-3 min-h-11 text-metadata`}
          >
            {showingAside ? "Hide" : "Show"} the {count(asideCount, "file", "files")} set aside
          </button>
          {showingAside && aside.data ? list(aside.data, false) : null}
        </>
      ) : null}
    </section>
  );
});

function LibraryPage() {
  const { libraryId } = Route.useParams();
  const { library, isPending, isError } = useLibrary(libraryId);
  const scan = useScanLibrary();
  const isOwner = useIsOwner();
  const navigate = useNavigate();
  const [removing, setRemoving] = useState(false);
  const [making, setMaking] = useState(false);

  if (isPending) {
    return null;
  }
  if (isError) {
    return <ServerNotAnswering />;
  }
  if (!library) {
    return (
      <NotOnServer title="This library is not on the server">It may have been removed.</NotOnServer>
    );
  }

  const scanning = isScanning(library);

  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <div className="min-w-0">
          <h1 className={`mb-1 font-bold text-screen-title tracking-tight ${saidByTheMenuClass}`}>
            {library.name}
          </h1>
          <p className="text-fg-muted text-metadata">
            {contents(library)}
            {library.last_scanned_at ? ` · Scanned ${moment(library.last_scanned_at)}` : null}
          </p>
        </div>
        {/* Scanning and removing are the owner's to do. */}
        {isOwner ? (
          <MoreMenu about={library.name}>
            <MenuItem
              icon={RefreshCw}
              disabled={scanning || scan.isPending}
              onSelect={() => scan.mutate(library.id)}
            >
              {scanning ? "Scanning…" : "Scan again"}
            </MenuItem>
            <MenuLink
              to="/libraries/$libraryId/duplicates"
              params={{ libraryId: library.id }}
              icon={Copy}
            >
              Books held twice
            </MenuLink>
            <MenuLink
              to="/libraries/$libraryId/scans"
              params={{ libraryId: library.id }}
              icon={History}
            >
              What scans changed
            </MenuLink>
            <MenuItem icon={Tags} onSelect={() => setMaking(true)}>
              Make from the files…
            </MenuItem>
            <MenuItem icon={Trash2} onSelect={() => setRemoving(true)}>
              Remove library
            </MenuItem>
          </MoreMenu>
        ) : null}
      </div>

      {scan.isError ? <ErrorText className="mt-4">{scan.error.message}</ErrorText> : null}
      <div className="mt-4 empty:hidden">
        <ScanProgress scan={library.scan} />
      </div>

      <div className="mt-8">
        <LibraryContents
          // Another library starts over: the series kept in sight while
          // the next ones arrive are never another library's.
          key={library.id}
          libraryId={library.id}
          seriesCount={library.series_count}
          scanning={scanning}
        />
        <UnreadableFiles
          libraryId={library.id}
          asideCount={library.set_aside_count ?? 0}
          isOwner={isOwner}
        />
      </div>

      <FromFilesDialog library={library} open={making} onClose={() => setMaking(false)} />
      <RemoveLibraryDialog
        library={library}
        open={removing}
        onClose={() => setRemoving(false)}
        onRemoved={() => navigate({ to: "/" })}
      />
    </>
  );
}
