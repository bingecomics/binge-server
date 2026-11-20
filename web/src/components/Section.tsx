import type { ReactNode } from "react";

/** A row of a list inside a section, with a line above every row but the first. */
export const rowClass = "border-divider border-t py-3 first:border-t-0";

/** A row with something at each end. */
export const rowSidesClass = "flex items-baseline justify-between gap-6";

/** A titled card of a settings page. `action` sits at the end of the title's line. */
export function Section({
  title,
  action,
  children,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="mt-6 max-w-2xl rounded-card border border-divider bg-surface px-5 py-6 md:px-8">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <h2 className="font-semibold text-section-title">{title}</h2>
        {action}
      </div>
      {children}
    </section>
  );
}

/**
 * A row of a `<ul>`: what a thing is called, a line about it, and what can be done with it.
 * On a phone the actions sit under the text in every row; from `sm` up they sit at the end
 * and the text wraps, so rows look the same whatever the length of their text.
 */
export function ListRow({
  title,
  detail,
  children,
}: {
  /** A name, or a link named so. */
  title: ReactNode;
  detail: ReactNode;
  children?: ReactNode;
}) {
  return (
    <li className={`${rowClass} flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-6`}>
      <div className="min-w-0 sm:flex-1">
        <p className="truncate font-semibold">{title}</p>
        <p className="text-caption text-fg-muted">{detail}</p>
      </div>
      {children ? <div className="flex shrink-0 gap-2">{children}</div> : null}
    </li>
  );
}

/** A name and its value, as a row of a `<dl>`. */
export function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={`${rowClass} ${rowSidesClass}`}>
      <dt className="text-fg-muted text-metadata">{label}</dt>
      <dd className="text-right">{children}</dd>
    </div>
  );
}
