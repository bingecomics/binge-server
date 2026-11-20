import { memo } from "react";
import type { Group } from "../api/client";
import { groupName } from "../format";
import { HoldingCard } from "./HoldingCard";

/**
 * A group of a series as a card, whatever its kind: a volume, a limited
 * series, the annuals, the one-shots. It opens the group's own page.
 */
// Memoized: a series' page is drawn again whenever its books change.
export const GroupCard = memo(function GroupCard({ group }: { group: Group }) {
  return (
    <HoldingCard
      what={{ kind: "groups", id: group.id }}
      link={{
        to: "/series/$seriesId/groups/$groupId",
        params: { seriesId: group.series_id, groupId: group.id },
      }}
      cover={group.cover}
      name={groupName(group)}
      bookCount={group.book_count}
      unreadCount={group.unread_count}
    />
  );
});
