import { useHealth } from "../api/health";
import { moment } from "../format";

/** The connection to the server in a line, with its version, and when it was built under it. */
export function ServerStatus() {
  const health = useHealth();

  let dot = "bg-outline";
  let label = "Connecting…";
  let built: number | null = null;
  if (health.isError) {
    dot = "bg-danger";
    label = "Server unreachable";
  } else if (health.data) {
    const healthy = health.data.database === "ok";
    dot = healthy ? "bg-success" : "bg-danger";
    label = healthy ? `Connected · v${health.data.version}` : "Database unavailable";
    built = health.data.built_at ?? null;
  }

  return (
    <div className="text-caption text-fg-muted">
      <p className="flex items-center gap-2" aria-live="polite">
        <span className={`size-2 shrink-0 rounded-full ${dot}`} aria-hidden="true" />
        {label}
      </p>
      {/* The version stays the same from one build to the next; this does not. */}
      {built == null ? null : <p className="pl-4">Built {moment(built)}</p>}
    </div>
  );
}
