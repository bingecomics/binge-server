import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Layers, Plus } from "lucide-react";
import { useState } from "react";
import { gotNoAnswer } from "../api/client";
import { useCollections, useCreateCollection } from "../api/collections";
import { Button } from "../components/Button";
import { CollectionCard } from "../components/CollectionCard";
import { MyBingeParts } from "../components/MyBinge";
import { NameAndSummaryDialog } from "../components/NameAndSummary";
import { Notice, ServerNotAnswering } from "../components/Notice";
import { ErrorText } from "../components/text";
import { YoursAndShared } from "../components/YoursAndShared";
import { count } from "../format";

export const Route = createFileRoute("/collections/")({ component: CollectionsPage });

/**
 * The collections a person sees: their own, and apart from them the ones
 * other people shared with them. Anyone makes a collection.
 */
function CollectionsPage() {
  const collections = useCollections();
  const create = useCreateCollection();
  const navigate = useNavigate();
  const [making, setMaking] = useState(false);

  if (collections.isPending) {
    return null;
  }
  if (collections.isError) {
    return gotNoAnswer(collections.error) ? (
      <ServerNotAnswering />
    ) : (
      <ErrorText>{collections.error.message}</ErrorText>
    );
  }

  return (
    <>
      <MyBingeParts />
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <div className="min-w-0">
          <h1 className="sr-only">Collections</h1>
          <p className="text-fg-muted text-metadata">
            {count(collections.data.length, "collection", "collections")}
          </p>
        </div>
        <Button variant="primary" onClick={() => setMaking(true)}>
          <Plus className="size-5" aria-hidden="true" />
          New collection
        </Button>
      </div>

      {collections.data.length === 0 ? (
        <div className="mt-8">
          <Notice icon={Layers} title="No collections yet">
            A collection puts series of any library together under a name: a publisher’s event, a
            writer’s work, what to read with the children. Make one here and add series from their
            menus. It is yours alone until you share it.
          </Notice>
        </div>
      ) : (
        <YoursAndShared
          things={collections.data}
          card={(collection, titleAs) => (
            <CollectionCard key={collection.id} collection={collection} titleAs={titleAs} />
          )}
        />
      )}

      <NameAndSummaryDialog
        open={making}
        onClose={() => setMaking(false)}
        title="New collection"
        save={create}
        saveLabel="Make the collection"
        onSaved={(collection) =>
          navigate({ to: "/collections/$collectionId", params: { collectionId: collection.id } })
        }
      />
    </>
  );
}
