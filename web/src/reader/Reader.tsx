import { useCanGoBack, useNavigate, useRouter } from "@tanstack/react-router";
import {
  ArrowLeft,
  BookOpen,
  ChevronLeft,
  ChevronRight,
  EyeOff,
  GalleryHorizontalEnd,
  ImagePlus,
  LoaderCircle,
  type LucideIcon,
  SlidersHorizontal,
  TriangleAlert,
} from "lucide-react";
import {
  type MouseEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useIsOwner } from "../api/account";
import { type Book, type Page, pageUrl } from "../api/client";
import { useKeepPage, useKeptIn } from "../api/kept";
import { useIsOwnList } from "../api/lists";
import { usePageProblem, useSaveProgress } from "../api/reading";
import { Button } from "../components/Button";
import { PageAsCover } from "../components/CoverChoices";
import { isOpenOverPage, Menu } from "../components/Menu";
import { FavoriteIcon } from "../components/MyBinge";
import { called } from "../format";
import type { Followed } from "./following";
import { backFrom } from "./home";
import { PageStrip } from "./PageStrip";
import { SettingsPanel } from "./SettingsPanel";
import { type Fit, useReaderSettings } from "./settings";
import { spreads } from "./spreads";

/** Screens loaded before they are turned to. */
const SCREENS_AHEAD = 2;

/** How long a page may take to arrive before the reader shows that it is waiting, in milliseconds. */
const PATIENCE = 300;

/** How much of the window a press of Space scrolls a tall page, and how much an arrow does. */
const SCROLL_STEP = 0.9;
const ARROW_STEP = 0.15;

const WIDER_THAN_TALL = "(min-aspect-ratio: 1/1)";

/** What a book has kept until its kept pages have come. The same each time, for what compares it. */
const NONE_KEPT: number[] = [];

function watchWindowShape(onChange: () => void) {
  const query = window.matchMedia(WIDER_THAN_TALL);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

/** Whether the window is wider than it is tall, which is what two pages need. */
function useWideWindow(): boolean {
  return useSyncExternalStore(watchWindowShape, () => window.matchMedia(WIDER_THAN_TALL).matches);
}

/** Where a page sits on screen: by itself, or as one side of a pair. */
type Side = "alone" | "left" | "right";

/** The box around the pages on screen, for each way of fitting them. */
const frameClass: Record<Fit, string> = {
  page: "size-full",
  width: "w-full items-start",
  height: "h-full shrink-0",
  original: "shrink-0 items-start",
};

function imageClass(fit: Fit, side: Side): string {
  switch (fit) {
    case "page": {
      // Each page gets its half and leans on the fold, so a pair meets there.
      const lean = { alone: "w-full", left: "w-1/2 object-right", right: "w-1/2 object-left" };
      return `h-full min-h-0 min-w-0 object-contain ${lean[side]}`;
    }
    case "width":
      return `h-auto min-w-0 ${side === "alone" ? "w-full" : "w-1/2"}`;
    case "height":
      return "h-full w-auto max-w-none";
    case "original":
      return "max-w-none";
  }
}

/** Why a page is not on screen, where the page would be. */
function PageProblem({ identity, number }: { identity: string; number: number }) {
  const problem = usePageProblem(identity, number);
  if (problem.isPending) {
    return null;
  }
  return (
    <div className="m-auto flex max-w-sm flex-col items-center gap-3 p-8 text-center" role="alert">
      <TriangleAlert className="size-8 text-danger" aria-hidden="true" />
      <p className="font-semibold">Page {number + 1} cannot be shown</p>
      <p className="text-fg-muted text-metadata">
        {problem.data ?? "The server sent it, but this browser cannot show the image."}
      </p>
    </div>
  );
}

function PageImage({
  identity,
  page,
  className,
  onSettled,
}: {
  identity: string;
  page: Page;
  className: string;
  /** Called when the image is on screen or has failed. */
  onSettled: (number: number) => void;
}) {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return <PageProblem identity={identity} number={page.number} />;
  }
  return (
    <img
      src={pageUrl(identity, page.number)}
      alt={`Page ${page.number + 1}`}
      // Known sizes keep the layout still while the image arrives.
      width={page.width ?? undefined}
      height={page.height ?? undefined}
      draggable={false}
      decoding="async"
      onLoad={() => onSettled(page.number)}
      onError={() => {
        setFailed(true);
        onSettled(page.number);
      }}
      className={className}
    />
  );
}

/** A bar of the reader, which slides out of the window when it is put away. */
const barClass =
  "fixed inset-x-0 z-10 border-divider bg-surface/95 backdrop-blur transition-transform " +
  "duration-200 motion-reduce:transition-none";

