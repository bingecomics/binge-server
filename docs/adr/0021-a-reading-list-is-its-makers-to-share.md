# 0021. A reading list is its maker's, to share

## Status

Accepted on 2026-11-20, decided with the owner and built the same day as
routes of the HTTP API and pages of the web app. It replaces
[0014](0014-reading-lists-belong-to-the-server.md).

## Context

A reading list belonged to the server: the owner made and changed every
list, and everyone read them
([0014](0014-reading-lists-belong-to-the-server.md)). That record turned
down lists that belong to a person because nothing asked for them yet.

Something asks for them now. A member could not keep a list of their own.
On a server several people use, every list the owner made was in front of
everybody. A list someone makes for themselves is theirs, and whether
the others see it is theirs to choose. The owner asked for one thing more
than all or nothing: to share a list with certain people.

## Decision

A reading list belongs to the person who made it.

- **Anyone makes one.** The owner and every member make lists, add to
  them, order them, and import them from CBL files. A new list is seen by
  nobody else.
- **Only its maker changes it.** The name, the summary, the books, their
  order, who it is shared with, and deleting it are the maker's. The
  server's owner has no more say over a member's list than
  another member has, and is not shown a list that is not shared with
  them.
- **Sharing is the maker's call, and it is all of what the list keeps
  about who sees it:** with everyone on the server, those who join later
  too (`reading_list.shared_with_all`), or with the people chosen one by
  one (`reading_list_share`). It needs nobody's leave. `PUT
  /reading-lists/{id}/sharing` says all of it at once, so taking someone
  off is saying it again without them.
- **Someone a list is shared with reads it.** They see whose it is, the
  books of it that they may read, and their own progress through it. They
  are not told who else it is shared with. As before, a list that would
  show them no book is not there for them and is answered 404; their own
  list is always there for them, an empty one too.
- **A request to change someone else's list is refused with 403** when
  the caller sees the list, and answered 404 when they do not, so asking
  does not tell whether a list is there.
- **Names are unique among one person's lists.** Two people may each have
  a list called the same; on a page that shows both, each says whose it
  is.
- **A list goes with its maker.** Removing a user removes their lists, as
  it removes their progress.
- **Rules kept for CBL imports are each person's own** (`cbl_rule.user_id`).
  What one person says a name in such files means decides nothing in
  another person's import.
- **Everyone on the server is known to everyone by name.** `GET
  /users/names` answers any signed-in caller with the names and ids of
  the people on the server and nothing else about them. Sharing with
  chosen people needs it.

The lists that were there became the owner's, shared with everyone, so
nobody lost one. Everything else of 0014 stands: a list holds its books by
identity, each once, in an order; adding does exactly what was asked or is
refused; importing a CBL file is a dry run and the list is made by the
ordinary request.

## Consequences

- Choosing a cover stays the owner's, as it is for books and series, so a
  list has a chosen cover only when it is the owner's own. A member's list
  shows the covers of its first books. Letting each person choose a cover
  for their list needs a way to clear the files of a removed member's
  covers first.
- A list its maker shared with someone who reads none of its libraries
  says "Shared with" them to the maker and is not there for that person.
  Access to libraries is the owner's to give
  ([0009](0009-first-run-setup-makes-the-owner.md)), and a list does not
  go around it.
- A member learns the names of the other people on the server. On a
  server for a household or friends that is what sharing is for.
- When a member loses a library, the books of it in their own lists are
  no longer shown to them and keep their places, as books whose files are
  away do.
- An AI client acts as the person whose token it has, so the lists it
  makes are that person's.

## Alternatives considered

- **Two kinds of list**, the server's and a person's, as 0014 suggested
  for later. Two sets of rules for one thing. A list shared with everyone
  does what a server's list did.
- **Shared with everyone or with nobody.** It is less to build, and the
  owner asked for chosen people.
- **A permission to share**, given by the owner to each member. Sharing a
  list shows nobody a book they may not read, so there is nothing for the
  owner to guard.
- **The owner sees every list.** It would make "only you" untrue for
  members. The owner can still remove a member, and their lists go with
  them.
