import type { Page } from "../api/client";

/**
 * Groups page numbers into what is on screen together, in reading order.
 *
 * One page at a time, or in pairs when `twoPages`. In pairs, the cover stands
 * alone, so the pages after it face each other the way they do in print, and
 * so does any page wider than it is tall, which is two pages drawn as one. A
 * page whose size is not known counts as an ordinary one.
 */
export function spreads(pages: Page[], twoPages: boolean): number[][] {
  if (!twoPages) {
    return pages.map((page) => [page.number]);
  }
  const alone = (page: Page) => page.number === 0 || (page.width ?? 0) > (page.height ?? 0);
  const groups: number[][] = [];
  let index = 0;
  while (index < pages.length) {
    const page = pages[index] as Page;
    const next = pages[index + 1];
    if (alone(page) || !next || alone(next)) {
      groups.push([page.number]);
      index += 1;
    } else {
      groups.push([page.number, next.number]);
      index += 2;
    }
  }
  return groups;
}
