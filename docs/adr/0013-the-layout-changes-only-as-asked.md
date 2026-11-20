# 0013. The layout changes only as asked

## Status

Accepted on 2026-11-17, decided with the owner. Built the same day as
routes of the HTTP API. The MCP tools that will use the same operations are
not built.

## Context

Series, groups and places are the server's own record
([0012](0012-series-and-groups-are-a-layout-over-the-files.md)). Import
fills it once. Until now nothing could change it afterwards, so a wrong
guess at import stayed wrong.

The record is meant to be put right by an AI client, and by a person. The
first sketch of the operations was two: set a book's place, and rename a
series, where renaming a series onto a name that another one has would
merge the two. The owner turned the merge down: the server is not to take
any decision by itself. Whoever asks for a change must get either exactly
that change or an error that says why not.

## Decision

Every operation on the layout does exactly what was asked, or is refused
with the reason. The server decides nothing on the side.

Whoever changes the layout calls these operations directly, and a change
takes effect when it is made. That holds for an AI client as for a person:
it acts as the owner, with one of the owner's API tokens today and through
MCP tools later, and is told at once when a request is refused. Nothing
waits for approval. This replaces the proposals of 0012, which the owner
dropped on 2026-11-17.

The operations, all for the owner:

| Operation | Refused when |
| --- | --- |
| Create a series in a library | Another series of the library has the name |
| Rename a series | Another series of the library has the name |
| Delete a series | It holds books |
| Create a group in a series | The series has a group of that kind, number and name |
| Rename a group: its number or its name | The series has a group of that kind, number and name |
| Delete a group | It holds books |
| Set a book's place: series, group, number, date published | The series or group is not there, the group is in another series, the series is in another library, or the file could not be read |

What follows from "nothing on the side":

- **A name that is taken is an error, not a merge.** To merge two series,
  the books of one are moved to the other, one by one, and the emptied
  series is deleted. The error carries the id of the series or group that
  is in the way, so whoever asked does not have to look for it.
- **Names are compared as import compares them**: case, accents, spaces
  and punctuation do not count. `Spider-Man` and `spider man` are one name.
  A series may take its own name written another way.
- **A place names its series and group by id.** An id that is not there is
  an error. Setting a place never makes a series or a group.
- **A place is given whole.** What a request leaves out, the book does not
  have: no group means the book sits directly in its series.
- **Nothing is read into a request.** A part the server does not know,
  such as a misspelt `groupId`, is refused and not taken as left out. So is
  a blank name or number: only leaving a part out means "none".
- **Every kind of group has the same parts.** A group may have a number,
  from 1 to 65535 as a volume's is at import, a name, both or neither,
  whatever its kind. Two groups of a series are the same when kind, number
  and name are, so a series may hold several groups of one kind: the
  appearances in "Showcase" beside those in "The Brave and the Bold".
  (Until 2026-11-17 each kind had parts of its own: a volume needed a
  number and took no name, and so on. The owner asked for one rule: a
  group's kind is its name, not its behaviour.)
- **Issue numbers are not unique.** Two scans of the same issue are two
  books and may both be number 5 of the same group.
- **Nothing is removed for being empty.** A series or a group that holds no
  books is listed like any other and stays until it is deleted. This
  includes a series whose files have all gone from the folder: the scan
  no longer hides it.
- **Only what holds no books is deleted.** "Holds books" means files that
  are in the library now. Deleting a series also forgets the places it kept
  for books whose files are gone; such a book is placed afresh, by import,
  if its file comes back. Deleting a group leaves such a book in its series
  with no group.

Import stays the one thing that decides by itself, as 0012 says: a book
with no place gets one from its tag, file name and folder, and a series or
a group is made for it when it needs one. It does this once per book.

## Consequences

- Fixing the layout takes several small requests, where a server that
  guessed would take one. That is the price of never being surprised by
  what the server did. A client that wants "merge these two" writes the
  loop itself.
- Import does not remember the names a series used to go by. After
  "Batman" is renamed "Batman (2016)", a new file tagged "Batman" starts a
  new series called "Batman". 0012 expected import to need the old names;
  that is not built, and under this rule putting the book right is one more
  request.
- An emptied series shows in the web app as a series with no books. Its
  page says so and offers the owner to delete it. Nobody is left with a
  card they cannot get rid of.
- The API's error shape gains one optional part, `conflicts_with`. Every
  other error is unchanged.
- An AI client's changes are live as it makes them. Nothing holds them
  back for the owner to look at first, and nothing records what was
  changed: there is no undo and no history. A change that was wrong is put
  right by another change. The files are never touched, and removing a
  library and adding it again imports it afresh.
- The operations live in `crates/core`, with no HTTP in them, so the MCP
  server can call the same code and gets the same refusals.

## Alternatives considered

- **Proposals: an AI client's changes wait until the owner approves
  them.** What 0012 first decided. It needs a store of pending changes, a
  page to review them, and rules for a proposal that no longer fits what
  it was made against. Dropped by the owner, who wants the AI client to
  call the operations itself and to get the error when one is refused.

- **Renaming onto a taken name merges the two series.** One request
  instead of many. Turned down by the owner: it is the server deciding what
  happens to every book of a series because of a name.
- **Setting a place by series name, making the series when it is not
  there.** Convenient, and a typing mistake becomes a new series.
- **Removing a series as soon as it is empty.** What the scan did before.
  It makes a series vanish as a side effect of moving its last book, and a
  series that was just created would vanish too.
- **Hiding empty series from the lists and keeping them in the
  database.** What the lists did before for a series whose files were
  gone. Whoever created a series could not see it, and whoever emptied one
  could not find it to delete it.