/** The buttons of a bar, in a row. */
const barRowClass = "flex items-center gap-1 px-2 py-1";

const barButtonClass =
  "grid size-11 shrink-0 place-items-center rounded-control text-fg-muted transition-colors " +
  "duration-150 hover:bg-raised hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent " +
  "aria-expanded:bg-raised aria-expanded:text-fg aria-pressed:text-accent";

/** What the reader follows, as the words that say what a book is next in, and the end of. */
const FOLLOWED: Record<Followed, { next: string; end: string }> = {
  list: { next: "Next in the reading list", end: "the reading list" },
  group: { next: "Next in the series", end: "this part of the series" },
  series: { next: "Next in the series", end: "the series" },
};

/** What a book was the last of: what the reader followed, a volume by that name. */
function endOf(followed: Followed, book: Book): string {
  return followed === "group" && book.group_kind === "volume"
    ? "this volume"
    : FOLLOWED[followed].end;
}

/** What is said over the last page, above the bar at the bottom. */
const offerClass =
  "pointer-events-auto flex max-w-full items-center gap-4 rounded-card border border-divider " +
  "bg-surface p-2 pl-4 shadow-xl";

/**
 * The reader: a book's pages filling the window, with bars that come and go.
 *
 * `identity` is the book's, and `pages` are not empty. It opens on the page
 * `startAt`, from 0. What the book is read as a part of is `followed`, and
 * the book that comes after it there is `next`: going on from the last
 * page offers that book, and going on once more opens it in this book's
 * place (`onNext`). At the end of what is followed, the reader says so.
 *
 * While the reading is not `recorded`, turning pages saves nothing.
 * `onRecord` turns that on and off.
 */
