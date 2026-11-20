# 0014. Reading lists belong to the server

## Status

Superseded by [0021](0021-a-reading-list-is-its-makers-to-share.md) on
2026-11-20: a list now belongs to the person who made it. What this record
says of how a list holds its books and of importing a CBL file still
stands.

Accepted on 2026-11-18, proposed to the owner with the plan for step W3 and
built the same day as routes of the HTTP API and pages of the web app. The
MCP tools that will use the same operations are not built.

## Context

A reading list is books of any series in an order someone chose: a
crossover, an event, a character's story across titles. The plan left one
question open for when it was built: whose is a list?

There are two answers. A list can belong to the server: an administrator
makes it, everyone sees it, and a reader is not shown the books of a list
that they may not read. Or it can belong to a person, who may then show
it to everybody.

Binge Server has one owner and the members the owner lets in
([0009](0009-first-run-setup-makes-the-owner.md)). The owner is the one who
arranges the library: series, groups and places are theirs to change
([0013](0013-the-layout-changes-only-as-asked.md)). A reading list is the
same kind of work, and most lists arrive from outside as CBL files that
describe a published order, not a person's taste. An AI client, which acts
as the owner, is expected to make lists too.

## Decision

A reading list belongs to the server.

- The owner makes a list, changes it, imports one and deletes it. Members
  read.
- Everyone sees the same lists, each with the books they may read. A list
  that would show a member no book is not there for them: it is answered
  404, as a library they may not read is. The owner sees every list, an
  empty one too.
- What is personal stays personal: how far someone is through a list is
  worked out from their own progress, which hangs on the books
  ([0004](0004-identify-books-by-page-content.md)), not on the list.
- A list holds its books by identity, each once, in an order. A book whose
  file is moved or renamed stays in it. One whose file is gone is not
  shown, keeps its place, and is back when the file is.
- Names are unique among lists by the same key as series' names: capitals,
  accents, spaces and punctuation do not tell two apart.
- Adding does exactly what was asked or is refused, as the layout's
  operations are: a request that names something that is not on the
  server, or a file that could not be read, adds nothing. A book the list
  already holds is skipped, and the answer says how many were added.

Importing a CBL file makes nothing by itself. `POST /reading-lists/import`
reads the file and answers with a plan: for each entry the book found and
how, or why none was. The server keeps nothing of the file. The owner puts
right what was missed by saying which series a name means, or which book
an entry is, and that is kept as a rule (`cbl_rule`) for this import and
the ones after. The list is then made with the ordinary request that makes
any list, from the books the plan found. So there is one way to make a
list, a dry run cannot leave anything behind, and an AI client can do the
whole import with the same four calls.

## Consequences

- A member cannot keep a list of their own. If that is wanted later it is
  a second kind of list, owned by a person, and this one stays as it is.
- Positions in the API count among the books the caller is shown. The
  server maps them onto the whole list, so the books that are not shown
  keep their neighbours when the order is changed.
- A list's cover is chosen as a book's or a series' is, and kept the same
  way. Without one, the covers of its first four books stand for it
  together. A chosen cover is the list's face for everyone who sees the
  list: a member sees it also when it is a page of a book they may not
  read.
- The reader follows a list by carrying its id in the address
  (`/read/{book}?list={id}`). The server has no "next book of the list"
  route: a client that reads from a list has the list's books and takes
  the one after.
- Rules outlive the import they were made for. One that turns out wrong is
  forgotten from the same table of entries, where the entry it matched
  says so.

## Alternatives considered

- **Lists that belong to a person, shared on request.** More
  moving parts for a server with one owner: who may see, who may change,
  what happens to a shared list when its maker is removed. Nothing the
  owner asked for needs it yet.
- **An import the server keeps half done**, with its own table of matches
  until it is confirmed. It would need a way to list, resume and abandon
  imports. Sending the file again is cheap, and the rules already carry
  what was decided from one try to the next.
