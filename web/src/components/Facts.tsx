import { Link, type LinkProps } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { quietLinkClass } from "./text";

/** A list of what is known about something, each fact a `Fact` with its label. */
export function Facts({ className = "", children }: { className?: string; children: ReactNode }) {
  return (
    <dl
      className={`grid max-w-prose grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-2 text-metadata ${className}`}
    >
      {children}
    </dl>
  );
}

export function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-fg-muted">{label}</dt>
      <dd className="break-words">{children}</dd>
    </>
  );
}

/**
 * What something is in (reading lists, collections), each leading to its
 * page: one fact, or nothing when it is in none. `label` is the fact's name
 * for one and for several.
 */
export function In<T extends { id: number; name: string }>({
  label,
  things,
  link,
}: {
  label: [string, string];
  things: T[] | undefined;
  link: (thing: T) => LinkProps;
}) {
  if (!things?.length) {
    return null;
  }
  return (
    <Fact label={things.length === 1 ? label[0] : label[1]}>
      <ul>
        {things.map((thing) => (
          <li key={thing.id}>
            <Link {...link(thing)} className={quietLinkClass}>
              {thing.name}
            </Link>
          </li>
        ))}
      </ul>
    </Fact>
  );
}
