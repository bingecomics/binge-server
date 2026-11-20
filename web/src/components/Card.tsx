import { Link, type LinkProps } from "@tanstack/react-router";
import type { ReactNode } from "react";
import type { Pickable } from "../selection";
import { Cover } from "./Cover";
import { usePick } from "./Pick";

/**
 * The card itself, whose corners hold its tick and its menu. It is as tall
 * as the tallest card of its row, so that the last line of every card of
 * the row is at the same height.
 */
const cardClass = "group/card relative flex flex-col";
/**
 * What is under the tick and the menu: the cover and the lines. A finger
 * resting on it picks the card, so the browser keeps its own menu of the
 * link away.
 */
const insideClass =
  "group flex grow flex-col rounded-cover [-webkit-touch-callout:none] " +
  "[@media(hover:none)]:select-none";
/** The cover's lift under the pointer, and the line around it while the card is picked. */
const liftClass =
  "rounded-cover transition-transform duration-150 group-hover:-translate-y-0.5 " +
  "motion-reduce:transition-none group-data-picked/card:outline-3 " +
  "group-data-picked/card:outline-accent group-data-picked/card:outline-offset-2";

/**
 * The one card of the app: a cover and up to three lines under it, for a
 * book, a series, a group of a series, a reading list or a kept page, on
 * every page that shows them.
 *
 * The lines are always the same three. `name` is what the card is called,
 * on two lines at most. `under` is one line that tells it from its
 * neighbours, left out when there is nothing to say. `standing` is the last
 * line, how much of it there is or how far it was read, and it sits at the
 * foot of the card: a row of cards whose names take one line or two still
 * ends on one line. How far a book was read is drawn on the cover's lower
 * edge (`done`, from 0 to 1), the same place on every card. Text too long
 * for the card shows whole under the pointer.
 *
 * `link` is where the card leads; one with none leads nowhere. `about` says
 * what the card is to someone who cannot see it, for its tick: its name,
 * unless the page around the card says part of it.
 * `menu` goes in the corner of the cover, and the tick that picks the card
 * in the other, unless it is not `pickable`.
 */
export function Card({
  what,
  name,
  about = name,
  pickable = true,
  link,
  cover,
  covers,
  picture,
  unreadable = false,
  done,
  titleAs: Title = "h3",
  under,
  standing,
  menu,
}: {
  what: Pickable;
  name: string;
  about?: string;
  pickable?: boolean;
  link?: LinkProps;
  cover?: string | null;
  /** The covers a reading list shows together as one: see `Cover`. */
  covers?: string[];
  /** The address of a picture shown in the cover's place: see `Cover`. */
  picture?: string;
  unreadable?: boolean;
  done?: number;
  /** The heading the name is: one level under the heading above the cards. */
  titleAs?: "h2" | "h3";
  under?: string | null;
  standing: ReactNode;
  menu?: ReactNode;
}) {
  const pick = usePick(what, about, pickable);
  const inside = (
    <>
      <div className={liftClass}>
        <Cover
          cover={cover}
          covers={covers}
          picture={picture}
          unreadable={unreadable}
          done={done}
        />
      </div>
      <Title className="mt-2 line-clamp-2 font-semibold text-metadata" title={name}>
        {name}
      </Title>
      {under ? (
        <p className="truncate text-caption text-fg-muted" title={under}>
          {under}
        </p>
      ) : null}
      <div className="mt-auto">{standing}</div>
    </>
  );
  return (
    <li className={cardClass} {...pick.card}>
      {link ? (
        <Link {...link} className={insideClass} {...pick.link}>
          {inside}
        </Link>
      ) : (
        <div className={insideClass}>{inside}</div>
      )}
      {pick.tick}
      {menu}
    </li>
  );
}
