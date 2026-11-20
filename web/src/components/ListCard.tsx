import { memo } from "react";
import type { ReadingList } from "../api/client";
import { years } from "../format";
import { HoldingCard } from "./HoldingCard";

/**
 * A reading list as a card. It opens the list. Until a cover is chosen for
 * it, the covers of its first books stand for it together: `covers`. Under its name
 * are the years its books span.
 */
export const ListCard = memo(function ListCard({
  list,
  titleAs = "h2",
}: {
  list: ReadingList;
  titleAs?: "h2" | "h3";
}) {
  return (
    <HoldingCard
      what={{ kind: "lists", id: list.id }}
      link={{ to: "/lists/$listId", params: { listId: list.id } }}
      cover={list.cover}
      covers={list.covers}
      titleAs={titleAs}
      name={list.name}
      under={years(list.first_year, list.last_year)}
      bookCount={list.book_count}
      unreadCount={list.unread_count}
    />
  );
});
