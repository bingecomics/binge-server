import { useId } from "react";

const PLAY = "M 320 270 L 760 512 L 320 754 Z";

/**
 * The Binge mark, drawn as the readers draw it: a charcoal tile, a comic
 * panel's border in paper, and a play triangle filled with amber halftone
 * dots. Its colours are its own and do not follow the theme.
 */
export function BingeMark({ className }: { className?: string }) {
  // One page can hold the mark twice, and each copy needs its own names.
  const id = useId();
  const play = `${id}-play`;
  const dots = `${id}-dots`;
  return (
    <svg viewBox="0 0 1024 1024" className={className} aria-hidden="true">
      <defs>
        <clipPath id={play}>
          <path d={PLAY} />
        </clipPath>
        <pattern id={dots} width="60" height="60" patternUnits="userSpaceOnUse">
          <circle cx="30" cy="30" r="19" className="fill-brand-amber" />
        </pattern>
      </defs>
      <rect width="1024" height="1024" rx="230" className="fill-brand-charcoal" />
      <rect
        x="88"
        y="88"
        width="848"
        height="848"
        rx="150"
        fill="none"
        strokeWidth="44"
        className="stroke-brand-paper"
      />
      <rect width="1024" height="1024" fill={`url(#${dots})`} clipPath={`url(#${play})`} />
      <path
        d={PLAY}
        fill="none"
        strokeWidth="36"
        strokeLinejoin="round"
        className="stroke-brand-paper"
      />
    </svg>
  );
}
