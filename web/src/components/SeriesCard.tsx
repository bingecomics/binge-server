import { memo, type ReactNode } from "react";
import type { Series } from "../api/client";
import { HoldingCard } from "./HoldingCard";

/**
 * A series as a card. It opens the series' books.
 *
 * `titleAs` is the heading its name is: one level under the heading above the
 * cards.
 */
// Memoized: the pages that list series change every second while a scan runs.
export const SeriesCard = memo(function SeriesCard({
  series,
  titleAs = "h2",
  more,
}: {
  series: Series;
  titleAs?: "h2" | "h3";
  /** What else its menu offers on the page it is shown on. */
  more?: ReactNode;
}) {
  return (
    <HoldingCard
      what={{ kind: "series", id: series.id }}
      link={{ to: "/series/$seriesId", params: { seriesId: series.id } }}
      cover={series.cover}
      titleAs={titleAs}
      name={series.name}
      bookCount={series.book_count}
      unreadCount={series.unread_count}
      wanted={series.wanted}
      more={more}
    />
  );
});
