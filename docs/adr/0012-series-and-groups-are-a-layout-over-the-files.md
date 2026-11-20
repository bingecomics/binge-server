# 0012. Series and groups are a layout over the files

## Status

Accepted on 2026-11-17, decided with the owner. Built the same day: import
and the layout made from the database, and then the operations that change
it, whose rules are in
[0013](0013-the-layout-changes-only-as-asked.md). Not built: the MCP
tools. The defaults under "Open questions" are still a draft.

Amended on 2026-11-17: an AI client changes the layout directly. The
proposals this record first decided on were dropped by the owner (see the
decision, the alternatives and 0013).

Amended on 2026-11-17, by the owner after seeing a real library: the one
group of specials held annuals, guest appearances, one-shots and collected
editions together. It is split into three kinds of group: annuals,
one-shots and appearances in other titles. The same day the owner asked
that every kind of group behave alike: the same parts, the same reading
order inside it, the same page. Only what a kind is called differs.

## Context

Today a series is a folder and a book is a file in it. The web app splits a
series into volumes by reading one tag. That breaks down in three places:
a folder that holds several runs of one title, specials mixed in with
numbered issues, the same title spread over two folders.

Another way reads a book's series, volume and number from the tag first,
then from the file name, then from the folder, and puts books with the
same series name together wherever they are. That fixes the folders. But
if what is shown is recomputed from the files on every scan, nothing that
was decided about it can be kept, and volumes and one bucket of specials
are not enough to say what is inside a series.

Binge is meant to be organized by an AI client working through MCP (ADR
0002). An AI can tell that two folders are one series, that an issue is an
annual, that a mini series belongs under its parent title. What it decides
has to live somewhere a rescan does not overwrite, and it cannot live in
the folders, which the server does not write to (ADR 0005).

## Decision

Two things are kept apart: the tags, which are the file's, and the
organization, which is Binge's.

**Tags** are what a file says about itself in its ComicInfo: credits,
summary, characters, dates and the rest. The server shows them as the
book's details. They never decide where a book sits.

- The server keeps a copy of them in its database, because a list, a sort
  or a search cannot open every archive each time.
- The copy only follows the file. Nobody edits it, neither a person nor an
  AI. When a file's size or modification time changes, its tags are read
  again and the copy is overwritten.
- That check runs in a scan. Proposed, not decided and not built: it also
  runs for the files of a book or a series when its page is opened, so a
  retagged file shows its new details without waiting for a scan.

**The organization** is a record in Binge's database, and the only thing
the layout is built from.

- Library › Series › Group › Issue. Everything is inside one library, and a
  book has one home.
- A series is a record with a name. It is not a folder.
- A group is a part of a series with its own page. Its kinds are volume,
  limited series, annuals, collected editions, one-shots and appearances
  in other titles. "Group" is the name in the code and the API. People see
  the kind.
- A book may sit directly in its series, in no group.
- Inside any group, numbered issues go in number order. The books with no
  number come after them: the dated ones by date published, and the
  undated ones last, in no order that means anything.
- A special is a book that is no issue of a run. It is an annual, a
  collected edition, an appearance or a one-shot.

**Import** writes a book's place once, when the server first sees it. It
is done by the scan, with no AI.

- The book's series, volume and number are taken from the tag, else the
  file name, else the folder. The file name patterns are the ones comic
  files are commonly named by: a series name, a volume (`v1`, `vol 2`), a
  number, an `SP` marker for a special. No guessing from words such as
  "Annual" in a name.
- Books whose series names match go in one series. Names are compared
  loosely (case, punctuation and spacing do not count), inside one library.
- A book is a special when its name carries the marker, when its `Format`
  tag names one, or when it has neither a number nor a volume. The `Format`
  tag says whether a special is an annual or a collected edition. Any other
  special goes in the one-shots: import never makes an appearance, which
  only the owner or an AI client can tell. A book that is no special goes
  in its volume when it has one. Anything else sits directly in the series.
- A file whose name gives no number and no volume takes its series from
  its folder, not from its own name. Making each such file a series of
  its own would scatter a folder of collected editions over the library.
  Kept together, they show as the books of one series, each by its name.
- After that the file is not asked again. Its path is only where the pages
  are.

**Changes** to the organization are made in Binge and nowhere else: by a
person in the web app, or by an AI client through MCP. Both use the same
operations, and a change takes effect when it is made
([0013](0013-the-layout-changes-only-as-asked.md)).

