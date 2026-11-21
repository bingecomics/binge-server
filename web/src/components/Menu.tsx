import { createLink } from "@tanstack/react-router";
import { Check, EllipsisVertical, type LucideIcon } from "lucide-react";
import {
  type AnchorHTMLAttributes,
  type CSSProperties,
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { flushSync } from "react-dom";
import { buttonClass } from "./Button";
import { cardCornerClass } from "./Cover";
import { iconButtonClass } from "./NavLink";

/** Room kept between a menu and the edges of the window, in pixels. */
const EDGE = 8;
/** Room between a menu and its button, in pixels. */
const GAP = 4;
/** How wide a menu is, in pixels: its longest choices fit on one line. */
const WIDTH = 256;
/** Room enough under a button for most menus, five choices tall, in pixels. */
const ROOM = 240;

/**
 * Where a menu goes: under its button and starting where the button starts.
 * With little room under the button and more over it, it goes over the
 * button. Near the right edge it ends where the button ends, or failing
 * that starts at the left edge. It is never taller than the room it has:
 * what does not fit is scrolled to. Worked out from the button alone, so
 * the menu is in place before it shows.
 */
function placeBy(button: HTMLElement): CSSProperties {
  const at = button.getBoundingClientRect();
  const below = window.innerHeight - at.bottom;
  const under = below >= ROOM || below >= at.top;
  const fromStart = at.left + WIDTH + EDGE <= window.innerWidth;
  const toEnd = !fromStart && at.right - WIDTH >= EDGE;
  return {
    width: WIDTH,
    maxHeight: (under ? below : at.top) - GAP - EDGE,
    top: under ? at.bottom + GAP : "auto",
    bottom: under ? "auto" : window.innerHeight - at.top + GAP,
    left: fromStart ? at.left : toEnd ? "auto" : EDGE,
    right: toEnd ? window.innerWidth - at.right : "auto",
  };
}

/**
 * Whether a menu or a dialog is open over the page. It takes the keys for
 * itself, Escape first: what listens to the keys of the whole window asks
 * this, and leaves them alone when it is.
 */
export function isOpenOverPage(): boolean {
  return document.querySelector(":popover-open, dialog[open]") !== null;
}

/**
 * A button that opens a short list of things to do. The list shows over
 * everything else, also when the button sits in a row that scrolls. It
 * closes on a choice, on Escape, on a click elsewhere, when the keyboard
 * leaves it, and when the page moves under it.
 *
 * `children` are the choices: `MenuItem`s and `MenuLink`s. They are only
 * there while the menu is open.
 */
export function Menu({
  label,
  title,
  className,
  button,
  children,
}: {
  /** What the button is for, read out. */
  label: string;
  /** What the button says under the pointer. */
  title?: string;
  /** How the button looks. */
  className: string;
  /** What the button shows. */
  button: ReactNode;
  children: ReactNode;
}) {
  const id = useId();
  const opener = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<CSSProperties | null>(null);
  const open = place !== null;

  useEffect(() => {
    if (!open) {
      return;
    }
    // The menu stays where it was put, so it goes when its button moves:
    // when the window changes size, or something the button is in scrolls.
    function close(event: Event) {
      const moved = event.target;
      if (event.type === "resize" || (moved instanceof Node && moved.contains(opener.current))) {
        menu.current?.hidePopover();
      }
    }
    window.addEventListener("scroll", close, { capture: true, passive: true });
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("scroll", close, { capture: true });
      window.removeEventListener("resize", close);
    };
  }, [open]);

  return (
    <>
      <button
        ref={opener}
        type="button"
        popoverTarget={id}
        aria-label={label}
        title={title}
        className={className}
      >
        {button}
      </button>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: it hears the clicks of its choices, which are buttons and links */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: a choice made with the keyboard is a click too */}
      <div
        ref={menu}
        id={id}
        popover="auto"
        style={place ?? undefined}
        onBeforeToggle={(event) => {
          // Filled and in place before the browser shows it.
          if (event.newState === "open" && opener.current) {
            const at = placeBy(opener.current);
            flushSync(() => setPlace(at));
          }
        }}
        onToggle={(event) => {
          if (event.newState === "closed") {
            setPlace(null);
          }
        }}
        onClick={(event) => {
          // A choice was made.
          if ((event.target as Element).closest("button, a")) {
            menu.current?.hidePopover();
          }
        }}
        onBlur={(event) => {
          // The keyboard went on to something that is not the menu's.
          const to = event.relatedTarget;
          if (to && to !== opener.current && !event.currentTarget.contains(to)) {
            menu.current?.hidePopover();
          }
        }}
        className="fixed m-0 overflow-y-auto rounded-card border border-divider bg-surface p-2 text-fg shadow-lg"
      >
        {open ? children : null}
      </div>
    </>
  );
}

