import type { ReactNode } from "react";
import { coverGridClass } from "./Cover";
import { navHeadingClass } from "./NavLink";

type Made = { id: number; mine: boolean };

/** Some of them as cards, under a heading when there are two kinds to tell apart. */
function Some<T extends Made>({
  heading,
  things,
  card,
}: {
  heading: string | null;
  things: T[];
  card: (thing: T, titleAs: "h2" | "h3") => ReactNode;
}) {
  if (things.length === 0) {
    return null;
  }
  return (
    <section className="mt-8">
      {heading ? <h2 className={`mb-3 ${navHeadingClass}`}>{heading}</h2> : null}
      <ul className={coverGridClass}>
        {things.map((thing) => card(thing, heading ? "h3" : "h2"))}
      </ul>
    </section>
  );
}

/**
 * The reading lists or the collections a person sees, as cards: their own,
 * and apart from them the ones other people shared with them. `card` is the
 * card of one, its name the heading it is told to be.
 */
export function YoursAndShared<T extends Made>({
  things,
  card,
}: {
  things: T[];
  card: (thing: T, titleAs: "h2" | "h3") => ReactNode;
}) {
  const mine = things.filter((thing) => thing.mine);
  const shared = things.filter((thing) => !thing.mine);
  return (
    <>
      <Some heading={shared.length > 0 ? "Yours" : null} things={mine} card={card} />
      <Some heading="Shared with you" things={shared} card={card} />
    </>
  );
}
