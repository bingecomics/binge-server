import { createFileRoute } from "@tanstack/react-router";
import { useIsOwner } from "../api/account";
import { useDatabaseCopies, useMakeCopy, useScheduleCopies } from "../api/backups";
import { useHealth } from "../api/health";
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

/** What the server says about itself, and for the owner the copies of its database. */
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
      {isOwner ? <DatabaseCopies /> : null}
    </>
  );
}
