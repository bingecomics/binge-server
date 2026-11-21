# 0024. A library makes lists and collections from what its files say

## Status

Accepted on 2026-11-20 and built the same day in the scan, as a route of
the HTTP API and as a choice of each library in the web app.

## Context

Tagging tools write into a file what its book is part of: a story arc
with the book's number in it (`StoryArc` and `StoryArcNumber`), another
series the book is also an issue of, as in a crossover (`AlternateSeries`
and `AlternateNumber`), and a group its series belongs to (`SeriesGroup`).
People have tagged whole libraries this way and expect a server to show
it: an arc as a reading list in its order, a group as a collection. Made
by hand, that is hundreds of lists.

Three things had to be settled.

- **Whether a library does it unasked.** It fills the server with lists
  nobody made, and they are shared.
- **Whose they are, and what they are afterwards.** A reading list
  ([0021](0021-a-reading-list-is-its-makers-to-share.md)) and a
  collection
  ([0022](0022-a-collection-holds-series-and-is-shared-as-a-list-is.md))
  belong to a person, who changes them by hand.
- **What the files may undo.** A list that follows its files exactly
  cannot be put right by its owner without retagging, and the server
  never writes to a file. A list its owner changes by hand must not be
  put back by the next scan.

## Decision

A library makes reading lists and collections from what its files say
when its owner asks it to.

- **It is a choice of each library, and two choices.** Reading lists from
  arcs, and collections from groups of series, are turned on and off
  apart. A library makes nothing until it is asked: the API adds a
  library that makes nothing unless the request says otherwise, and the
  web app asks when a library is added, with both ticked. A library that
  was there before makes nothing until its owner turns it on.
- **What the files say is always kept.** The scan keeps the arcs and the
  groups each file names, one row for each, whatever the library makes.
  So turning a choice on makes everything at once, from all the files
  were found to say, with no scan.
- **A story arc becomes a reading list.** Several arcs in one tag are
  separated by commas, and go with their numbers by the order both are
  written in. The other series a book is an issue of counts as an arc,
  with its number. The books of a list come in the order their numbers
  give, across series, and the ones with no number after them by name.
- **A group of series becomes a collection** of the series whose books
  name it. Several groups in one tag are separated by commas.
- **They are the owner's, shared with everyone.** A person sees of them
  what they see of any list: the books and series of the libraries they
  read.
- **Afterwards they are a list and a collection like any other.** Their
  owner renames them, changes what they hold and its order, shares them
  otherwise and deletes them. Each remembers the name the files fill it
  under, so it is still filled under another name.
- **The files only add.** What a file says waits from when the file is
  read until a scan has put it where it goes: the book in the lists it
  names, each where its number says among the books there, and its
  series in the collections it names. A scan takes nothing out and moves
  nothing that is there. A book the owner took out of a list stays out
  until its file is read again, which a file is when it changes.
- **A series is put in a collection once.** Every issue of a series
  names its group, so the next issue would bring back a series its owner
  took out. A series that already went in from one of its books is not
  put again by another.
- **A list or a collection that is not there is made whole**, with
  everything that names it in the libraries that make them, not only
  what the scan just read. So one that was deleted comes back, complete,
  with the next file that names it.
- **A name already taken by one of the owner's is filled.** The files do
  not make a second list of a name the owner has: they put their books
  in that one, which stays as it was made and shared.
- **Turning a choice off makes nothing more.** What was made stays.

## Consequences

- No file is changed and nothing is kept in a library: the lists and
  collections are rows of the database like the ones made by hand.
- Files read before this was built are read once more by the next scan,
  to learn what they name. Pages, covers and progress stay. Until that
  scan has ended, a choice turned on makes only what was read so far, and
  the scan makes the rest.
- Each list and each collection is written by itself. A scan that stops
  halfway, or one that fails, leaves the rest waiting, and the next scan
  makes it. A choice that could not all be made when it was turned on is
  turned back, so that asking again makes it.
- Deleting a list the files made does not stop them from making it again
  when a file that names it is read. Turning the library's choice off
  does.
- A book stays where it is in a list when its file's number changes. Its
  owner moves it by hand.
- Two libraries that name the same arc fill one list.
- `Library` grows `from_files`, `ReadingList` and `Collection` grow
  `from_files`, and the API grows `PUT /libraries/{id}/from-files`.

## Alternatives considered

- **Make them for every library, unasked.** A library of well tagged
  files would put hundreds of shared lists on a server whose owner never
  asked for one.
- **A list that mirrors its files**, which nobody changes by hand. It
  would always agree with the tags, and a wrong tag could only be put
  right by writing to the file, which the server does not do.
- **Put back at every scan whatever the files say**, from all the files
  and not only the ones read. Nothing taken out by hand would stay out.
- **Keep no record of what the files made**, and find a list by its name
  alone. A list its owner renamed would be made a second time under the
  old name.
