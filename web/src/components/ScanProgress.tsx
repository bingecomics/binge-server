import { TriangleAlert } from "lucide-react";
import type { ScanStatus } from "../api/client";
import { count } from "../format";

/** What a library's scan is doing right now, or what went wrong in the last one. */
export function ScanProgress({ scan }: { scan: ScanStatus }) {
  if (scan.phase === "idle") {
    return scan.problem ? (
      <p className="flex max-w-2xl items-start gap-2 text-danger text-metadata" role="alert">
        <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
        {scan.problem}
      </p>
    ) : null;
  }

  const finding = scan.phase === "finding";
  const label = finding
    ? `Looking for comics… ${count(scan.found, "file", "files")} so far`
    : `Making covers: ${scan.analyzed.toLocaleString()} of ${scan.to_analyze.toLocaleString()}`;
  // Unknown while still looking: nobody knows yet how many there are.
  const done = finding || scan.to_analyze === 0 ? undefined : scan.analyzed / scan.to_analyze;

  return (
    <div className="max-w-md">
      <p className="text-fg-muted text-metadata">{label}</p>
      <div
        role="progressbar"
        aria-label="Scan progress"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={done === undefined ? undefined : Math.round(done * 100)}
        className="mt-2 h-1.5 overflow-hidden rounded-full bg-raised"
      >
        <div
          className={`h-full rounded-full bg-accent transition-[width] duration-500 ${
            done === undefined ? "w-full animate-pulse motion-reduce:animate-none" : ""
          }`}
          style={done === undefined ? undefined : { width: `${done * 100}%` }}
        />
      </div>
    </div>
  );
}
