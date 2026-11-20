# 0016. A person is the names they go by

## Status

Accepted on 2026-11-18, decided with step W4 of the plan and built the
same day as routes of the HTTP API and pages of the web app. The MCP tools
that will propose names and merges are not built.

## Context

Files name the people who made a book as text: `Writer` and `Penciller` in
`ComicInfo.xml`, several names in one field, split by commas. The same
person is spelled several ways across a library: with and without a middle
initial, surname first, with a number after the name where a tagging
source tells two people of one name apart, with different capitals or
accents. W4 asks for a page of a writer's work and for a library narrowed
by a writer. Both need to know that two spellings are one person, and the
files cannot be made to agree: the server does not write in a library
([0005](0005-library-folders-are-read-only.md)).

A server can keep people, each with the other names they go by and a way
to make two into one, or keep the names as text and have no person.

The owner's rules bear on it twice more. The server takes no decision of
its own about how the library is arranged
([0013](0013-the-layout-changes-only-as-asked.md)), and guessing that two
spellings are one person is such a decision: "J. Smith" and "John Smith"
may or may not be. And the server does no inference
([0002](0002-ai-work-runs-in-the-users-ai-client.md)): working out who is
who is the kind of work an AI client does well and proposes.

## Decision

A person is a set of names. `person` is the person and the name they are
shown by, `person_name` is one way of writing them, and `book_credit` says
that a book names a spelling for a job (writer or penciller).

- A credit holds the spelling, not the person. Making two people one moves
  the names of one to the other, and every book follows without being
  touched. Taking a name back out makes it a person of its own again, and
  its books follow it back. Both are exact and can be undone.
- Two spellings that differ only in capitals, accents, spaces or
  punctuation are one name from the start, by the same key that tells
  series' names apart. Nothing else is merged by the server: not initials,
  not word order, not a number after the name.
- The owner says the rest: merges people, adds a name before any file uses
  it, chooses the name a person is shown by, and makes a name its own
  person again. Members read.
- All of it is kept in the database, in the data directory. A name stays
  when no file spells it any more, so what the owner said outlives a
  rescan, a retagging and a library that is removed and added again.
- A person is there for someone only when a book they may read names them,
  as a reading list is ([0014](0014-reading-lists-belong-to-the-server.md)).
  Counts are over those books.
- The credits are written by the scan, from the same reading of the file
  that fills `book_info`. Books scanned before this existed are filled in
  once, when the server starts, from what `book_info` already holds: no
  file is opened again.

A library is narrowed by person, not by spelling, and so are the links
from a book's and a series' credits.

## Consequences

- The names in a book's `info` are still what its file says. The people of
  a book are asked for with `GET /books/{id}/credits`, and an overview of
  a series, a group or a reading list names people, each with an id.
- Until the owner merges them, a writer spelled two ways is two people,
  each with part of the work. That is visible on the person's page and is
  put right there in two clicks. It is the price of the server not
  guessing.
- Only writers and pencillers are people today, because those are the two
  jobs the scan reads. Another job is another value of the credit's role,
  with no change to people.
- A merge is the owner's and applies to everyone: there is one answer to
  who a person is on a server.
- An MCP tool that proposes merges needs nothing new in the core:
  `Catalog::merge_people`, `name_person` and `split_person` are the
  operations, and the list of people with their names is the input.

## Alternatives considered

- **Names as text.** No page of a person that survives two
  spellings, and narrowing by writer would take one spelling at a time.
- **A table of aliases beside text credits.** Every question about a
  person would then go through text comparison at the time it is asked;
  with the names as rows the join is by id and the alias is data like the
  rest.
- **Merging by likeness at scan time** (initials, surname first, trailing
  numbers). It would be right most of the time and wrong without a trace,
  joining two people who share a name. A proposal an owner accepts is the
  same convenience with someone answerable for it.
