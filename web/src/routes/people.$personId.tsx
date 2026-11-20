import { createFileRoute } from "@tanstack/react-router";
import { Merge, Tags } from "lucide-react";
import { useState } from "react";
import { useIsOwner } from "../api/account";
import { gotNoAnswer, type Person, type PersonDetails, reason } from "../api/client";
import {
  useAddPersonName,
  useBooksOfPerson,
  useMergePeople,
  usePeopleNamed,
  usePerson,
  useSeriesOfPerson,
  useShowPersonBy,
  useSplitPerson,
} from "../api/people";
import { BookCard } from "../components/BookCard";
import { Button } from "../components/Button";
import { Dialog, DialogActions } from "../components/Dialog";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { ChoiceRow, MenuItem, MoreMenu } from "../components/Menu";
import { NotOnServer, ServerNotAnswering } from "../components/Notice";
import { ListRow } from "../components/Section";
import { SeriesCard } from "../components/SeriesCard";
import { Shelf } from "../components/Shelf";
import { ErrorText } from "../components/text";
import { books as bookCount, workOf } from "../format";
import { AFTER_TYPING, useSettled } from "../settled";

export const Route = createFileRoute("/people/$personId")({
  params: {
    parse: ({ personId }) => ({ personId: Number(personId) }),
    stringify: ({ personId }) => ({ personId: String(personId) }),
  },
  component: PersonPage,
});

/** How many people a search for someone to merge with shows: one more is asked for, the person being one of them. */
const PEOPLE_FOUND = 8;

/**
 * Makes this person and another one, for the files that spell one person
 * two ways. The other is looked for by name, then asked about before
 * anything is done.
 */
function MergeDialog({ person, onClose }: { person: PersonDetails; onClose: () => void }) {
  const merge = useMergePeople(person.id);
  const [typed, setTyped] = useState("");
  const [other, setOther] = useState<Person | null>(null);
  const words = useSettled(typed.trim(), AFTER_TYPING);
  const found = usePeopleNamed(words, PEOPLE_FOUND + 1);
  // With the box emptied, the answer to what was in it is put away.
  const people = typed.trim()
    ? (found.data?.people ?? []).filter((one) => one.id !== person.id).slice(0, PEOPLE_FOUND)
    : [];

  if (other) {
    return (
      <>
        <p className="mt-2 text-fg-muted text-metadata">
          “{other.name}” and “{person.name}” become one person, shown as “{person.name}”, with the
          books of both. No file is changed. A name can be made a person of its own again, under
          Names.
        </p>
        {merge.error ? <ErrorText className="mt-4">{reason(merge.error)}</ErrorText> : null}
        <DialogActions onCancel={() => setOther(null)}>
          <Button
            variant="primary"
            disabled={merge.isPending}
            onClick={() => merge.mutate([other.id], { onSuccess: onClose })}
          >
            {merge.isPending ? "Merging…" : "Make them one"}
          </Button>
        </DialogActions>
      </>
    );
  }
  return (
    <>
      <p className="mt-2 text-fg-muted text-metadata">
        For when the files spell one person two ways. Look for the other spelling.
      </p>
      <Field
        className="mt-4"
        label="Look for someone"
        type="search"
        value={typed}
        onChange={(event) => setTyped(event.target.value)}
        autoFocus
      />
      <ul className="-mx-3 mt-2 max-h-72 overflow-y-auto">
        {people.map((one) => (
          <ChoiceRow
            key={one.id}
            name={one.name}
            under={workOf(one)}
            onChoose={() => setOther(one)}
          />
        ))}
      </ul>
      {/* An answer still shown from the words before says nothing about these. */}
      {words && found.data && !found.isPlaceholderData && people.length === 0 ? (
        <p className="mt-2 text-fg-muted text-metadata">Nobody else is found for “{words}”.</p>
      ) : null}
      {found.error ? <ErrorText className="mt-4">{reason(found.error)}</ErrorText> : null}
      <DialogActions onCancel={onClose}>{null}</DialogActions>
    </>
  );
}

/**
 * The names a person goes by: which one they are shown by, the way to make
 * one a person of its own again, and the way to add one the files will use.
 */
