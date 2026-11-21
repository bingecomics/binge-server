import { createFileRoute, Navigate } from "@tanstack/react-router";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import {
  useBringing,
  useForgetBrought,
  useSayBroughtLibrary,
  useStartBringing,
} from "../api/bring";
import type { Brought, BroughtLibrary } from "../api/client";
import { useLibraries } from "../api/libraries";
import { Button } from "../components/Button";
import { Field, fieldClass } from "../components/Field";
import { Form } from "../components/Form";
import { ProgressBar } from "../components/ProgressBar";
import { ListRow, Row, rowClass, Section } from "../components/Section";
import { ErrorText, PathText } from "../components/text";
import { count, day, moment, seriesCount, timeSpent } from "../format";

export const Route = createFileRoute("/settings/bring")({ component: BringPage });

/** "1,204 files". */
function files(amount: number): string {
  return count(amount, "file", "files");
}

/**
 * One library of the other server: its folder there, the library here that is taken for it, which
 * the owner puts right, and how many of its files are books here.
 */
function TheirLibrary({ library }: { library: BroughtLibrary }) {
  const ours = useLibraries();
  const say = useSayBroughtLibrary();
  const found = library.by_path + library.by_file;
  const missing = library.books - found;
  return (
    <li className={rowClass}>
      <p className="font-semibold">{library.name}</p>
      {library.folders.map((folder) => (
        <PathText key={folder} path={folder} />
      ))}
      <label className="mt-3 block font-semibold text-metadata">
        The same library here
        <select
          className={fieldClass}
          value={library.library_id ?? ""}
          disabled={say.isPending || !ours.data}
          onChange={(event) =>
            say.mutate({
              theirId: library.their_id,
              libraryId: event.target.value === "" ? null : Number(event.target.value),
            })
          }
        >
          <option value="">None on this server</option>
          {ours.data?.map((here) => (
            <option key={here.id} value={here.id}>
              {here.name}
            </option>
          ))}
        </select>
      </label>
      {say.error ? <ErrorText className="mt-2">{say.error.message}</ErrorText> : null}
      {library.library_id == null ? (
        <p className="mt-2 text-caption text-fg-muted">
          {files(library.books)} there. Say which library here holds the same folder, and they are
          looked for in it.
        </p>
      ) : (
        <p className="mt-2 text-caption text-fg-muted">
          {found.toLocaleString()} of {files(library.books)} are books here
          {library.by_file > 0
            ? `: ${library.by_path.toLocaleString()} at the same place, ${library.by_file.toLocaleString()} found by name and size`
            : ""}
          .
        </p>
      )}
      {library.library_id != null && missing > 0 ? (
        <details className="mt-2 text-caption text-fg-muted">
          <summary className="cursor-pointer">
            {files(missing)} {missing === 1 ? "is" : "are"} not here
          </summary>
          {library.missing.map((path) => (
            <PathText key={path} path={path} className="mt-1" />
          ))}
          {missing > library.missing.length ? (
            <p className="mt-1">And {files(missing - library.missing.length)} more.</p>
          ) : null}
        </details>
      ) : null}
    </li>
  );
}

