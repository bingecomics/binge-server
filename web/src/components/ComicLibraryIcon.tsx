import { useId } from "react";

/** One issue: an upright cover, before it is turned about the middle of the icon. */
const ISSUE = { x: 5.5, y: 4.5, width: 13, height: 16, rx: 1.6 };
const LOWER = "rotate(9 12 12.5)";
const UPPER = "rotate(-8 12 12.5)";

/**
 * The readers' icon for a library of comics: a loose pile of two issues, the
 * top one with a title bar. On the 24-unit grid of the icons beside it and in
 * the colour of the text around it. Its lines are the readers' weight, thinner
 * than those icons' because the two issues' lines run together.
 */
export function ComicLibraryIcon({ className }: { className?: string }) {
  const cut = useId();
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {/* The lower issue shows only where the upper one does not cover it. A
          cut, not a fill in the background's colour, so it is right on any
          background. */}
      <mask id={cut} maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24">
        <rect width="24" height="24" fill="white" stroke="none" />
        <rect {...ISSUE} transform={UPPER} fill="black" stroke="none" />
      </mask>
      <g mask={`url(#${cut})`}>
        <rect {...ISSUE} transform={LOWER} />
      </g>
      <g transform={UPPER}>
        <rect {...ISSUE} />
        <path d="M8.5 8.3h7" />
      </g>
    </svg>
  );
}
