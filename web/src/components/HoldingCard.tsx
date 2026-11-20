import type { LinkProps } from "@tanstack/react-router";
import type { Pickable } from "../selection";
import { Card } from "./Card";
import { Held } from "./Held";
import { BooksMenu } from "./ReadMarks";

/**
 * The card of something that holds books: a series, a group of one, or a
 * reading list. It opens what it holds; the menu on its cover marks all its
 * books, and its tick picks it. One that holds no books has neither.
 *
 * `titleAs` is the heading its name is: one level under the heading above
 * the cards.
 */
export function HoldingCard({
  what,
  link,
  cover,
  covers,
  titleAs,
  name,
  under,
  bookCount,
  unreadCount,
}: {
  what: Pickable;
  link: LinkProps;
  cover?: string | null;
  covers?: string[];
  titleAs?: "h2" | "h3";
  name: string;
  /** One line that tells it from its neighbours, when there is one to say. */
  under?: string | null;
  bookCount: number;
  unreadCount: number;
}) {
  const holds = bookCount > 0;
  return (
    <Card
      what={what}
      pickable={holds}
      link={link}
      cover={cover}
      covers={covers}
      titleAs={titleAs}
      name={name}
      under={under}
      standing={<Held bookCount={bookCount} unreadCount={unreadCount} />}
      menu={
        holds ? (
          <BooksMenu
            corner
            about={name}
            what={{ [what.kind]: [what.id] }}
            unreadCount={unreadCount}
          />
        ) : null
      }
    />
  );
}
