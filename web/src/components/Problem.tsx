import { X } from "lucide-react";
import { dismissProblem, useProblem } from "../problems";
import { iconButtonClass } from "./NavLink";

/**
 * What there is to say about the last thing done from a menu or a bar, over
 * the page for a while: what went wrong with it, or that it was done.
 */
export function Problem() {
  const said = useProblem();
  if (!said) {
    return null;
  }
  return (
    <div
      role={said.wrong ? "alert" : "status"}
      className={`fixed inset-x-4 top-16 z-40 mx-auto flex w-fit max-w-full items-center gap-2 rounded-card border bg-surface py-1 pr-1 pl-4 text-metadata shadow-xl ${
        said.wrong ? "border-danger text-danger" : "border-divider text-fg"
      }`}
    >
      {said.sentence}
      <button
        type="button"
        onClick={dismissProblem}
        aria-label="Dismiss"
        className={`${iconButtonClass} size-9`}
      >
        <X className="size-4" aria-hidden="true" />
      </button>
    </div>
  );
}
