import { memo } from "react";
import type { Collection } from "../api/client";
import { madeBy, seriesCount } from "../format";
import { Card } from "./Card";

/**
 * A collection as a card. It opens the collection. The covers of its first
 * series stand for it together. Under its name is whose it is when it is
 * someone else's, and how many series it holds.
 */
export const CollectionCard = memo(function CollectionCard({
  collection,
  titleAs = "h2",
}: {
  collection: Collection;
  titleAs?: "h2" | "h3";
}) {
  return (
    <Card
      link={{ to: "/collections/$collectionId", params: { collectionId: collection.id } }}
      cover={collection.cover}
      covers={collection.covers}
      titleAs={titleAs}
      name={collection.name}
      under={madeBy(collection)}
      standing={
        <p className="text-caption text-fg-muted">{seriesCount(collection.series_count)}</p>
      }
    />
  );
});
