import { BingeMark } from "./BingeMark";

/**
 * The readers' lockup, the mark beside "BINGE" in its own face, with "Server"
 * small after it. Everything is sized from the type size around it, in the
 * readers' proportions.
 */
export function Brand() {
  return (
    <span className="inline-flex items-center gap-[0.4em] align-middle">
      <BingeMark className="size-[1.2em] shrink-0" />
      <span className="font-wordmark uppercase leading-none tracking-[0.07em]">Binge</span>
      <span className="text-caption text-fg-muted">Server</span>
    </span>
  );
}
