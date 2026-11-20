# 0017. A view is a library narrowed, kept by name

## Status

Accepted on 2026-11-18, decided with step W6 of the plan and built the
same day as routes of the HTTP API and pages of the web app. The MCP tool
that will make views is not built: an AI client makes one today with the
owner's API token and `POST /views`.

## Context

W4 made a library something that can be narrowed and ordered, with what is
asked written in the address: "unread Vertigo from the nineties" is
`/libraries/2?read=unread&publisher=Vertigo&year=1990s`. W6 asks for such
a thing to be kept: as a row on Home, as an entry in the menu, and made by
the owner or by an AI client. Its step is done when a row made by an AI
client appears on Home.

A kept filter can be both a row of Home and an entry of the side menu:
one thing, two places.

Three questions had to be answered.

- **Whose is it.** A filter could be each person's own, as a bookmark is.
  But the one who makes them here is the owner, or an AI client acting for
  the owner, for everyone who reads the server: "this month's picks",
  "new readers start here". Reading lists were decided the same way
  ([0014](0014-reading-lists-belong-to-the-server.md)).
- **What is kept.** The address could be kept as text. Then a person the
  view narrows by could be merged into another and the text would name
  someone who is no longer there, and nothing could check a view when it
  is made.
- **What it shows.** A view narrowed by "unread" cannot hold a list of
  series: unread is not the same series for two readers.

## Decision

A view is a library, an order and a narrowing, kept under a name. It is
not a list of series: its series are asked for when it is shown, for the
one who looks.

- **It belongs to the server.** The owner makes, changes and deletes
  views. Everyone who may read the view's library sees it, and nobody
  else. What it shows is each reader's own where it is narrowed or ordered
  by reading.
- **It is kept as what it asks, not as an address.** `kept_view` holds the
  library, the name, the order and where it shows; `kept_view_value` holds
  one row for each value it asks for or rules out. A view answers with
  `asked`, which has the fields the series of a library take, so a client
  shows a view by asking `GET /libraries/{id}/series` with them. What a
  view asks is read as that route reads it, so a year that is not one or
  an order the server does not know is refused when the view is made.
- **A person in a view is followed.** When two people are made one, the
  views that narrowed by the one that went narrow by the one that stayed.
- **One thing, two places.** `on_home` makes it a row on Home and
  `in_menu` an entry in the menu. Either, both or neither: a view in
  neither is still kept and listed for the owner.
- **Where a row sits is each person's.** The server says which views are
  rows; the order of Home, and whether a row is shown at all, is what
  each person chose ([0018](0018-what-a-person-chooses-is-kept-as-keys-and-values.md)).
- **A view goes with its library.** Removing the library removes its
  views.

## Consequences

- A row made by an AI client appears on Home for every reader of that
  library, without anyone arranging anything. That is the point, and it
  means making a view is the owner's to allow: the routes that write are
  the owner's.
- Nothing has to be kept up to date. A view over a library that grew shows
  the new series the next time it is asked for.
- A view is of one library. "Unread across everything" is not a view
  today; it would need a narrowing that spans libraries, which the series
  of a library do not have.
- A member cannot keep a view of their own. If that is wanted later, a
  view gets an owner column and the same routes serve it; nothing kept
  today stands in the way.
- The MCP tool that makes a view needs nothing new in the core:
  `Catalog::add_view` is the operation, and `GET /libraries/{id}/choices`
  says what a library can be narrowed by.

## Alternatives considered

- **Keep the address as text.** The smallest thing to build, and what a
  browser bookmark already is. It cannot be checked, does not follow a
  merged person, and ties what is kept to how the web app writes its
  addresses.
- **A view per person.** Right for bookmarks, wrong for the one who is
  asked to make them here: an AI client would have to make the same row
  once for every reader.
- **A list of series, filled in when the view is made.** It would go stale
  with the first scan and could not say "unread" at all.
