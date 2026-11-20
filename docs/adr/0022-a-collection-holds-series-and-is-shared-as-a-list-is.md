# 0022. A collection holds series, and is shared as a list is

## Status

Accepted on 2026-11-20, decided with the owner and built the same day as
routes of the HTTP API and pages of the web app.

## Context

A reading list puts books in an order
([0021](0021-a-reading-list-is-its-makers-to-share.md)). People also keep
series together with no order among them: a publisher's event, a writer's
work, what is read with the children. A reading list does not serve for
that: it holds books, so a series that gets a new issue would have to be
added to again, and it asks for an order nobody has in mind.

A series is a record of the layout, not a folder
([0012](0012-series-and-groups-are-a-layout-over-the-files.md)). It is
changed only as asked ([0013](0013-the-layout-changes-only-as-asked.md)):
given another name, given or losing books, and deleted once it is empty.
What a collection keeps through those changes had to be settled before
its table was.

## Decision

A collection is a name, a summary and series picked by hand, from any
library.

- **It holds the record of each series.** A series stays in it under
  another name and with other books. Books moved from one series to
  another do not take the collection with them: the series they left is
  still the one in it. A series that is deleted leaves it, and so do the
  series of a library that is removed.
- **No order of its own.** Its series are shown by name. It holds a
  series once.
- **It is its maker's, to share**, exactly as a reading list is (0021):
  any member makes one, only the maker changes it, and it is shared with
  the people chosen or with everyone. Someone it is shared with sees
  whose it is and the series they may read. A shared collection that
  would show them no series is not there for them. Names are unique among
  one person's collections. A collection goes with its maker.
- **Adding takes whole series.** "Add to a collection…" is in the menu of
  a series and in the bar of what is picked when only series are picked.
  A series the person may not read is refused, and nothing is added.
- **Search finds it by its name**, and a series' page says which
  collections it is in, and which reading lists hold a book of it.
- **Its cover** is made of the covers of its first series. No cover is
  chosen for it.

## Consequences

- The part that reading lists and collections share, whose a thing is and
  who it is shared with, is one piece of the core (`sharing.rs`), so the
  two cannot drift apart.
- Merging two series by hand, by moving the books and deleting the empty
  one, takes the emptied series out of its collections. The person adds
  the other one. An operation that merges series, if one is built, has to
  carry collections over.
- A collection can be empty, and for someone it is shared with it can
  hold fewer series than for its maker.
- The API grows `/collections`, `/collections/{id}`,
  `/collections/{id}/sharing`, `/collections/{id}/series`,
  `/series/{id}/collections` and `/series/{id}/reading-lists`. A search
  answers with `collections` too.

## Alternatives considered

- **Hold series by name**, as a reading list holds books by identity. A
  series has no identity beyond its record: a name is changed on purpose,
  and two libraries can each have a series of one name.
- **Hold books, and show their series.** It would follow books that are
  moved, and a collection would change as issues are placed, which nobody
  asked for.
- **One kind of thing for lists and collections.** They hold different
  things and one has an order. The shared part is in one place without
  it.
- **Any book, group or series in a collection.** People pick series. A
  group or a book can come later without changing what is here.
