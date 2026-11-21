/**
 * The rows of Home and how a person arranges them: the order they come in
 * and which are put away. The server keeps the arrangement for each person
 * under `HOME_ROWS_KEPT_AS`, as JSON, so Home is the same in another
 * browser.
 *
 * A row is named by an id: one of Home's own, or `collection:` and the id
 * of a collection that is a row on Home. The libraries end the page and are
 * not part of it: a narrow window has no other way to them.
 */
export const HOME_ROWS_KEPT_AS = "home.rows";

export type Arrangement = {
  /** The ids of the rows, in the order chosen. A row not named comes where it would by itself. */
  order: string[];
  /** The ids of the rows put away. */
  hidden: string[];
};

/** The id of the row a collection makes. */
export function collectionRow(collectionId: number): string {
  return `collection:${collectionId}`;
}

function ids(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((one) => typeof one === "string") : [];
}

/** Reads an arrangement as the server keeps it. Anything else is no arrangement. */
export function arrangementIn(kept: string | undefined): Arrangement {
  try {
    const read: unknown = JSON.parse(kept ?? "{}");
    const { order, hidden } = (read ?? {}) as Record<string, unknown>;
    return { order: ids(order), hidden: ids(hidden) };
  } catch {
    return { order: [], hidden: [] };
  }
}

/** An arrangement as the server keeps it. */
export function keptAs(arrangement: Arrangement): string {
  return JSON.stringify(arrangement);
}

/**
 * The rows there are, in the order an arrangement gives them. `rows` come
 * in the order they have by themselves. One the arrangement does not name,
 * as a collection put on Home since, goes after the row it follows by itself.
 */
export function arranged<Row extends { id: string }>(rows: Row[], order: string[]): Row[] {
  const placed = (id: string) => rows.find((row) => row.id === id);
  const result = order.map(placed).filter((row) => row !== undefined);
  rows.forEach((row, place) => {
    if (result.includes(row)) {
      return;
    }
    // After the nearest row before it that is already placed, or first.
    const before = rows
      .slice(0, place)
      .reverse()
      .find((one) => result.includes(one));
    result.splice(before === undefined ? 0 : result.indexOf(before) + 1, 0, row);
  });
  return result;
}
