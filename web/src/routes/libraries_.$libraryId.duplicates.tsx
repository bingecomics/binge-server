import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronLeft, CopyCheck } from "lucide-react";
import { useIsOwner } from "../api/account";
import type { HeldTwice } from "../api/client";
import { useHeldTwice, useLibrary } from "../api/libraries";
import { Cover } from "../components/Cover";
import { Notice, NotOnServer } from "../components/Notice";
import { rowClass } from "../components/Section";
import { ErrorText, PathText, textLinkClass, upLinkClass } from "../components/text";
import { called, count, moment, size, titleUnder } from "../format";

export const Route = createFileRoute("/libraries_/$libraryId/duplicates")({
  params: {
    parse: ({ libraryId }) => ({ libraryId: Number(libraryId) }),
    stringify: ({ libraryId }) => ({ libraryId: String(libraryId) }),
  },
  component: DuplicatesPage,
});

/** One book the library holds more than once: what it is, and each of its files. */
function Twice({ held }: { held: HeldTwice }) {
  // Every file of it is the same book: the first says which.
  const book = held.files[0]?.book;
  if (!book) {
    return null;
  }
  const under = titleUnder(book);
  return (
    <li className={`${rowClass} flex gap-4`}>
      <div className="w-16 shrink-0">
        <Cover cover={book.cover} />
      </div>
      <div className="min-w-0 flex-1">
        <h2 className="font-semibold">
          <Link to="/books/$bookId" params={{ bookId: book.id }} className="hover:underline">
            {called(book)}
          </Link>
        </h2>
        {under ? <p className="text-fg-muted text-metadata">{under}</p> : null}
        <ul className="mt-2 flex flex-col gap-2">
          {held.files.map((file) => (
            <li key={file.book.id}>
              <PathText path={file.path} />
              <p className="text-caption text-fg-muted">
                {size(file.book.file_size)} · Added {moment(file.book.added_at)}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </li>
  );
}

/**
 * The books a library holds in more than one file. They are the same book
 * by their pages, whatever the files are called and however they are
 * tagged. The server only says so: the files are the owner's to remove.
 */
function DuplicatesPage() {
  const { libraryId } = Route.useParams();
  const { library, isPending } = useLibrary(libraryId);
  const isOwner = useIsOwner();
  // Only the owner is answered.
  const held = useHeldTwice(libraryId, isOwner);

  if (isPending) {
    return null;
  }
  if (!library || !isOwner) {
    return (
      <NotOnServer title="This page is not here">
        It is the owner's, for a library that is on the server.
      </NotOnServer>
    );
  }
  const files = held.data?.reduce((all, one) => all + one.files.length, 0) ?? 0;
  return (
    <>
      <Link to="/libraries/$libraryId" params={{ libraryId }} className={upLinkClass}>
        <ChevronLeft className="size-4" aria-hidden="true" />
        {library.name}
      </Link>
      <h1 className="mt-2 mb-2 font-bold text-screen-title tracking-tight">Books held twice</h1>
      {held.isError ? (
        <ErrorText>{held.error.message}</ErrorText>
      ) : held.data?.length === 0 ? (
        <Notice icon={CopyCheck} title="No book is here twice">
          Every file of this library has pages of its own.
        </Notice>
      ) : held.data ? (
        <>
          <p className="max-w-prose text-fg-muted">
            {count(held.data.length, "book is", "books are")} in this library more than once, in{" "}
            {count(files, "file", "files")}. The files of a book have the same pages, whatever they
            are called. The server deletes nothing: take the files you do not want out of the
            folder, then{" "}
            <Link to="/libraries/$libraryId" params={{ libraryId }} className={textLinkClass}>
              scan the library again
            </Link>
            .
          </p>
          <ul className="mt-6 max-w-3xl">
            {held.data.map((one) => (
              <Twice key={one.files[0]?.book.id} held={one} />
            ))}
          </ul>
        </>
      ) : null}
    </>
  );
}
