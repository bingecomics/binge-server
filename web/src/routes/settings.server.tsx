import { createFileRoute } from "@tanstack/react-router";
import { useIsOwner } from "../api/account";
import { useDatabaseCopies, useMakeCopy, useScheduleCopies } from "../api/backups";
import type { UpdateStatus } from "../api/client";
import { useHealth } from "../api/health";
import { useServerUpdate } from "../api/updates";
import { Button } from "../components/Button";
import { Tick } from "../components/Field";
import { ListRow, Row, Section } from "../components/Section";
import { ErrorText, PathText, textLinkClass } from "../components/text";
import { build, moment, size } from "../format";

export const Route = createFileRoute("/settings/server")({ component: ServerPage });

/**
 * The copies of the database the server keeps in its data directory: one a
 * day while that is on, the oldest making room, and one more whenever the
 * owner asks. The owner's to see.
 */
function DatabaseCopies() {
  const copies = useDatabaseCopies();
  const make = useMakeCopy();
  const schedule = useScheduleCopies();
  const kept = copies.data;
  const problem = copies.error ?? make.error ?? schedule.error;
  return (
    <Section
      title="Copies of the database"
      action={
        <Button disabled={make.isPending || !kept} onClick={() => make.mutate()}>
          {make.isPending ? "Copying…" : "Make a copy now"}
        </Button>
      }
    >
      <p className="text-fg-muted text-metadata">
        A copy holds everything the server keeps but the covers: the libraries as they are laid out,
        what everyone read, the reading lists. They guard against a mistake, not against a disk that
        fails: they are on the same disk as the database.
      </p>
      {problem ? <ErrorText className="mt-3">{problem.message}</ErrorText> : null}
      {kept ? (
        <>
          <Tick
            className="mt-4"
            name="Make a copy every day"
            under={`The latest ${kept.kept} are kept. An older one is removed when a new one is made.`}
            checked={kept.scheduled}
            disabled={schedule.isPending}
            onChange={(scheduled) => schedule.mutate(scheduled)}
          />
          <PathText path={kept.folder} className="mt-4" />
          {kept.copies.length === 0 ? (
            <p className="mt-2 text-fg-muted">None yet.</p>
          ) : (
            <ul className="mt-2">
              {kept.copies.map((copy) => (
                <ListRow key={copy.name} title={moment(copy.made_at)} detail={size(copy.size)}>
                  <PathText path={copy.name} />
                </ListRow>
              ))}
            </ul>
          )}
        </>
      ) : null}
    </Section>
  );
}

/**
 * The server's own updates: whether a newer version is out, and the button that brings the
 * server to it. The server fetches the version, makes sure it is Binge's, and starts again as
 * it. The owner's to see.
 */
/** What the last look for a new version found, when it found none. */
function looked(status: UpdateStatus) {
  if (status.looked_at == null) {
    return "The server has not looked for a new version since it started.";
  }
  const when = `Looked ${moment(status.looked_at)}.`;
  return status.problem ? when : `This is the newest version. ${when}`;
}

function Updates() {
  const update = useServerUpdate();
  const { status, updating } = update;
  const newer = status?.newer;
  const cannot = status?.cannot;
  const busy = updating != null || update.look.isPending || update.setLooks.isPending;
  const problem = update.error?.message ?? (updating ? null : status?.problem);

  if (cannot?.why === "turned_off") {
    return (
      <Section title="Updates">
        <p className="text-fg-muted text-metadata">{cannot.message}</p>
      </Section>
    );
  }
  return (
    <Section
      title="Updates"
      action={
        <Button disabled={busy || !status} onClick={() => update.look.mutate()}>
          {update.look.isPending ? "Looking…" : "Look for a new version"}
        </Button>
      }
    >
      {!status ? null : updating ? (
        <p role="status">
          {updating === "fetching"
            ? "Fetching the new version and making sure it is Binge's…"
            : "Starting again as the new version. This page loads again when it runs."}
        </p>
      ) : newer ? (
        <>
          <p>
            <span className="font-semibold">Version {newer.version} is out.</span> This server runs{" "}
            {status.version}.{" "}
            <a className={textLinkClass} href={newer.notes} target="_blank" rel="noreferrer">
              What changed
            </a>
          </p>
          {cannot ? null : (
            <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2">
              <Button
                variant="primary"
                disabled={busy}
                onClick={() => update.start.mutate(newer.version)}
              >
                Update to {newer.version}
              </Button>
              <p className="text-caption text-fg-muted">
                The server stops for a moment while it starts again.
              </p>
            </div>
          )}
        </>
      ) : (
        <p className="text-fg-muted">{looked(status)}</p>
      )}
      {status && cannot ? (
        <p className="mt-3 text-fg-muted text-metadata">{cannot.message}</p>
      ) : null}
      {status?.not_started && !updating ? (
        <ErrorText className="mt-3">{status.not_started}</ErrorText>
      ) : null}
      {problem ? <ErrorText className="mt-3">{problem}</ErrorText> : null}
      {status ? (
        <Tick
          className="mt-4"
          name="Look for a new version every day"
          under="Looking asks GitHub, where the versions are published. Nothing about your library or about you is sent."
          checked={status.looks}
          disabled={busy}
          onChange={(looks) => update.setLooks.mutate(looks)}
        />
      ) : null}
    </Section>
  );
}

/** What the server says about itself, and for the owner its updates and the copies of its database. */
function ServerPage() {
  const health = useHealth();
  const isOwner = useIsOwner();
  const server = health.data;
  return (
    <>
      <Section title="About this server">
        {health.isError ? (
          <ErrorText>The server is not answering. Check that it is still running.</ErrorText>
        ) : (
          <dl>
            <Row label="Name">{server?.name ?? "…"}</Row>
            <Row label="Version">{server?.version ?? "…"}</Row>
            {server && build(server) ? <Row label="Build">{build(server)}</Row> : null}
            <Row label="Database">
              {server ? (server.database === "ok" ? "Working" : "Unavailable") : "…"}
            </Row>
            <Row label="API description">
              <a className={textLinkClass} href="/api/v1/openapi.json">
                openapi.json
              </a>
            </Row>
          </dl>
        )}
      </Section>
      {isOwner ? <Updates /> : null}
      {isOwner ? <DatabaseCopies /> : null}
    </>
  );
}
