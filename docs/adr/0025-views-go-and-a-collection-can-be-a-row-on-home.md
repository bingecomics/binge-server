# 0025. Views go, and a collection can be a row on Home

## Status

Accepted on 2026-11-21. The owner decided it on 2026-11-20, and it was
built the next day: a migration, a route of the HTTP API, and the web app.
It replaces
[0017](0017-a-view-is-a-library-narrowed-and-kept-by-name.md).

## Context

A view ([0017](0017-a-view-is-a-library-narrowed-and-kept-by-name.md))
kept what a library's page was asked under a name, and showed it as a row
on Home or an entry in the menu. It was the only way to put a row of
one's own on Home. It had three limits: only the owner made one, it was
of one library, and it could ask only what the server has a field for.

Since then a collection
([0022](0022-a-collection-holds-series-and-is-shared-as-a-list-is.md))
does the rest of that job with none of the limits. Anyone makes one, its
series come from any library, they are picked and not asked for, it is
shared with the people chosen or with everyone, and a library makes them
from what its files say
([0024](0024-a-library-makes-lists-and-collections-from-what-its-files-say.md)).

That left two things a person had to tell apart to answer one question,
"how do I get this on Home?", and two places for them: views in the
menu and in the settings, collections with the rest of what is a
person's own.

## Decision

Views are removed, and a collection can be a row on Home.

- **Nothing is kept of a view.** The tables, the routes under `/views`
  and the pages that made and changed views go. The views there were are
  dropped: a view was a question and a collection is an answer, and the
  server does not turn one into the other.
- **A library is still narrowed.** Its order and what narrows it stay in
  the address of its page, which a browser keeps and a person sends to
  another. The server keeps none of it.
- **A collection's maker puts it on Home.** `PUT
  /collections/{id}/on-home` sets `on_home`. Only the one whose
  collection it is can.
- **It is a row for everyone who sees it.** Each person's row holds the
  series they may read. A collection shared with everyone is a row on
  everyone's Home, and one that is nobody else's is a row on its maker's
  alone.
- **Where the row sits is each person's.** `home.rows`
  ([0018](0018-what-a-person-chooses-is-kept-as-keys-and-values.md))
  names it `collection:` and the id, in the order the person chose, and
  among the ones they put away.
- **Not in the menu.** A collection is reached from the collections, with
  the rest of what is a person's own.

## Consequences

- One thing fewer to learn, and a row on Home is no longer the owner's
  alone: a member has rows of their own, and a row holds series of more
  than one library.
- A row does not keep itself up to date. A collection holds what was put
  in it, and gains a series when someone adds one, or when a scan does
  for a collection its files made. "Everything of this publisher" no
  longer grows by itself.
- A row cannot follow how far someone has read. Home's own rows do that:
  what is being read, what comes next, and what a person wants to read.
- Someone who shares a collection and puts it on Home adds a row to the
  Home of the people it is shared with. They put it away where they
  arrange Home. Sharing already shows it among their collections.
- An arrangement of Home that names the row of a view names a row that no
  longer exists, which a client leaves out, as 0018 asks of it.
- `/views` leaves the API. `/api/v1` is not frozen yet.
- What 0018 says of Home's arrangement and the server's views now holds
  for the collections that are rows.

## Alternatives considered

- **Keeping both.** Two ways to the same row, one of them the owner's
  alone and of one library.
- **Making a collection of each view.** What a view showed depended on
  who looked and changed with the library. A collection made from it
  would hold what one person saw on one day, under a name that says
  otherwise.
- **Each person chooses which collections are rows on their own Home.**
  The owner could then not set up Home for the people of the server, which
  is what a view was for. Putting a row away is already each person's.
- **Only the owner's collections become rows on other people's Home.** A
  rule more, for something each person undoes where they arrange Home.
