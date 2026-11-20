/**
 * A thin bar filled as far as `done`, from 0 to 1. Its height, its place and
 * the roundness of its ends are `className`'s to say. It is not read out:
 * say the same in words beside it.
 */
export function ProgressBar({ done, className = "" }: { done: number; className?: string }) {
  return (
    <div className={`overflow-hidden bg-raised ${className}`} aria-hidden="true">
      <div className="h-full rounded-[inherit] bg-accent" style={{ width: `${done * 100}%` }} />
    </div>
  );
}
