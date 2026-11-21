import { Link, useLocation } from "@tanstack/react-router";
import { BookUser, Heart, HeartOff, Layers, ListOrdered } from "lucide-react";
import { choiceClass } from "./Button";
import { navLinkClass } from "./NavLink";
import { WantToReadIcon } from "./WantToRead";

/** What the place of a person's own is called, wherever it is named. */
export const MY_BINGE = "My Binge";

/** What stands for My Binge wherever it is named. */
export const MyBingeIcon = BookUser;

/** What stands for a favorite page wherever one is named, and for taking one off. */
export const FavoriteIcon = Heart;
export const NoFavoriteIcon = HeartOff;

/**
 * The parts of My Binge, in the order they are shown: what a person wants
 * to read, the reading lists and the collections they see, and the pages
 * they made favorites.
 */
const PARTS = [
  { to: "/want-to-read", label: "Want to read", icon: WantToReadIcon },
  { to: "/lists", label: "Reading lists", icon: ListOrdered },
  { to: "/collections", label: "Collections", icon: Layers },
  { to: "/favorites", label: "Favorites", icon: FavoriteIcon },
] as const;

/** Where My Binge opens: its first part. */
export const MY_BINGE_OPENS = PARTS[0].to;

/**
 * The entry of the menu beside the page that leads to My Binge. It is
 * marked on every page of My Binge, a list's own page included, which is
 * more than the page it leads to.
 */
export function MyBingeLink() {
  const page = useLocation({ select: (location) => location.pathname });
  const shown = PARTS.some(({ to }) => page === to || page.startsWith(`${to}/`));
  return (
    <Link
      to={MY_BINGE_OPENS}
      className={navLinkClass}
      data-status={shown ? "active" : undefined}
      aria-current={shown ? "true" : undefined}
    >
      <MyBingeIcon className="size-5 shrink-0" />
      <span className="whitespace-nowrap">{MY_BINGE}</span>
    </Link>
  );
}

/**
 * The parts of My Binge as a row of choices, over the page of each of
 * them. The one chosen says which page this is, so the page's own title is
 * only read out. A narrow window has no menu beside the page to say that
 * this is My Binge, so the row says it there.
 */
export function MyBingeParts() {
  return (
    <nav aria-label={MY_BINGE} className="mb-8">
      <p aria-hidden="true" className="mb-3 font-bold text-screen-title tracking-tight md:hidden">
        {MY_BINGE}
      </p>
      <ul className="flex flex-wrap gap-2">
        {PARTS.map(({ to, label, icon: Icon }) => (
          <li key={to}>
            <Link
              to={to}
              activeOptions={{ exact: true, includeSearch: false }}
              className={choiceClass}
            >
              <Icon className="size-4 shrink-0" />
              {label}
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}
