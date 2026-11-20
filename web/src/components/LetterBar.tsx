import { Link } from "@tanstack/react-router";
import type { Letter } from "../api/client";
import { count } from "../format";
import { type Asked, onPage, SERIES_A_PAGE } from "../narrowing";

/** The letters a name can start with: `#` is for every name that starts with none of the others. */
const LETTERS = ["#", ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"];

/** What a letter is called when read out. */
function said(letter: string): string {
  return letter === "#" ? "Numbers and signs" : letter;
}

const letterClass =
  "grid h-9 min-w-9 flex-1 place-items-center rounded-control font-semibold text-metadata " +
  "[@media(hover:none)]:h-11";

/**
 * The letters the names of a library's series start with, for a library in
 * the order of its names. A letter leads to the page its first series is
 * on, and the page then goes down to that series. A letter no series starts
 * with, under what is asked, is greyed.
 *
 * `letters` are the ones some series starts with.
 */
export function LetterBar({
  libraryId,
  asked,
  letters,
  onChosen,
}: {
  libraryId: number;
  asked: Asked;
  letters: Letter[];
  /** A letter was clicked: also the one the address already asks for, which changes nothing in it. */
  onChosen: (letter: string) => void;
}) {
  const present = new Map(letters.map((letter) => [letter.letter, letter]));
  return (
    <nav
      aria-label="Series by first letter"
      // In a narrow window the letters go on sideways, out to its edges.
      className="-mx-gutter mb-6 flex gap-0.5 overflow-x-auto px-gutter [scrollbar-width:none]"
    >
      {LETTERS.map((letter) => {
        const found = present.get(letter);
        if (!found) {
          return (
            <span key={letter} aria-hidden="true" className={`${letterClass} text-fg-muted/40`}>
              {letter}
            </span>
          );
        }
        const page = Math.floor(found.offset / SERIES_A_PAGE) + 1;
        return (
          <Link
            key={letter}
            to="/libraries/$libraryId"
            params={{ libraryId }}
            search={onPage(asked, page)}
            hash={letter === "#" ? "0" : letter}
            replace
            // The page goes to the letter's first series itself.
            resetScroll={false}
            // Marked only while the address asks for this very letter: every
            // letter of the page shown leads to that same page.
            activeOptions={{ exact: true, includeHash: true }}
            onClick={() => onChosen(letter)}
            aria-label={`${said(letter)}, ${count(found.count, "series", "series")}`}
            className={`${letterClass} text-fg hover:bg-raised aria-[current]:bg-fg aria-[current]:text-background`}
          >
            {letter}
          </Link>
        );
      })}
    </nav>
  );
}

/** The letter an address asks for after its `#`, if it is one. `0` stands for `#`. */
export function letterIn(hash: string): string | undefined {
  const letter = hash === "0" ? "#" : hash.toUpperCase();
  return LETTERS.includes(letter) ? letter : undefined;
}
