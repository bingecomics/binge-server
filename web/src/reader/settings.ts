import { useCallback, useMemo } from "react";
import { useKeepSetting, useSettings } from "../api/settings";

/** The ways a page is sized to the window. */
export const FITS = ["page", "width", "height", "original"] as const;
export type Fit = (typeof FITS)[number];

/**
 * How the reader shows books. The person's own, kept by the server so that
 * it follows them to another browser: what they usually read with, and what
 * they changed while reading a series, which that series then keeps.
 */
export type ReaderSettings = {
  /** Two pages side by side when the window is wide enough. */
  twoPages: boolean;
  fit: Fit;
  /** Pages run from right to left, as in manga. */
  rightToLeft: boolean;
};

const DEFAULTS: ReaderSettings = { twoPages: false, fit: "page", rightToLeft: false };

/** The key the server keeps what the person usually reads with under. */
const USUAL = "reader";

/**
 * The key the server keeps the settings of each series under: one value
 * for all of them, by the series' id, each with when it was last changed.
 */
const BY_SERIES = "reader.series";

/**
 * How many series keep settings of their own. The ones changed longest ago
 * are let go, so the value stays well under what the server takes: 8 KB,
 * and a series takes about 80 bytes of it.
 */
const MOST_SERIES = 60;

/** Where the settings were kept before the server kept them. Only read. */
const IN_BROWSER = "binge.reader";

type OfSeries = ReaderSettings & {
  /** When they were last changed, in seconds since 1970. */
  at: number;
};

/** Settings from what was kept, whatever wrote it. Nothing for what is no settings at all. */
function read(kept: unknown): ReaderSettings | undefined {
  if (typeof kept !== "object" || kept === null) {
    return undefined;
  }
  const { twoPages, fit, rightToLeft } = kept as Record<string, unknown>;
  return {
    twoPages: typeof twoPages === "boolean" ? twoPages : DEFAULTS.twoPages,
    fit: FITS.find((known) => known === fit) ?? DEFAULTS.fit,
    rightToLeft: typeof rightToLeft === "boolean" ? rightToLeft : DEFAULTS.rightToLeft,
  };
}

/** What a text holds as JSON, or nothing when it is none. */
function parsed(text: string | null | undefined): unknown {
  try {
    return text ? JSON.parse(text) : undefined;
  } catch {
    return undefined;
  }
}

function inBrowser(): ReaderSettings | undefined {
  try {
    return read(parsed(localStorage.getItem(IN_BROWSER)));
  } catch {
    // Storage is blocked.
    return undefined;
  }
}

/** The settings each series keeps, by the series' id. */
function bySeries(text: string | undefined): Record<string, OfSeries> {
  const kept = parsed(text);
  if (typeof kept !== "object" || kept === null) {
    return {};
  }
  const series: Record<string, OfSeries> = {};
  for (const [id, one] of Object.entries(kept)) {
    const settings = read(one);
    if (settings) {
      const at = (one as { at?: unknown }).at;
      series[id] = { ...settings, at: typeof at === "number" ? at : 0 };
    }
  }
  return series;
}

/**
 * The settings a book is read with, and the ways to change them.
 *
 * A book of a series (`seriesId`) is read with what that series keeps, or
 * else with what the person usually reads with. A change made while
 * reading it is kept for the series alone. A book in no series is read
 * with the usual, and changes that.
 *
 * `own` says whether the series keeps settings of its own. `makeUsual`
 * makes what is on screen the usual, and `backToUsual` lets go of the
 * series' own.
 *
 * The person's settings must have arrived: see `useSettings`.
 */
export function useReaderSettings(seriesId: number | null | undefined) {
  const kept = useSettings().data;
  const { mutate: keep } = useKeepSetting();
  const usualText = kept?.[USUAL];
  const seriesText = kept?.[BY_SERIES];

  // What this browser kept before the server did stands in until the first change.
  const usual = useMemo(() => read(parsed(usualText)) ?? inBrowser() ?? DEFAULTS, [usualText]);
  const series = useMemo(() => bySeries(seriesText), [seriesText]);
  const own = seriesId != null ? series[seriesId] : undefined;
  const settings: ReaderSettings = own ?? usual;

  const keepSeries = useCallback(
    (id: number, changed: ReaderSettings | null) => {
      const { [id]: _before, ...others } = series;
      const all = changed
        ? { ...others, [id]: { ...changed, at: Math.floor(Date.now() / 1000) } }
        : others;
      const latest = Object.entries(all)
        .sort(([, one], [, other]) => other.at - one.at)
        .slice(0, MOST_SERIES);
      keep({ key: BY_SERIES, value: JSON.stringify(Object.fromEntries(latest)) });
    },
    [series, keep],
  );
  const keepUsual = useCallback(
    (changed: ReaderSettings) => keep({ key: USUAL, value: JSON.stringify(changed) }),
    [keep],
  );

  const change = useCallback(
    (changed: Partial<ReaderSettings>) => {
      const after = { ...settings, ...changed };
      if (seriesId != null) {
        keepSeries(seriesId, after);
      } else {
        keepUsual(after);
      }
    },
    [settings, seriesId, keepSeries, keepUsual],
  );
  const backToUsual = useCallback(() => {
    if (seriesId != null && own) {
      keepSeries(seriesId, null);
    }
  }, [seriesId, own, keepSeries]);
  const makeUsual = useCallback(() => {
    // Without when the series last changed them, which is the series' own.
    const { twoPages, fit, rightToLeft } = settings;
    keepUsual({ twoPages, fit, rightToLeft });
    backToUsual();
  }, [settings, keepUsual, backToUsual]);

  return { settings, change, own: own !== undefined, makeUsual, backToUsual };
}
