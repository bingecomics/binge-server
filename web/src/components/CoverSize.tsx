import { LayoutGrid } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useKeepSetting, useSettings } from "../api/settings";
import { COVER_SIZE, chooseCoverSize, coverSize } from "../coverSize";
import { useSettled } from "../settled";

/** The key the server keeps the size of the covers under, for each person. */
const KEPT_AS = "cover-size";

/** How long after the slider stops the server is told, in milliseconds. */
const AFTER_SLIDING = 600;

/**
 * The slider that sizes the covers of every grid and row, in the bar at
 * the top. A window narrower than a laptop's has no room for it, and its
 * covers do not follow it: see `with-cover-slider` in `styles.css`.
 *
 * The size is the person's, kept by the server, so it is the same in
 * another browser. This browser keeps it too (`coverSize.ts`), to have it
 * before the server has answered.
 */
export function CoverSize() {
  const kept = useSettings().data?.[KEPT_AS];
  const keep = useKeepSetting();
  const [size, setSize] = useState(coverSize);
  // Whether the slider was moved here: only that is worth telling the server.
  const moved = useRef(false);

  // What was chosen in another browser arrives with the settings.
  useEffect(() => {
    const theirs = Number(kept);
    const sensible = kept !== undefined && theirs >= COVER_SIZE.min && theirs <= COVER_SIZE.max;
    if (sensible && theirs !== coverSize()) {
      chooseCoverSize(theirs);
      setSize(theirs);
    }
  }, [kept]);

  const settled = useSettled(size, AFTER_SLIDING);
  // biome-ignore lint/correctness/useExhaustiveDependencies: told once per size the slider stops at
  useEffect(() => {
    if (moved.current && String(settled) !== kept) {
      keep.mutate({ key: KEPT_AS, value: String(settled) });
    }
  }, [settled]);

  return (
    <label
      className="hidden h-11 items-center gap-2 px-2 with-cover-slider:flex"
      title="Size of the covers"
    >
      <LayoutGrid className="size-4 text-fg-muted" aria-hidden="true" />
      <input
        type="range"
        min={COVER_SIZE.min}
        max={COVER_SIZE.max}
        step={COVER_SIZE.step}
        value={size}
        onChange={(event) => {
          moved.current = true;
          // The covers follow through the styles, at once.
          chooseCoverSize(event.target.valueAsNumber);
          setSize(event.target.valueAsNumber);
        }}
        aria-label="Size of the covers"
        className="w-24 accent-accent"
      />
    </label>
  );
}
