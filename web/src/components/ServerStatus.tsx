import { Link } from "@tanstack/react-router";
import { useIsOwner } from "../api/account";
import { useHealth } from "../api/health";
import { useUpdateStatus } from "../api/updates";
import { moment } from "../format";
import { textLinkClass } from "./text";

/**
 * The connection to the server in a line, with its version, and when it was built under it. The
 * owner is told here when a newer version is out.
 */
export function ServerStatus() {
  const health = useHealth();
  const newer = useUpdateStatus(useIsOwner()).data?.newer;

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
      {newer ? (
        <p className="pl-4">
          <Link to="/settings/server" className={textLinkClass}>
            Version {newer.version} is out
          </Link>
        </p>
      ) : null}
    </div>
  );
}