function NamesDialog({ person, onClose }: { person: PersonDetails; onClose: () => void }) {
  const showBy = useShowPersonBy(person.id);
  const split = useSplitPerson(person.id);
  const add = useAddPersonName(person.id);
  const [typed, setTyped] = useState("");
  const busy = showBy.isPending || split.isPending || add.isPending;
  const problem = showBy.error ?? split.error;

  return (
    <>
      <p className="mt-2 text-fg-muted text-metadata">
        Every way the files spell this person. Books that use any of them are theirs.
      </p>
      <ul className="mt-2">
        {person.names.map((name, place) => (
          <ListRow
            key={name.id}
            title={name.name}
            detail={place === 0 ? "The name they are shown by" : "Another name"}
          >
            {place === 0 ? null : (
              <>
                <Button disabled={busy} onClick={() => showBy.mutate(name.id)}>
                  Show by this
                </Button>
                <Button variant="quiet" disabled={busy} onClick={() => split.mutate(name.id)}>
                  Make its own person
                </Button>
              </>
            )}
          </ListRow>
        ))}
      </ul>
      {problem ? <ErrorText className="mt-2">{reason(problem)}</ErrorText> : null}
      <Form
        className="mt-4"
        onSubmit={() => add.mutate(typed, { onSuccess: () => setTyped("") })}
        problem={add.error ? reason(add.error) : null}
        actions={
          <DialogActions onCancel={onClose}>
            <Button type="submit" variant="primary" disabled={busy || !typed.trim()}>
              Add the name
            </Button>
          </DialogActions>
        }
      >
        <Field
          label="Another name"
          hint="A spelling the files will use, before any of them does."
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
        />
      </Form>
    </>
  );
}

/** The books that name the person, from the first that came out, with the way to the rest. */
function Books({ personId }: { personId: number }) {
  const books = useBooksOfPerson(personId);
  const shown = books.data?.pages.flatMap((part) => part.books) ?? [];
  const total = books.data?.pages[0]?.total ?? 0;
  if (books.isError) {
    return <ErrorText>{reason(books.error)}</ErrorText>;
  }
  if (shown.length === 0) {
    return null;
  }
  return (
    <Shelf
      title="Books"
      note={
        books.hasNextPage ? (
          <div className="mt-8 flex flex-wrap items-center gap-4">
            <Button disabled={books.isFetchingNextPage} onClick={() => books.fetchNextPage()}>
              {books.isFetchingNextPage ? "Loading…" : "Show more"}
            </Button>
            <p className="text-fg-muted text-metadata">
              {shown.length.toLocaleString()} of {bookCount(total)}
            </p>
          </div>
        ) : null
      }
    >
      {shown.map((book) => (
        <BookCard key={book.id} book={book} />
      ))}
    </Shelf>
  );
}

/** The page of someone the books name: who they are, their series and their books. */
function PersonPage() {
  const { personId } = Route.useParams();
  const person = usePerson(personId);
  const series = useSeriesOfPerson(personId);
  const isOwner = useIsOwner();
  const [doing, setDoing] = useState<"merge" | "names" | null>(null);

  if (person.isPending) {
    return null;
  }
  if (person.isError) {
    return gotNoAnswer(person.error) ? (
      <ServerNotAnswering />
    ) : (
      <NotOnServer title="This person is not on the server">
        No book you can read names them. They may have been made one with someone else.
      </NotOnServer>
    );
  }

  const { data } = person;
  const others = data.names.slice(1);
  const close = () => setDoing(null);
  return (
    <>
      <header className="mb-10 flex flex-wrap items-start justify-between gap-x-4 gap-y-3">
        <div className="min-w-0">
          <h1 className="font-bold text-screen-title tracking-tight">{data.name}</h1>
          <p className="mt-1 text-fg-muted text-metadata">{workOf(data)}</p>
          {others.length > 0 ? (
            <p className="mt-1 text-fg-muted text-metadata">
              Also spelled {others.map((name) => name.name).join(", ")}
            </p>
          ) : null}
        </div>
        {/* Who is who is the owner's to say. */}
        {isOwner ? (
          <MoreMenu about={data.name}>
            <MenuItem icon={Merge} onSelect={() => setDoing("merge")}>
              Merge with someone…
            </MenuItem>
            <MenuItem icon={Tags} onSelect={() => setDoing("names")}>
              Names…
            </MenuItem>
          </MoreMenu>
        ) : null}
      </header>

      {series.isError ? <ErrorText>{reason(series.error)}</ErrorText> : null}
      {series.data?.length ? (
        <Shelf title="Series">
          {series.data.map((one) => (
            <SeriesCard key={one.id} series={one} titleAs="h3" />
          ))}
        </Shelf>
      ) : null}
      <Books personId={personId} />

      <Dialog open={doing === "merge"} onClose={close} title={`Merge ${data.name} with someone`}>
        <MergeDialog person={data} onClose={close} />
      </Dialog>
      <Dialog open={doing === "names"} onClose={close} title={`Names of ${data.name}`}>
        <NamesDialog person={data} onClose={close} />
      </Dialog>
    </>
  );
}
