import { Tick } from "../components/Field";
import { textLinkClass } from "../components/text";
import { FITS, type Fit, useReaderSettings } from "./settings";

type Option<T> = { value: T; label: string };

const PAGES: Option<"one" | "two">[] = [
  { value: "one", label: "One" },
  { value: "two", label: "Two" },
];

const FIT_LABELS: Record<Fit, string> = {
  page: "Page",
  width: "Width",
  height: "Height",
  original: "Original",
};
const FIT_OPTIONS: Option<Fit>[] = FITS.map((value) => ({ value, label: FIT_LABELS[value] }));

const DIRECTIONS: Option<"ltr" | "rtl">[] = [
  { value: "ltr", label: "Left to right" },
  { value: "rtl", label: "Right to left" },
];

/** One setting: a row of choices of which exactly one is picked. */
function Choice<T extends string>({
  legend,
  name,
  options,
  value,
  onChange,
}: {
  legend: string;
  name: string;
  options: Option<T>[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <fieldset>
      <legend className="text-caption text-fg-muted">{legend}</legend>
      <div className="mt-1 grid auto-cols-fr grid-flow-col gap-1 rounded-control bg-raised p-1">
        {options.map((option) => (
          <label
            key={option.value}
            className="grid min-h-9 cursor-pointer place-items-center rounded-control px-2 text-center text-caption text-fg-muted has-checked:bg-surface has-checked:font-semibold has-checked:text-fg has-focus-visible:outline-2 has-focus-visible:outline-accent"
          >
            <input
              type="radio"
              name={name}
              value={option.value}
              checked={option.value === value}
              onChange={() => onChange(option.value)}
              className="sr-only"
            />
            {option.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/**
 * The reader's settings, shown under its top bar: how the pages are shown,
 * and whether this reading is recorded.
 *
 * In a book of a series (`seriesId`, and `series` is its name) a change is
 * kept for that series alone. Once the series keeps settings of its own,
 * they can be made the usual ones or let go for the usual.
 */
export function SettingsPanel({
  seriesId,
  series,
  wide,
  recorded,
  onRecord,
}: {
  seriesId: number | null | undefined;
  series: string | null | undefined;
  /** Whether the window has room for two pages. */
  wide: boolean;
  recorded: boolean;
  onRecord: (recorded: boolean) => void;
}) {
  const { settings, change: onChange, own, makeUsual, backToUsual } = useReaderSettings(seriesId);
  return (
    <section
      aria-label="Reader settings"
      className="absolute top-full right-2 mt-2 flex max-h-[calc(100dvh-4.5rem)] w-[min(20rem,calc(100vw-1rem))] flex-col gap-4 overflow-y-auto rounded-card border border-divider bg-surface p-4 shadow-xl"
    >
      <Choice
        legend="Pages on screen"
        name="pages"
        options={PAGES}
        value={settings.twoPages ? "two" : "one"}
        onChange={(pages) => onChange({ twoPages: pages === "two" })}
      />
      {settings.twoPages && !wide ? (
        <p className="-mt-2 text-caption text-fg-muted">
          Two pages are shown when the window is wider than it is tall.
        </p>
      ) : null}
      <Choice
        legend="Fit"
        name="fit"
        options={FIT_OPTIONS}
        value={settings.fit}
        onChange={(fit) => onChange({ fit })}
      />
      <Choice
        legend="Reading direction"
        name="direction"
        options={DIRECTIONS}
        value={settings.rightToLeft ? "rtl" : "ltr"}
        onChange={(direction) => onChange({ rightToLeft: direction === "rtl" })}
      />
      {series ? (
        <p className="-mt-1 text-caption text-fg-muted">
          {own ? (
            <>
              Kept for {series}.{" "}
              <button type="button" onClick={makeUsual} className={textLinkClass}>
                Use for everything
              </button>{" "}
              or{" "}
              <button type="button" onClick={backToUsual} className={textLinkClass}>
                go back to your usual
              </button>
              .
            </>
          ) : (
            `What you change here is kept for ${series}.`
          )}
        </p>
      ) : null}
      <Tick
        name="Record this reading"
        under="Off, turning pages does not move your place in the book and is left out of what you read."
        checked={recorded}
        onChange={onRecord}
        className="border-divider border-t pt-4"
      />
    </section>
  );
}