/** What was read from the other server, before anything is made of it. */
function Found({ brought }: { brought: Brought }) {
  const forget = useForgetBrought();
  const { reading } = brought;
  return (
    <>
      <Section
        title="What was found"
        action={
          <Button disabled={forget.isPending} onClick={() => forget.mutate()}>
            {forget.isPending ? "Forgetting…" : "Forget it"}
          </Button>
        }
      >
        <p className="text-fg-muted text-metadata">
          Read from Kavita {brought.version} as {brought.their_name}, {moment(brought.taken_at)}.
          This is what there is to bring. Nothing of it is on this server yet.
        </p>
        {forget.error ? <ErrorText className="mt-3">{forget.error.message}</ErrorText> : null}
      </Section>

      <Section title="Libraries">
        {brought.libraries.length === 0 ? (
          <p className="text-fg-muted">The key reads no library there.</p>
        ) : (
          <ul>
            {brought.libraries.map((library) => (
              <TheirLibrary key={library.their_id} library={library} />
            ))}
          </ul>
        )}
      </Section>

      <Section title={`What ${brought.their_name} read there`}>
        <dl>
          <Row label="Finished">{reading.finished.toLocaleString()}</Row>
          <Row label="Started and not finished">{reading.started.toLocaleString()}</Row>
          <Row label="Of those, books that are here">{reading.found.toLocaleString()}</Row>
          <Row label="Sittings that were timed">{reading.timed.toLocaleString()}</Row>
          {reading.timed > 0 ? (
            <Row label="Time spent in them">{timeSpent(reading.seconds)}</Row>
          ) : null}
          <Row label="Sittings with only their day">{reading.dated.toLocaleString()}</Row>
          {reading.first_at != null ? <Row label="First read">{day(reading.first_at)}</Row> : null}
          {reading.last_at != null ? <Row label="Last read">{day(reading.last_at)}</Row> : null}
        </dl>
      </Section>

      <Section title="And also">
        <dl>
          <Row label="Reading lists">{brought.lists.toLocaleString()}</Row>
          <Row label="Collections">{brought.collections.toLocaleString()}</Row>
          <Row label="Series on want to read">{brought.wanted.toLocaleString()}</Row>
          <Row label="Marked pages">{brought.pages.toLocaleString()}</Row>
        </dl>
      </Section>

      {brought.manages ? (
        <Section title="People">
          <ul>
            {brought.people.map((person) => (
              <ListRow
                key={person.name}
                title={person.name}
                detail={`${
                  person.manages
                    ? "Manages that server"
                    : person.libraries.length === 0
                      ? "Reads no library"
                      : `Reads ${person.libraries.join(", ")}`
                }${person.limited ? " · limited by age there" : ""}`}
              />
            ))}
          </ul>
        </Section>
      ) : null}
    </>
  );
}

/**
 * Bringing what the owner has on the Kavita server they are leaving. The server reads it with an
 * address and a key and shows what it found: which libraries are which, how many of the files are
 * books here, and what there is of reading, lists, collections and people. Nothing is changed on
 * the other server, and nothing is made of it here yet.
 */
function Bring() {
  const bringing = useBringing();
  const start = useStartBringing();
  // Until something is typed, the address is the one read last.
  const [typed, setAddress] = useState<string | null>(null);
  const [key, setKey] = useState("");
  const status = bringing.data;
  const address = typed ?? status?.brought?.address ?? "";
  const reading = status?.reading === true;

  return (
    <>
      <Section title="Bring from Kavita">
        <p className="text-fg-muted text-metadata">
          If you are coming from a Kavita server, this server can read it: its libraries and their
          files, what you read there and when, your reading lists, collections, want to read and
          bookmarks, and who uses it. It only reads. Nothing is changed on the other server, and
          your key is used once and not kept.
        </p>
        {reading ? (
          <div className="mt-4" role="status">
            <p>
              Reading the other server…{" "}
              {status.series_total > 0
                ? `${status.series_read.toLocaleString()} of ${seriesCount(status.series_total)}`
                : ""}
            </p>
            <ProgressBar
              className="mt-2 h-1 rounded-full"
              done={status.series_total > 0 ? status.series_read / status.series_total : 0}
            />
          </div>
        ) : (
          <Form
            className="mt-4"
            onSubmit={() => start.mutate({ address, key }, { onSuccess: () => setKey("") })}
            problem={start.error?.message ?? status?.problem ?? bringing.error?.message}
            actions={
              <Button
                type="submit"
                variant="primary"
                className="mt-4"
                disabled={start.isPending || !status || !address.trim() || !key.trim()}
              >
                {status?.brought ? "Read it again" : "Read the other server"}
              </Button>
            }
          >
            <Field
              label="Its address"
              hint="As you open it in a browser, such as http://192.168.1.20:5000."
              value={address}
              onChange={(event) => setAddress(event.target.value)}
              autoComplete="off"
              inputMode="url"
            />
            <Field
              className="mt-4"
              label="Your key there"
              hint="In Kavita: your account settings, under API key."
              type="password"
              value={key}
              onChange={(event) => setKey(event.target.value)}
              autoComplete="off"
            />
          </Form>
        )}
      </Section>
      {status?.brought && !reading ? <Found brought={status.brought} /> : null}
    </>
  );
}

/** Bringing from another server is the owner's. Anyone else is led to their own account. */
function BringPage() {
  return useIsOwner() ? <Bring /> : <Navigate to="/settings/account" replace />;
}