**Files are not touched.** Retagging files through MCP is wanted and will
be designed later. It falls under ADR 0005: explicit write access, and a
dry run first.

## Data sketch

A draft. Names and shapes will change when it is built.

| Table | What it holds |
| --- | --- |
| `book_info` | As today: the copy of the file's tags, overwritten whenever the file changes. It can grow to more of ComicInfo's fields |
| `series` | `(id, library_id, name, name_key)`. `name_key` is the name as it is compared, once per library. The `folder` column is gone |
| `series_group` | `(id, series_id, kind, number, name)`. `kind` is `volume`, `limited`, `annual`, `collected`, `one_shot` or `appearance` |
| `book_place` | `(library_id, content_hash, series_id, group_id, number, published)`. Written at import, changed only in Binge. Kept by the book's identity (ADR 0004), so it stays when the file is moved, renamed, retagged or gone for a while |
| `book` | One row per file, and nothing about where it sits: lists join it to `book_place` by library and identity. A file that cannot be read has no identity, so it has no place and is in no series; a library lists such files apart. (The first build also copied the place onto this row. The copy went on 2026-11-17, so that changing a place is one write.) |
| `proposal`, `proposal_change` | Never built. They were to hold an AI client's changes while they waited for the owner. Dropped with proposals on 2026-11-17 |

## Consequences

- Retagging a file changes the details shown and never where the book
  sits. There is nothing to import again.
- A library whose share is offline during a scan loses nothing: the places
  are kept by identity and the books return to them.
- A folder that holds two series shows as two. The same series in two
  folders shows as one. Two different series that share a name are
  imported as one until somebody splits them.
- With no AI, a library is its series by name, each a flat list of numbered
  issues, with volumes and specials where the files marked them. That is
  the floor, not the goal.
- A limited series arrives from import as a series of its own, because its
  name differs from its parent's. Putting it under the parent is the AI's
  work, or a person's.
- A better import rule in a later version does not rearrange books that
  are already in. Until places can be changed in Binge, removing a library
  and adding it again is the way to import it afresh.
- When a series is renamed or two are merged, import will need to know the
  names it used to go by. Not needed until a place can be changed.
- An AI can improve a book's details only by retagging its file, which
  needs write access. In a read-only library the details stay as the files
  have them.
- The volume tabs the web app works out by itself are replaced by groups
  from the server, and the API for series and books changes. It may, until
  the iOS reader connects.
- The MCP step in the plan needs tools that change the layout, not only
  read-only ones.
- Nothing in the layout names a folder, so a library that is not a file
  system becomes possible. None is planned.

## Open questions

1. **The AI's tools.** Settled in part by 0013: the tools that change the
   layout are its operations, called directly. Open: which tools the MCP
   server offers for reading, and whether changes get a history or an
   undo.
2. **Retagging through MCP.** Set aside by the owner for later.
3. **Dates.** Settled when it was built: import copies the date published
   into the place, so the layout reads no tag at all.
4. **A one-shot tied to nothing.** Default: a series that holds one
   special, shown as a single book.
5. **Story arcs, collections and reading lists across series.** A story
   arc is a tag that several issues share. Whether a collection is built
   from it, and how it is shown, is not decided.

## Alternatives considered

- **A series is a folder**, as in Binge until now. Predictable, and
  it ties what people see to how the files happen to be stored.
- **Work the layout out from the files on every scan**, reading the tag,
  then the file name, then the folder. That order is taken for import.
  Not taken: recomputing what is shown on every scan, which keeps nothing
  that was decided, and knowing only volumes and one bucket of specials.
- **One store of attributes with a recorded source for each value** (tag,
  file name, folder, AI, person) and an order of strength between them.
  The first draft of this record. Turned down by the owner as more
  complicated than the problem.
- **Import every tag into the database and make that the truth**, with an
  action to import again after a file is retagged. Turned down by the
  owner: tags stay the file's and are read as they are.
- **Let the AI rename files and folders, and keep the layout in the file
  system.** Against ADR 0005 as the default, and no use for a library that
  is not a file system.
- **An AI's changes come as proposals**, and nothing changes until the
  owner approves one. The first decision here. Dropped by the owner on
  2026-11-17 for direct changes: see 0013.
