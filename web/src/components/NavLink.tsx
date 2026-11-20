import { Link } from "@tanstack/react-router";
import type { ComponentType } from "react";
import type { FileRouteTypes } from "../routeTree.gen";

/** How a link of a menu beside the page looks, and the one for the page being shown. */
export const navLinkClass =
  "flex min-h-11 shrink-0 items-center gap-3 rounded-control px-3 text-metadata text-fg-muted " +
  "transition-colors duration-150 hover:bg-raised hover:text-fg " +
  "data-[status=active]:bg-raised data-[status=active]:font-semibold data-[status=active]:text-fg";

/** How a button that is only an icon looks. */
/**
 * For a title the menu beside the page already shows, such as a library's
 * name: it is only read out where that menu is.
 */
export const saidByTheMenuClass = "md:sr-only";

export const iconButtonClass =
  "grid shrink-0 place-items-center rounded-control text-fg-muted " +
  "transition-colors duration-150 hover:bg-raised hover:text-fg";

/**
 * The column beside the page that holds a menu, under the bar at the top.
 * Wide windows only: see each menu for what a narrow one gets.
 */
export const sideMenuClass =
  "border-divider bg-surface md:sticky md:top-14 md:h-[calc(100dvh-3.5rem)] md:border-r";

/** The small heading over a part of a menu. */
export const navHeadingClass = "font-semibold text-caption text-fg-muted uppercase tracking-wider";

/** Routes that need nothing filled in. */
type PlainRoute = Exclude<FileRouteTypes["to"], `${string}$${string}`>;

/**
 * A link of a menu, marked while its page is the one shown. `withPagesUnder`
 * keeps it marked on the pages under its own, as a list's page is under the
 * page of all lists.
 */
export function NavLink({
  to,
  icon: Icon,
  label,
  withPagesUnder = false,
}: {
  to: PlainRoute;
  /** One of the icon set's, or one of Binge's own. */
  icon: ComponentType<{ className?: string }>;
  label: string;
  withPagesUnder?: boolean;
}) {
  return (
    <Link
      to={to}
      className={navLinkClass}
      activeOptions={{ exact: !withPagesUnder, includeSearch: false }}
    >
      <Icon className="size-5 shrink-0" />
      <span className="whitespace-nowrap">{label}</span>
    </Link>
  );
}
