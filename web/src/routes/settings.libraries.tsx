import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import { useLibraries } from "../api/libraries";
import { AddLibraryDialog } from "../components/AddLibraryDialog";
import { Button } from "../components/Button";
import { rowClass, rowSidesClass, Section } from "../components/Section";
import { PathText, textLinkClass } from "../components/text";
import { books } from "../format";

export const Route = createFileRoute("/settings/libraries")({ component: LibrariesPage });

/** The libraries of the server and where each is. Adding one is the owner's to do. */
function LibrariesPage() {
  const libraries = useLibraries();
  const isOwner = useIsOwner();
  const [adding, setAdding] = useState(false);

  return (
    <Section
      title="Libraries"
      action={isOwner ? <Button onClick={() => setAdding(true)}>Add a library</Button> : null}
    >
      {libraries.data?.length === 0 ? (
        <p className="text-fg-muted">
          {isOwner ? "None yet." : "None yet. The owner decides what you read."}
        </p>
      ) : (
        <ul>
          {libraries.data?.map((library) => (
            <li key={library.id} className={rowClass}>
              <div className={rowSidesClass}>
                <Link
                  to="/libraries/$libraryId"
                  params={{ libraryId: library.id }}
                  className={`font-semibold ${textLinkClass}`}
                >
                  {library.name}
                </Link>
                <span className="shrink-0 text-fg-muted text-metadata">
                  {books(library.book_count)}
                </span>
              </div>
              {library.path ? <PathText path={library.path} className="mt-1" /> : null}
            </li>
          ))}
        </ul>
      )}
      {isOwner ? <AddLibraryDialog open={adding} onClose={() => setAdding(false)} /> : null}
    </Section>
  );
}
