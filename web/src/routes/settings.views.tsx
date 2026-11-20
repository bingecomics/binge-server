import { createFileRoute, Link, Navigate } from "@tanstack/react-router";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import type { View } from "../api/client";
import { useLibraries } from "../api/libraries";
import { linkTo, useDeleteView, useViews } from "../api/views";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ListRow, Section } from "../components/Section";
import { ErrorText, textLinkClass } from "../components/text";
import { ViewDialog } from "../components/ViewDialog";

export const Route = createFileRoute("/settings/views")({ component: ViewsPage });

/** Where a view shows, in a few words. */
function shownIn(view: View): string {
  if (view.on_home && view.in_menu) {
    return "A row on Home, and in the menu";
  }
  if (view.on_home) {
    return "A row on Home";
  }
  return view.in_menu ? "In the menu" : "Not shown anywhere";
}

/**
 * The views the server keeps: each a library narrowed and ordered, under a
 * name. They are made on a library's page, or by an app the owner gave a
 * token to; here they are renamed, placed and deleted. The owner's page.
 */
function ViewsPage() {
  const isOwner = useIsOwner();
  const views = useViews();
  const libraries = useLibraries();
  const remove = useDeleteView();
  const [changing, setChanging] = useState<View | null>(null);
  const [deleting, setDeleting] = useState<View | null>(null);

  if (!isOwner) {
    return <Navigate to="/settings/account" replace />;
  }
  const libraryOf = (view: View) =>
    libraries.data?.find((library) => library.id === view.library_id)?.name;
  return (
    <Section title="Views">
      <p className="mb-3 text-fg-muted text-metadata">
        A view is a library in an order and narrowed, kept under a name: “Unread Vertigo of the
        nineties”. Make one on a library's page with Keep view. Everyone who reads the library sees
        it.
      </p>
      {views.isError ? (
        <ErrorText>{views.error.message}</ErrorText>
      ) : views.data?.length === 0 ? (
        <p className="text-fg-muted">None yet.</p>
      ) : (
        <ul>
          {views.data?.map((view) => (
            <ListRow
              key={view.id}
              title={
                <Link {...linkTo(view)} className={textLinkClass}>
                  {view.name}
                </Link>
              }
              detail={[libraryOf(view), shownIn(view)].filter(Boolean).join(" · ")}
            >
              <Button onClick={() => setChanging(view)}>
                Change<span className="sr-only"> {view.name}</span>
              </Button>
              <Button variant="quiet" onClick={() => setDeleting(view)}>
                Delete<span className="sr-only"> {view.name}</span>
              </Button>
            </ListRow>
          ))}
        </ul>
      )}
      <ViewDialog
        open={changing !== null}
        onClose={() => setChanging(null)}
        view={changing ?? undefined}
      />
      <ConfirmDialog
        open={deleting !== null}
        onClose={() => {
          setDeleting(null);
          remove.reset();
        }}
        title="Delete this view?"
        confirm="Delete"
        confirming="Deleting…"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => {
          if (deleting) {
            remove.mutate(deleting.id, { onSuccess: () => setDeleting(null) });
          }
        }}
      >
        “{deleting?.name}” goes from Home and the menu for everyone. The library and its books stay
        as they are.
      </ConfirmDialog>
    </Section>
  );
}
