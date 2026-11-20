import { Link, type LinkProps } from "@tanstack/react-router";
import { ChevronLeft, ChevronRight } from "lucide-react";
import type { ReactNode } from "react";
import { buttonClass } from "./Button";

/** A way to another page of a list, or the same thing greyed when there is none that way. */
function Turn({ to, children }: { to: LinkProps | null; children: ReactNode }) {
  if (to === null) {
    return (
      <span aria-disabled="true" className={`${buttonClass()} opacity-40`}>
        {children}
      </span>
    );
  }
  return (
    <Link {...to} className={buttonClass()}>
      {children}
    </Link>
  );
}

/**
 * The page of a list that an address asks for, from what it says under
 * `page`. Left out for the first, so the first page has one address.
 */
export function pageAsked(said: unknown): number | undefined {
  const page = Number(said);
  return Number.isSafeInteger(page) && page > 1 ? page : undefined;
}

/**
 * The way through the pages of a list, under it. Nothing for a list that
 * fits on one page.
 */
export function Pages({
  label,
  page,
  total,
  aPage,
  to,
}: {
  /** What the pages are of, read out: "Pages of series". */
  label: string;
  /** The page shown, from 1. */
  page: number;
  /** How many things the list holds. */
  total: number;
  /** How many of them a page shows. */
  aPage: number;
  /** Where a page of the list is. */
  to: (page: number) => LinkProps;
}) {
  const last = Math.ceil(total / aPage);
  if (last <= 1) {
    return null;
  }
  return (
    <nav aria-label={label} className="mt-10 flex items-center justify-center gap-4">
      {/* From a page past the last, as an address kept from a longer list, back is the last. */}
      <Turn to={page > 1 ? to(Math.min(page - 1, last)) : null}>
        <ChevronLeft className="size-4" aria-hidden="true" />
        Previous
      </Turn>
      <p className="text-fg-muted text-metadata" aria-current="page">
        Page {page.toLocaleString()} of {last.toLocaleString()}
      </p>
      <Turn to={page < last ? to(page + 1) : null}>
        Next
        <ChevronRight className="size-4" aria-hidden="true" />
      </Turn>
    </nav>
  );
}