export function Reader({
  book,
  identity,
  pages,
  startAt,
  listId,
  followed,
  next,
  onNext,
  recorded,
  onRecord,
}: {
  book: Book;
  identity: string;
  pages: Page[];
  startAt: number;
  /** The reading list the book is read from, to go back to. */
  listId?: number;
  followed: Followed | null;
  next?: Book;
  onNext: (next: Book) => void;
  recorded: boolean;
  onRecord: (recorded: boolean) => void;
}) {
  const { settings } = useReaderSettings(book.series_id);
  const owner = useIsOwner();
  // A list someone shared keeps the cover its maker gave it.
  const ownList = useIsOwnList(listId);
  const wide = useWideWindow();
  const paired = settings.twoPages && wide;
  const layout = useMemo(() => spreads(pages, paired), [pages, paired]);
  const lastPage = pages.length - 1;

  const [page, setPage] = useState(() => Math.min(Math.max(startAt, 0), lastPage));
  const [barsShown, setBarsShown] = useState(true);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [stripOpen, setStripOpen] = useState(false);
  /**
   * Whether the reader went on from the last page: the next book is then
   * offered, or the end of what is followed is said.
   */
  const [pastEnd, setPastEnd] = useState(false);
  /** The page the slider is dragged to, until it is let go. */
  const [scrub, setScrub] = useState<number | null>(null);
  const [settled, setSettled] = useState<ReadonlySet<number>>(new Set());

  const at = Math.max(
    layout.findIndex((screen) => screen.includes(page)),
    0,
  );
  const shown = layout[at] ?? [page];
  const first = shown[0] ?? page;
  const last = shown[shown.length - 1] ?? page;
  const rightToLeft = settings.rightToLeft;

  /** Which way the book goes on: 1 to the right, -1 to the left. */
  const forward = rightToLeft ? -1 : 1;

  /** Shows a page. One turned to on the page itself puts the bars away. */
  const goTo = useCallback((target: number, from: "page" | "bars") => {
    setPage(target);
    setPastEnd(false);
    if (from === "page") {
      setBarsShown(false);
      setSettingsOpen(false);
    }
  }, []);

  const router = useRouter();
  const navigate = useNavigate();

  /**
   * Goes `by` screens on. Past the end there is nowhere to go, so the bars
   * come up and show that this is the last page. The book that follows is
   * offered there, and going on once more opens it.
   */
  const turn = useCallback(
    (by: number, from: "page" | "bars") => {
      const target = layout[at + by];
      if (target) {
        goTo(target[0] ?? 0, from);
      } else if (by > 0 && next && pastEnd) {
        onNext(next);
      } else {
        setBarsShown(true);
        setPastEnd(by > 0);
      }
    },
    [layout, at, goTo, next, pastEnd, onNext],
  );

  // Leaving goes back to where the book was opened from. Opened by its
  // address, there is no such place: then to the list it is read from, or
  // else to its home.
  const canGoBack = useCanGoBack();
  const close = useCallback(() => {
    if (canGoBack) {
      router.history.back();
    } else {
      void navigate({ ...backFrom(book, listId), replace: true });
    }
  }, [canGoBack, router, navigate, book, listId]);

  // Progress is the last page on screen, saved when a page is turned.
  // Opening a book does not count as reading it, unless it opens on its last
  // page and so is read by being opened. Nor does a change of what fits on
  // screen: going from two pages to one must not take a finished book back.
  //
  // A reading that is not recorded saves nothing. Recorded from some page
  // on, it saves the page it is on then, if pages were turned meanwhile.
  const { save, problem: saveProblem } = useSaveProgress(book.id);
  const savedAt = useRef<number | null>(
    last === lastPage && !book.progress?.completed ? null : page,
  );
  useEffect(() => {
    if (recorded && savedAt.current !== page) {
      savedAt.current = page;
      save(last);
    }
  }, [recorded, page, last, save]);

  // The pages of this book that were kept. With two on screen, the first
  // is the one kept, and letting go lets go of both.
  const kept = useKeptIn(book.id).data ?? NONE_KEPT;
  const { mutate: keepPage } = useKeepPage();
  const keptShown = shown.filter((number) => kept.includes(number));
  const toggleKept = () => {
    const ofBook = { bookId: book.id, identity };
    if (keptShown.length === 0) {
      keepPage({ ...ofBook, number: first, keep: true });
    }
    for (const number of keptShown) {
      keepPage({ ...ofBook, number, keep: false });
    }
  };
  const pickFromStrip = useCallback((number: number) => goTo(number, "bars"), [goTo]);
  // Put away with the bars, it asks for no pictures and follows no page.
  const stripShown = stripOpen && barsShown;

  // Each new screen starts at its top, on the side reading starts from.
  const scroller = useRef<HTMLDivElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when what is on screen changes
  useLayoutEffect(() => {
    const area = scroller.current;
    area?.scrollTo({ top: 0, left: rightToLeft ? area.scrollWidth : 0 });
  }, [first, settings, wide, rightToLeft]);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      // A menu that is open takes the keys for itself: Escape closes it, and
      // a page turned under it would put away the bar it hangs from.
      if (isOpenOverPage()) {
        return;
      }
      const target = event.target instanceof Element ? event.target : null;
      if (event.key === "Escape") {
        event.preventDefault();
        if (settingsOpen) {
          setSettingsOpen(false);
        } else {
          close();
        }
        return;
      }
      // A slider or a choice uses the arrows itself, and Space presses the
      // control that has focus.
      if (target?.closest("input")) {
        return;
      }
      if (event.key === " " && target?.closest("button, a")) {
        return;
      }
      const area = scroller.current;
      switch (event.key) {
        case "ArrowRight":
          turn(forward, "page");
          break;
        case "ArrowLeft":
          turn(-forward, "page");
          break;
        case "ArrowDown":
        case "ArrowUp":
          // The page scrolls inside the reader, which the browser does not
          // scroll for a key unless it has focus.
          area?.scrollBy({
            top: (event.key === "ArrowUp" ? -1 : 1) * area.clientHeight * ARROW_STEP,
          });
          break;
        case "PageDown":
        case "PageUp":
        case " ": {
          // Down a tall page first, and on to the next at its end.
          const back = event.key === "PageUp" || (event.key === " " && event.shiftKey);
          const room = area
            ? back
              ? area.scrollTop
              : area.scrollHeight - area.clientHeight - area.scrollTop
            : 0;
          if (area && room > 1) {
            area.scrollBy({ top: (back ? -1 : 1) * area.clientHeight * SCROLL_STEP });
          } else {
            turn(back ? -1 : 1, "page");
          }
          break;
        }
        case "Home":
          goTo(0, "page");
          break;
        case "End":
          goTo(lastPage, "page");
          break;
        case "Tab":
          // The bars hold everything that can take focus.
          setBarsShown(true);
          return;
        default:
          return;
      }
      event.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [turn, goTo, close, settingsOpen, forward, lastPage]);

  // The sides of the window turn the page and its middle shows or hides the
  // bars. Which side goes on follows the reading direction.
  function onTap(event: MouseEvent) {
    if (settingsOpen) {
      setSettingsOpen(false);
      return;
    }
    const across = event.clientX / window.innerWidth;
    if (across < 1 / 3) {
      turn(-forward, "page");
    } else if (across > 2 / 3) {
      turn(forward, "page");
    } else {
      setBarsShown((shown) => !shown);
    }
  }

  const onSettled = useCallback((number: number) => {
    setSettled((before) => new Set(before).add(number));
  }, []);
  const loading = shown.some((number) => !settled.has(number));
  // Pages that come at once, as preloaded ones do, never show the spinner.
  const [waiting, setWaiting] = useState(false);
  useEffect(() => {
    if (!loading) {
      setWaiting(false);
      return;
    }
    const timer = setTimeout(() => setWaiting(true), PATIENCE);
    return () => clearTimeout(timer);
  }, [loading]);

  // The next screens load while this one is read, and the one before stays
  // at hand. They wait until this one has arrived, so they do not take from
  // it, and pages leafed past are not fetched. Kept in a map so the browser
  // does not drop them half loaded.
  const preloaded = useRef(new Map<number, HTMLImageElement>());
  useEffect(() => {
    if (loading) {
      return;
    }
    const wanted = new Set([
      ...layout.slice(at + 1, at + 1 + SCREENS_AHEAD).flat(),
      ...(layout[at - 1] ?? []),
    ]);
    const images = preloaded.current;
    for (const [number, image] of images) {
      if (!wanted.has(number)) {
        // Stops one that is still on its way.
        image.src = "";
        images.delete(number);
      }
    }
    for (const number of wanted) {
      if (!images.has(number)) {
        const image = new Image();
        image.decoding = "async";
        image.src = pageUrl(identity, number);
        images.set(number, image);
      }
    }
  }, [loading, layout, at, identity]);

  // The slider turns the page when it is let go, which the browser reports
  // wherever the pointer is by then; React has no name for that event. The
  // slider stands on the last page on screen, so one step back from there can
  // be the other page of a pair, which is on screen already: it means the
  // screen before.
  const slider = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const input = slider.current;
    if (!input) {
      return;
    }
    function letGo() {
      const target = Number(input?.value) - 1;
      if (target !== last && shown.includes(target)) {
        turn(-1, "bars");
      } else {
        goTo(target, "bars");
      }
      setScrub(null);
    }
    input.addEventListener("change", letGo);
    return () => input.removeEventListener("change", letGo);
  }, [last, shown, turn, goTo]);

  const position =
    scrub !== null ? `${scrub}` : first === last ? `${first + 1}` : `${first + 1}–${last + 1}`;

  /** A button of the bottom bar that goes `by` screens on, or on to the book that follows. */
  const arrow = (by: number, Icon: LucideIcon) => {
    const toNextBook = by === 1 && !layout[at + 1] && next !== undefined;
    return (
      <button
        type="button"
        onClick={() => turn(by, "bars")}
        disabled={!layout[at + by] && !toNextBook}
        className={barButtonClass}
        aria-label={toNextBook ? "Next book" : by === 1 ? "Next page" : "Previous page"}
      >
        <Icon className="size-6" aria-hidden="true" />
      </button>
    );
  };

  return (
    <div className="fixed inset-0 bg-background">
      {/* biome-ignore lint/a11y/noStaticElementInteractions: the keyboard turns pages through the window, and the bars have buttons for it */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: as above */}
      <div ref={scroller} onClick={onTap} className="flex size-full select-none overflow-auto">
        <div className={`m-auto flex ${frameClass[settings.fit]}`}>
          {shown.map((number, place) => {
            const found = pages[number];
            if (!found) {
              return null;
            }
            const onLeft = (place === 0) !== rightToLeft;
            const side: Side = shown.length === 1 ? "alone" : onLeft ? "left" : "right";
            return (
              <PageImage
                key={number}
                identity={identity}
                page={found}
                className={`${imageClass(settings.fit, side)} ${onLeft ? "order-1" : "order-2"}`}
                onSettled={onSettled}
              />
            );
          })}
        </div>
      </div>

      {waiting ? (
        <div className="pointer-events-none fixed inset-0 grid place-items-center">
          <LoaderCircle
            className="size-8 animate-spin text-fg-muted motion-reduce:animate-none"
            aria-label="Loading the page"
          />
        </div>
      ) : null}

      {/* Outside the bars, which are out of reach while they are hidden, and over the strip of pages. */}
      <div
        className={`pointer-events-none fixed inset-x-4 z-10 flex flex-col items-center gap-2 ${
          stripShown ? "bottom-52" : "bottom-16"
        }`}
      >
        {pastEnd && followed && next ? (
          <section aria-label={FOLLOWED[followed].next} className={offerClass}>
            <p className="min-w-0 text-metadata" aria-live="polite">
              <span className="block text-caption text-fg-muted">{FOLLOWED[followed].next}</span>
              <span className="block truncate font-semibold">{called(next)}</span>
            </p>
            <Button variant="primary" onClick={() => onNext(next)}>
              <BookOpen className="size-5" aria-hidden="true" />
              Read
            </Button>
          </section>
        ) : pastEnd && followed ? (
          <section aria-label="The end" className={offerClass}>
            <p className="min-w-0 text-metadata" aria-live="polite">
              That was the last book of {endOf(followed, book)}.
            </p>
            <Button onClick={close}>Close</Button>
          </section>
        ) : null}
        {saveProblem ? (
          <p
            className="w-fit rounded-control border border-divider bg-surface px-4 py-2 text-center text-caption text-danger shadow-xl"
            role="alert"
          >
            Your place is not being saved. {saveProblem}
          </p>
        ) : null}
      </div>

      <header
        inert={!barsShown}
        className={`${barClass} ${barRowClass} top-0 border-b ${barsShown ? "" : "-translate-y-full"}`}
      >
        <button type="button" onClick={close} className={barButtonClass} aria-label="Close">
          <ArrowLeft className="size-5" aria-hidden="true" />
        </button>
        <h1 className="min-w-0 flex-1 truncate font-semibold text-metadata">{called(book)}</h1>
        {recorded ? null : (
          <button
            type="button"
            onClick={() => onRecord(true)}
            className="flex min-h-9 shrink-0 items-center gap-1.5 rounded-control bg-raised px-2 text-caption text-fg-muted hover:text-fg"
            title="This reading is not recorded. Press to record it from here."
          >
            <EyeOff className="size-4" aria-hidden="true" />
            <span className="max-sm:sr-only">Not recorded</span>
            <span className="sr-only">. Record it from here</span>
          </button>
        )}
        <p className="shrink-0 px-2 text-fg-muted text-metadata tabular-nums" aria-live="polite">
          <span className="sr-only">Page </span>
          {position} of {pages.length}
        </p>
        <button
          type="button"
          onClick={toggleKept}
          className={barButtonClass}
          aria-pressed={keptShown.length > 0}
          aria-label="Favorite page"
          title={
            keptShown.length > 0 ? "A favorite. Press to remove it" : "Add this page to favorites"
          }
        >
          <FavoriteIcon
            className={`size-5 ${keptShown.length > 0 ? "fill-current" : ""}`}
            aria-hidden="true"
          />
        </button>
        {owner ? (
          <Menu
            label="Use what is on screen as a cover"
            title="Use as a cover"
            className={barButtonClass}
            button={<ImagePlus className="size-5" aria-hidden="true" />}
          >
            <PageAsCover book={book} pages={shown} listId={ownList ? listId : undefined} />
          </Menu>
        ) : null}
        <button
          type="button"
          onClick={() => setSettingsOpen((open) => !open)}
          className={barButtonClass}
          aria-label="Reader settings"
          aria-expanded={settingsOpen}
        >
          <SlidersHorizontal className="size-5" aria-hidden="true" />
        </button>
        {settingsOpen ? (
          <SettingsPanel
            seriesId={book.series_id}
            series={book.series_name}
            wide={wide}
            recorded={recorded}
            onRecord={onRecord}
          />
        ) : null}
      </header>

      <footer
        inert={!barsShown}
        className={`${barClass} bottom-0 border-t ${barsShown ? "" : "translate-y-full"}`}
      >
        {stripShown ? (
          <PageStrip
            identity={identity}
            pages={pages}
            shown={shown}
            kept={kept}
            rightToLeft={rightToLeft}
            onPick={pickFromStrip}
          />
        ) : null}
        <div className={barRowClass}>
          {arrow(-forward, ChevronLeft)}
          <input
            type="range"
            min={1}
            max={pages.length}
            value={scrub ?? last + 1}
            ref={slider}
            onChange={(event) => {
              const value = Number(event.target.value);
              // Dragged back to where it stood, it is let go without a word.
              setScrub(value === last + 1 ? null : value);
            }}
            dir={rightToLeft ? "rtl" : "ltr"}
            disabled={pages.length < 2}
            aria-label="Page"
            className="min-w-0 flex-1 accent-accent"
          />
          {arrow(forward, ChevronRight)}
          <button
            type="button"
            onClick={() => setStripOpen((open) => !open)}
            disabled={pages.length < 2}
            className={barButtonClass}
            aria-expanded={stripOpen}
            aria-label="Pages of the book"
            title="Pages of the book"
          >
            <GalleryHorizontalEnd className="size-5" aria-hidden="true" />
          </button>
        </div>
      </footer>
    </div>
  );
}
