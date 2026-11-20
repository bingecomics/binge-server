import { memo } from "react";
import type { ReadingList } from "../api/client";
import { madeBy, years } from "../format";
import { HoldingCard } from "./HoldingCard";

/**
 * A reading list as a card. It opens the list. Until a cover is chosen for
 * it, the covers of its first books stand for it together: `covers`. Under its name
 * are the years its books span, and whose it is when it is someone else's.
 */
export const ListCard = memo(function ListCard({
  list,
  titleAs = "h2",
}: {
  list: ReadingList;
  titleAs?: "h2" | "h3";
}) {
  const under = [years(list.first_year, list.last_year), madeBy(list)].filter(Boolean).join(" · ");
  return (
    <HoldingCard
      what={{ kind: "lists", id: list.id }}
      link={{ to: "/lists/$listId", params: { listId: list.id } }}
      cover={list.cover}
      covers={list.covers}
      titleAs={titleAs}
      name={list.name}
      under={under}
      bookCount={list.book_count}
      unreadCount={list.unread_count}
    />
  );
});
