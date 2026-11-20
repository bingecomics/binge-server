import { BookImage, FileWarning } from "lucide-react";
import { coverUrl } from "../api/client";
import { ProgressBar } from "./ProgressBar";

/** How many covers stand together for something that holds books of several series. */
const TOGETHER = 4;

/**
 * A cover in the usual comic proportions, or a placeholder until there is
 * one. `done`, from 0 to 1, is how far the book was read: a bar along the
 * cover's lower edge. It is not read out: say the same in words beside it.
 *
 * `covers` are the ones a reading list says to show together as one, when
 * no cover was chosen for it: four of them. With fewer it shows as `cover`
 * says.
 *
 * `picture` is the address of a picture to show in a cover's place: a page
 * of a book. It is shown whole, since a page drawn across two is wider
 * than a cover.
 */
export function Cover({
  cover,
  covers,
  picture,
  unreadable = false,
  done,
}: {
  cover?: string | null;
  covers?: string[];
  picture?: string;
  unreadable?: boolean;
  done?: number;
}) {
  const Placeholder = unreadable ? FileWarning : BookImage;
  const together = covers?.slice(0, TOGETHER) ?? [];
  const src = picture ?? (cover ? coverUrl(cover) : null);
  return (
    <div className="relative aspect-[2/3] overflow-hidden rounded-cover bg-raised ring-1 ring-divider">
      {together.length === TOGETHER ? (
        <div className="grid size-full grid-cols-2 grid-rows-2 gap-px bg-divider">
          {together.map((one) => (
            <img
              key={one}
              src={coverUrl(one)}
              alt=""
              loading="lazy"
              decoding="async"
              className="size-full min-h-0 object-cover"
            />
          ))}
        </div>
      ) : src ? (
        // The name beside every cover says what it is, so the image adds nothing to read out.
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          className={`size-full ${picture ? "object-contain" : "object-cover"}`}
        />
      ) : (
        <div className="grid size-full place-items-center">
          <Placeholder
            className={`size-8 ${unreadable ? "text-danger" : "text-fg-muted opacity-60"}`}
            aria-hidden="true"
          />
        </div>
      )}
      {done == null ? null : (
        <ProgressBar done={done} className="absolute inset-x-0 bottom-0 h-1.5" />
      )}
    </div>
  );
}

/**
 * The grid that covers sit in: as many columns as fit, none narrower than
 * the size of a cover, which the slider in the bar at the top sets.
 */
export const coverGridClass =
  "grid grid-cols-[repeat(auto-fill,minmax(var(--cover),1fr))] gap-x-4 gap-y-6 " +
  "md:gap-x-5 md:gap-y-8";

/**
 * One line of covers that goes on sideways, each as wide as a cover in the
 * grid at its narrowest. The line reaches the edges of the window through
 * the page's own margin, so a cover is cut by the window and not by empty
 * space. The space over and under it is for the lift and the focus ring of
 * a card, which a box that scrolls would cut off.
 */
export const coverLineClass =
  "-mx-gutter flex snap-x scroll-px-gutter gap-4 overflow-x-auto px-gutter py-1 " +
  "[scrollbar-width:thin] md:gap-5 [&>li]:w-cover [&>li]:shrink-0 [&>li]:snap-start";

/**
 * What sits in a top corner of a card's cover, over the link: seen while
 * the pointer is over the card, and when the keyboard is on it.
 */
export const cardCornerClass =
  "absolute top-1 grid size-9 place-items-center opacity-0 transition-opacity duration-150 " +
  "focus-visible:opacity-100 group-hover/card:opacity-100";
