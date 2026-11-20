import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ChevronLeft, CircleAlert, CircleCheck, FileUp, Undo2 } from "lucide-react";
import { memo, type ReactNode, useMemo, useState } from "react";
import type { Book, CblEntry, CblLine, CblPlan } from "../api/client";
import { useSearch } from "../api/libraries";
import {
  BIGGEST_CBL,
  type RuleFor,
  useCreateList,
  useForgetRule,
  useKeepRule,
  usePlanImport,
} from "../api/lists";
import { askForFile } from "../askForFile";
import { Button } from "../components/Button";
import { Dialog, DialogActions } from "../components/Dialog";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { placeClass } from "../components/ListBooks";
import { ChoiceRow } from "../components/Menu";
import { navHeadingClass } from "../components/NavLink";
import { rowClass } from "../components/Section";
import { ErrorText, upLinkClass } from "../components/text";
import { books as bookCount, called, count, titleUnder, yearOf } from "../format";
import { AFTER_TYPING, useSettled } from "../settled";

export const Route = createFileRoute("/lists/import")({ component: ImportPage });

/** How the series of an entry was found, to end "found …". */
const HOW: Record<NonNullable<CblLine["how"]>, string> = {
  rule: "by a rule of yours",
  name: "by its name",
  name_and_volume: "by its name and volume",
  without_article: "by its name without its article",
  without_words: "by its name without words such as TPB",
};

/** An entry as a reader would say it: "Night Runner #7 (1987)". */
function entryName(entry: CblEntry): string {
  const number = entry.number ? ` #${entry.number}` : "";
  const when = entry.volume ?? entry.year;
  return `${entry.series}${number}${when ? ` (${when})` : ""}`;
}

/** What the server found for an entry, in a sentence. */
function verdict(line: CblLine): string {
  const series = line.series_name ? `“${line.series_name}”` : "The series";
  switch (line.found) {
    case "book":
      return line.book
        ? `${called(line.book)}, found ${line.how ? HOW[line.how] : "by hand"}`
        : "Found";
    case "no_series":
      return "No series is called this.";
    case "no_issue": {
      const what = line.entry.number ? `#${line.entry.number}` : "book for it";
      return `${series} has no ${what}.`;
    }
    case "several":
      return `${count(line.candidates.length, "book", "books")} of ${series} fit.`;
  }
}

/** Some choices under a heading. Nothing at all when there are none. */
function Choices({ title, children }: { title: string; children: ReactNode[] }) {
  if (children.length === 0) {
    return null;
  }
  return (
    <>
      <h3 className={`mt-4 ${navHeadingClass}`}>{title}</h3>
      <ul className="-mx-3 mt-1">{children}</ul>
    </>
  );
}

function BookChoices({
  title,
  books,
  disabled,
  onChoose,
}: {
  title: string;
  books: Book[];
  disabled: boolean;
  onChoose: (book: Book) => void;
}) {
  return (
    <Choices title={title}>
      {books.map((book) => (
        <ChoiceRow
          key={book.id}
          name={called(book)}
          under={titleUnder(book) ?? yearOf(book)}
          disabled={disabled}
          onChoose={() => onChoose(book)}
        />
      ))}
    </Choices>
  );
}

/**
 * Says by hand what an entry is: its series, where the server then looks
 * for the number, or the book itself. What is said is kept as a rule, for
 * this import and the ones after. `onChanged` runs once a rule was kept or
 * forgotten.
 */