/**
 * The button in the corner of a card's cover. Where there is a pointer to
 * hover with, it shows while the pointer is over the card, and while its
 * menu is open. Where there is none it always shows.
 */
const cornerClass =
  `${cardCornerClass} right-1 rounded-full bg-surface text-fg shadow-md ring-1 ring-divider ` +
  "hover:text-accent [&:has(+:popover-open)]:opacity-100 [@media(hover:none)]:opacity-100";

/**
 * The menu of what can be done with something: `about` names it. On a card
 * it sits in the `corner` of the cover; on a page it is a button like the
 * ones beside it, or a `quiet` one in a row of a list, where a button on
 * every row would be all there is to see.
 */
export function MoreMenu({
  about,
  corner = false,
  quiet = false,
  children,
}: {
  about: string;
  corner?: boolean;
  quiet?: boolean;
  children: ReactNode;
}) {
  return (
    <Menu
      label={`More to do with ${about}`}
      title="More"
      className={
        corner ? cornerClass : quiet ? `${iconButtonClass} size-11` : `${buttonClass()} w-11 px-0`
      }
      button={<EllipsisVertical className="size-5" aria-hidden="true" />}
    >
      {children}
    </Menu>
  );
}

/** How a choice of a menu looks, and of a list of choices in a dialog. */
export const choiceClass =
  "flex min-h-11 w-full items-center gap-3 rounded-control px-3 text-left text-metadata text-fg " +
  "transition-colors duration-150 hover:bg-raised disabled:opacity-50";

/**
 * One thing to choose in a list of them, away from a menu: its name, with
 * a line under it that tells it from the others.
 */
export function ChoiceRow({
  name,
  under,
  disabled = false,
  onChoose,
}: {
  name: string;
  under?: string | null;
  disabled?: boolean;
  onChoose: () => void;
}) {
  return (
    <li>
      <button type="button" disabled={disabled} onClick={onChoose} className={choiceClass}>
        <span className="min-w-0 py-1">
          <span className="block truncate font-semibold">{name}</span>
          {under ? (
            <span className="block truncate text-caption text-fg-muted">{under}</span>
          ) : null}
        </span>
      </button>
    </li>
  );
}

/**
 * What a choice shows: its icon, and what it does. One of several that is
 * the one in force (`chosen`) ends in a tick.
 */
function Choice({
  icon: Icon,
  chosen = false,
  children,
}: {
  icon: LucideIcon;
  chosen?: boolean;
  children: ReactNode;
}) {
  return (
    <>
      <Icon className="size-5 shrink-0 text-fg-muted" aria-hidden="true" />
      <span className="min-w-0 flex-1">{children}</span>
      {chosen ? <Check className="size-4 shrink-0 text-accent" aria-hidden="true" /> : null}
    </>
  );
}

/** One thing to do, in a `Menu`. */
export function MenuItem({
  icon,
  onSelect,
  disabled = false,
  chosen = false,
  children,
}: {
  icon: LucideIcon;
  onSelect: () => void;
  disabled?: boolean;
  /** For one of several choices: this is the one in force. */
  chosen?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      disabled={disabled}
      aria-current={chosen ? "true" : undefined}
      className={choiceClass}
    >
      <Choice icon={icon} chosen={chosen}>
        {children}
      </Choice>
    </button>
  );
}

/** One file to save, in a `Menu`: the browser keeps what `href` answers. */
export function MenuDownload({
  icon,
  href,
  children,
}: {
  icon: LucideIcon;
  href: string;
  children: ReactNode;
}) {
  return (
    <a href={href} download className={choiceClass}>
      <Choice icon={icon}>{children}</Choice>
    </a>
  );
}

/** One place to go, in a `Menu`: a link of the router, by `to` and `params`. */
export const MenuLink = createLink(function MenuLink({
  icon,
  chosen = false,
  children,
  ...link
}: AnchorHTMLAttributes<HTMLAnchorElement> & {
  icon: LucideIcon;
  /** For one of several choices: this is the one in force. */
  chosen?: boolean;
}) {
  return (
    // The router marks a link to the page being shown; here the mark is the choice's.
    <a {...link} aria-current={chosen ? "true" : undefined} className={choiceClass}>
      <Choice icon={icon} chosen={chosen}>
        {children}
      </Choice>
    </a>
  );
});
