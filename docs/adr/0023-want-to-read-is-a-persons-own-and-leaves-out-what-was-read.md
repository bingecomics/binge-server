# 0023. Want to read is a person's own, and leaves out what was read

## Status

Accepted on 2026-11-20 and built the same day as routes of the HTTP API
and pages of the web app.

## Context

People keep a list of the series they mean to read. It is not a reading
list ([0021](0021-a-reading-list-is-its-makers-to-share.md)), which holds
books in an order, and not a collection
([0022](0022-a-collection-holds-series-and-is-shared-as-a-list-is.md)),
which is named and shared: it is one list, each person has theirs, and
nobody else has a use for it.

One thing had to be settled: what happens to a series on it once the
person has read all of it. Left there, the list fills with what is
already read and stops saying what to read. Taken off for good, a series
that is still coming out is lost from it the day the person catches up.

## Decision

Each person has one want to read, of whole series.

- **It is theirs alone.** Nobody else is shown it, the owner of the
  server included, and there is nothing to share. It goes with its
  person.
- **A series is on it until it is taken off.** Every series says to the
  one who asks whether it is on theirs (`wanted`), so a menu offers to
  put it there or to take it off.
- **What was read to the end is left out.** The list a person is shown
  holds the series on it that have a book they have not read. A series
  left out is still on it: it is back by itself when it gets a book, or
  when a book of it is marked as unread, and it is still theirs to take
  off.
- **The last put there comes first.** Series put there together come by
  name.
- **It holds the record of each series**, as a collection does (0022): a
  series stays on it under another name and leaves it when it is deleted
  or its library is removed. A series of a library the person no longer
  reads is not shown, and is back when they read the library again.
- **Putting takes whole series**, one or many: from the menu of a series
  and from the bar of what is picked when only series are picked. A
  series the person may not read is refused, and nothing is put there.
- **It is a row of Home**, arranged and put away like the others.

## Consequences

- Nothing is written when a book is read. What is left out is worked out
  when the list is asked for, so a mark put by mistake and taken back
  costs nothing.
- A person does not see the series left out, and cannot list them. The
  menu of such a series still offers to take it off.
- `Series` grows `wanted` everywhere a series is answered, which is one
  more look-up for each series.
- The API grows `GET /want-to-read`, `POST /want-to-read` and
  `DELETE /want-to-read/{series_id}`.

## Alternatives considered

- **Take a series off when it is read to the end.** Every way a book
  becomes read would have to do it, a mark put by mistake would lose the
  series, and a series that is still coming out would have to be put back
  by hand with each new book.
- **Leave everything on it until taken off.** The simplest, and the list
  would hold more and more of what is read.
- **A reading list or a collection called "Want to read".** A person
  would have to make it, and could delete or share it. A menu could not
  offer it in one step.