function Fix({
  line,
  onChanged,
  onClose,
}: {
  line: CblLine;
  onChanged: () => void;
  onClose: () => void;
}) {
  const keep = useKeepRule();
  const forget = useForgetRule();
  const [typed, setTyped] = useState(line.entry.series);
  const found = useSearch(useSettled(typed.trim(), AFTER_TYPING));
  const busy = keep.isPending || forget.isPending;
  const problem = keep.error ?? forget.error ?? found.error;
  const number = line.entry.number ? `#${line.entry.number}` : "the book";
  const ruleId = line.rule_id;

  function say(rule: RuleFor) {
    keep.mutate(rule, { onSuccess: onChanged });
  }

  return (
    <>
      <p className="mt-2 text-fg-muted text-metadata">
        Choose its series, and the server looks for {number} in it, at this import and the ones
        after. Or choose the book itself.
      </p>
      {ruleId != null ? (
        <Button
          className="mt-4"
          disabled={busy}
          onClick={() => forget.mutate(ruleId, { onSuccess: onChanged })}
        >
          <Undo2 className="size-4" aria-hidden="true" />
          Forget the rule that found it
        </Button>
      ) : null}
      <BookChoices
        title="Books that fit"
        books={line.candidates}
        disabled={busy}
        onChoose={(book) => say({ entry: line.entry, book_id: book.id })}
      />
      <Field
        className="mt-4"
        label="Look for the series or the book"
        type="search"
        value={typed}
        onChange={(event) => setTyped(event.target.value)}
      />
      <div className="max-h-72 overflow-y-auto">
        <Choices title="Series">
          {(found.data?.series ?? []).map((series) => (
            <ChoiceRow
              key={series.id}
              name={series.name}
              under={bookCount(series.book_count)}
              disabled={busy}
              onChoose={() => say({ entry: line.entry, series_id: series.id })}
            />
          ))}
        </Choices>
        <BookChoices
          title="Books"
          books={found.data?.books ?? []}
          disabled={busy}
          onChoose={(book) => say({ entry: line.entry, book_id: book.id })}
        />
        {/* An answer still shown from the words before says nothing about these. */}
        {found.data &&
        !found.isPlaceholderData &&
        found.data.series.length + found.data.books.length === 0 ? (
          <p className="mt-4 text-fg-muted text-metadata">Nothing found for “{typed.trim()}”.</p>
        ) : null}
      </div>
      {problem ? <ErrorText className="mt-4">{problem.message}</ErrorText> : null}
      <DialogActions onCancel={onClose}>{null}</DialogActions>
    </>
  );
}

/** One entry of the file: what it asks for, what was found, and the way to say otherwise. */
const Line = memo(function Line({
  line,
  place,
  onFix,
}: {
  line: CblLine;
  /** Its place in the file, from 0, which `onFix` takes. */
  place: number;
  onFix: (place: number) => void;
}) {
  const found = line.found === "book";
  const Icon = found ? CircleCheck : CircleAlert;
  return (
    <li className={`${rowClass} flex items-center gap-3`}>
      <span className={placeClass}>{(place + 1).toLocaleString()}</span>
      <Icon
        className={`size-5 shrink-0 ${found ? "text-success" : "text-danger"}`}
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1">
        <p className="truncate font-semibold text-metadata" title={entryName(line.entry)}>
          {entryName(line.entry)}
        </p>
        <p className={`text-caption ${found ? "text-fg-muted" : "text-danger"}`}>
          {found ? null : <span className="sr-only">Not found: </span>}
          {verdict(line)}
        </p>
      </div>
      <Button variant={found ? "quiet" : "secondary"} onClick={() => onFix(place)}>
        {found ? "Change" : "Fix"}
      </Button>
    </li>
  );
});

/**
 * What a CBL file would make, before anything is made: each entry with the
 * book found for it and how. The reader fixes the ones that found none, or
 * the wrong one, names the list and makes it.
 */
