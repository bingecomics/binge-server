import { Link } from "@tanstack/react-router";
import { Fragment, type ReactNode } from "react";
import type { Credit } from "../api/client";
import { type NarrowedBy, type ValueOf, withStanding } from "../narrowing";
import { quietLinkClass } from "./text";

/**
 * A fact about a book or a series that leads to its library narrowed by
 * it: the series of that publisher, of that year, by that writer. Books of
 * several libraries, as in a reading list, have no library to narrow: the
 * fact is then only said.
 */
export function NarrowedLink<By extends NarrowedBy>({
  libraryId,
  by,
  value,
  children,
}: {
  libraryId: number | undefined;
  by: By;
  value: ValueOf[By];
  children: ReactNode;
}) {
  if (libraryId === undefined) {
    return children;
  }
  return (
    <Link
      to="/libraries/$libraryId"
      params={{ libraryId }}
      search={withStanding({}, by, value, "asked")}
      className={quietLinkClass}
    >
      {children}
    </Link>
  );
}

/**
 * The people some books name for one job, a comma between them. Each leads
 * to the books' library narrowed by them, or to their own page where the
 * books are of several libraries.
 */
export function PeopleLinks({
  libraryId,
  by,
  people,
}: {
  libraryId: number | undefined;
  by: "writer" | "penciller";
  people: Credit[];
}) {
  return people.map((person, place) => (
    <Fragment key={person.id}>
      {place > 0 ? ", " : null}
      {libraryId === undefined ? (
        <Link to="/people/$personId" params={{ personId: person.id }} className={quietLinkClass}>
          {person.name}
        </Link>
      ) : (
        <NarrowedLink libraryId={libraryId} by={by} value={person.id}>
          {person.name}
        </NarrowedLink>
      )}
    </Fragment>
  ));
}
