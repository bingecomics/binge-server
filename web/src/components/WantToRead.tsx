import { Star, StarOff } from "lucide-react";
import { type BooksToMark, onlySeries } from "../api/reading";
import { useWant, useWantNoMore } from "../api/wanted";
import { MenuItem } from "./Menu";

/** What stands for want to read wherever it is named. */
export const WantToReadIcon = Star;

/**
 * The choice of a menu that puts what the menu is about on the reader's
 * want to read, or takes it off when the menu is about one series that is
 * on it. Want to read holds whole series, so a menu about anything else has
 * no such choice.
 */
export function WantToReadChoice({ what, wanted }: { what: BooksToMark; wanted?: boolean }) {
  const want = useWant();
  const noMore = useWantNoMore();
  const series = onlySeries(what);
  if (!series) {
    return null;
  }
  const [one] = series;
  return wanted && series.length === 1 && one !== undefined ? (
    <MenuItem icon={StarOff} onSelect={() => noMore.mutate(one)}>
      Remove from want to read
    </MenuItem>
  ) : (
    <MenuItem icon={WantToReadIcon} onSelect={() => want.mutate(series)}>
      Add to want to read
    </MenuItem>
  );
}