function Plan({ plan, file, onChanged }: { plan: CblPlan; file: File; onChanged: () => void }) {
  const navigate = useNavigate();
  const create = useCreateList();
  // What the file calls the list, or else what the file is called.
  const [name, setName] = useState(plan.name ?? file.name.replace(/\.[^.]+$/, ""));
  /** The entry being fixed, as its place from 0. */
  const [fixing, setFixing] = useState<number | null>(null);
  const fixed = fixing === null ? undefined : plan.lines[fixing];
  const books = useMemo(
    () => plan.lines.flatMap((line) => (line.found === "book" && line.book ? [line.book.id] : [])),
    [plan],
  );
  const missing = plan.lines.length - books.length;

  return (
    <>
      <Form
        className="mt-8 max-w-2xl"
        problem={create.error?.message}
        onSubmit={() =>
          create.mutate(
            { name, books },
            {
              onSuccess: (list) =>
                navigate({ to: "/lists/$listId", params: { listId: list.id }, replace: true }),
            },
          )
        }
        actions={
          <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-2">
            <Button
              type="submit"
              variant="primary"
              disabled={create.isPending || books.length === 0 || name.trim() === ""}
            >
              Make the list with {bookCount(books.length)}
            </Button>
            {missing > 0 ? (
              <p className="text-fg-muted text-metadata">
                {count(missing, "entry has", "entries have")} no book and{" "}
                {missing === 1 ? "is" : "are"} left out.
              </p>
            ) : null}
          </div>
        }
      >
        <Field
          label="Name of the list"
          value={name}
          onChange={(event) => setName(event.target.value)}
          maxLength={200}
          hint={
            plan.name_taken_by != null && name === plan.name
              ? "There is already a reading list with this name. Give this one another."
              : undefined
          }
        />
        <h2 className="mt-8 font-semibold text-section-title">
          {books.length.toLocaleString()} of {count(plan.lines.length, "entry", "entries")} found
        </h2>
        <ol className="mt-2">
          {plan.lines.map((line, place) => (
            <Line
              // The entries of a file have no name of their own but their place.
              // biome-ignore lint/suspicious/noArrayIndexKey: as above
              key={place}
              line={line}
              place={place}
              onFix={setFixing}
            />
          ))}
        </ol>
      </Form>

      <Dialog
        open={fixed !== undefined}
        onClose={() => setFixing(null)}
        title={fixed ? `What is ${entryName(fixed.entry)}?` : ""}
      >
        {fixed ? (
          <Fix
            line={fixed}
            onClose={() => setFixing(null)}
            onChanged={() => {
              setFixing(null);
              onChanged();
            }}
          />
        ) : null}
      </Dialog>
    </>
  );
}

/**
 * Imports a reading list from a CBL file, as a dry run first: the file is
 * read and matched, and nothing is made until the reader says so.
 */
function ImportPage() {
  const read = usePlanImport();
  /** The file as it was when it was chosen, so that it can be read again after a fix. */
  const [file, setFile] = useState<File | null>(null);
  const [plan, setPlan] = useState<CblPlan | null>(null);

  function choose() {
    askForFile(".cbl,.xml,application/xml,text/xml", async (chosen) => {
      // A copy: the file may change or go while its entries are being fixed.
      // One too big to send is not read: it is refused as it is.
      const kept =
        chosen.size > BIGGEST_CBL * 1024 * 1024
          ? chosen
          : new File([await chosen.arrayBuffer()], chosen.name);
      setFile(kept);
      setPlan(null);
      read.mutate(kept, { onSuccess: setPlan });
    });
  }

  const up = (
    <Link to="/lists" className={upLinkClass}>
      <ChevronLeft className="size-4" aria-hidden="true" />
      Reading lists
    </Link>
  );
  return (
    <>
      {up}
      <h1 className="mt-2 font-bold text-screen-title tracking-tight">Import a CBL file</h1>
      <p className="mt-2 max-w-prose text-fg-muted">
        A CBL file is a reading list someone wrote down: entries that each name a series, a number
        and a year. The server looks for every entry among your books and shows what it found.
        Nothing is made until you say so.
      </p>
      <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-2">
        <Button variant={plan ? "secondary" : "primary"} disabled={read.isPending} onClick={choose}>
          <FileUp className="size-5" aria-hidden="true" />
          {file ? "Choose another file" : "Choose a file"}
        </Button>
        {file ? <p className="min-w-0 break-all text-fg-muted text-metadata">{file.name}</p> : null}
      </div>
      {read.isError ? <ErrorText className="mt-4">{read.error.message}</ErrorText> : null}
      {plan && file ? (
        <Plan
          // A new plan for each file: the name typed for one is not the other's.
          key={`${file.name}:${file.lastModified}`}
          plan={plan}
          file={file}
          onChanged={() => read.mutate(file, { onSuccess: setPlan })}
        />
      ) : null}
    </>
  );
}
