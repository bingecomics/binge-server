import { Bookmark } from "lucide-react";
import { memo, useEffect, useMemo, useRef } from "react";
import { type Page, smallPageUrl } from "../api/client";

/**
 * The pages of the book as a strip of small pictures to jump by, over the
 * bar at the bottom. The pages on screen are marked and brought to the
 * middle, and a page that was kept carries its mark. It runs the way the
 * book is read.
 *
 * A picture is asked for when it comes into view, so a long book costs
 * what is looked at.
 */
/**
 * One page of the strip. Apart from the strip, so that turning a page
 * draws again the pages it left and came to, and not the whole book.
 */
const StripPage = memo(function StripPage({
  src,
  page,
  onScreen,
  first,
  isKept,
  onPick,
}: {
  src: string;
  page: Page;
  onScreen: boolean;
  /** Whether it is the first page on screen, which the strip is brought to. */
  first: boolean;
  isKept: boolean;
  onPick: (number: number) => void;
}) {
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (first) {
      button.current?.scrollIntoView({ inline: "center", block: "nearest" });
    }
  }, [first]);
  return (
    <button
      type="button"
      ref={button}
      onClick={() => onPick(page.number)}
      aria-current={onScreen ? "true" : undefined}
      aria-label={`Page ${page.number + 1}${isKept ? ", kept" : ""}`}
      className="group relative shrink-0 rounded-control text-caption text-fg-muted aria-[current]:font-semibold aria-[current]:text-fg"
    >
      <img
        src={src}
        alt=""
        loading="lazy"
        decoding="async"
        draggable={false}
        // The page's own shape, known before its picture comes.
        style={{ aspectRatio: `${page.width ?? 2} / ${page.height ?? 3}` }}
        className="h-24 w-auto rounded-cover bg-raised object-cover ring-1 ring-divider group-hover:ring-fg-muted group-aria-[current]:ring-2 group-aria-[current]:ring-accent"
      />
      <span className="block py-0.5 text-center tabular-nums">{page.number + 1}</span>
      {isKept ? (
        <Bookmark
          className="absolute top-1 right-1 size-4 fill-accent text-accent"
          aria-hidden="true"
        />
      ) : null}
    </button>
  );
});

export const PageStrip = memo(function PageStrip({
  identity,
  pages,
  shown,
  kept,
  rightToLeft,
  onPick,
}: {
  identity: string;
  pages: Page[];
  /** The pages on screen, by their numbers. */
  shown: number[];
  /** The pages that were kept, by their numbers. */
  kept: number[];
  rightToLeft: boolean;
  onPick: (number: number) => void;
}) {
  const keptOnes = useMemo(() => new Set(kept), [kept]);
  return (
    <nav
      aria-label="Pages of the book"
      dir={rightToLeft ? "rtl" : "ltr"}
      className="flex gap-2 overflow-x-auto border-divider border-b px-2 pt-2 pb-1 [scrollbar-width:thin]"
    >
      {pages.map((page) => (
        <StripPage
          key={page.number}
          src={smallPageUrl(identity, page.number)}
          page={page}
          onScreen={shown.includes(page.number)}
          first={page.number === shown[0]}
          isKept={keptOnes.has(page.number)}
          onPick={onPick}
        />
      ))}
    </nav>
  );
});
