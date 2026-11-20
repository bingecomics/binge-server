import type { ReactNode } from "react";

/** How a link inside running text looks. */
export const textLinkClass = "text-accent underline underline-offset-4";

/**
 * How a link looks where several sit side by side in a line of facts, such
 * as the people a book names: marked as links without each shouting.
 */
export const quietLinkClass =
  "underline decoration-outline/50 underline-offset-4 transition-colors duration-150 " +
  "hover:text-accent hover:decoration-accent";

/** Something that went wrong, announced to screen readers when it appears. */
export function ErrorText({
  id,
  className = "",
  children,
}: {
  id?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <p id={id} role="alert" className={`text-danger text-metadata ${className}`}>
      {children}
    </p>
  );
}

/** A folder path, which may break anywhere to fit. */
export function PathText({ path, className = "" }: { path: string; className?: string }) {
  return <p className={`break-all font-mono text-caption text-fg-muted ${className}`}>{path}</p>;
}

/** How the link above a page's title looks: the way back up to what holds the page. */
export const upLinkClass =
  "-ml-1 inline-flex min-h-11 items-center gap-1 rounded-control text-fg-muted text-metadata hover:text-fg";
