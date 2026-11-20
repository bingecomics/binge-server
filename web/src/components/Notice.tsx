import { Link } from "@tanstack/react-router";
import { FileWarning, SearchX, ServerCrash } from "lucide-react";
import type { ComponentType, ReactNode } from "react";
import { ApiProblem, gotNoAnswer } from "../api/client";
import { textLinkClass } from "./text";

/** A card that says where things stand when there is nothing else to show. */
export function Notice({
  icon: Icon,
  title,
  children,
  action,
}: {
  /** One of the icon set's, or one of Binge's own. */
  icon: ComponentType<{ className?: string }>;
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="max-w-2xl rounded-card border border-divider bg-surface p-8">
      <Icon className="size-10 text-accent" />
      <h2 className="mt-4 font-semibold text-section-title">{title}</h2>
      <p className="mt-2 text-fg-muted">{children}</p>
      {action ? <div className="mt-6">{action}</div> : null}
    </section>
  );
}

/** For an address that names something the server does not have. */
export function NotOnServer({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Notice
      icon={SearchX}
      title={title}
      action={
        <Link to="/" className={textLinkClass}>
          Back to Home
        </Link>
      }
    >
      {children}
    </Notice>
  );
}

export function ServerNotAnswering() {
  return (
    <Notice icon={ServerCrash} title="The server is not answering">
      Check that it is still running, then reload this page.
    </Notice>
  );
}

/** For a book the server did not hand over: why, from the error asking for it gave. */
export function BookNotOpened({ error }: { error: Error }) {
  if (gotNoAnswer(error)) {
    return <ServerNotAnswering />;
  }
  if (error instanceof ApiProblem && error.code === "not_found") {
    return (
      <NotOnServer title="This book is not on the server">
        It may have been removed, or its file may have been moved. A moved file is found again by
        scanning its library.
      </NotOnServer>
    );
  }
  return (
    <Notice icon={FileWarning} title="This book could not be opened">
      {error.message}
    </Notice>
  );
}
