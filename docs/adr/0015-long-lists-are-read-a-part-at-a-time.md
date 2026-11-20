# 0015. Long lists are read a part at a time

## Status

Accepted on 2026-11-18, decided with step W4 of the plan, where the plan
said paging would be decided, and built the same day.

## Context

Until W4 every list of the API was answered whole. That held while a
library was a few hundred series. The libraries Binge is for hold
thousands, and W4 added lists that grow with the whole server: everything
a writer is named on.

Three ways were open. Offsets (`offset` and `limit`, with a `total`) let a
client go to any part and say "page 3 of 12", and cost the database a scan
of what is skipped. A cursor (the last item seen) stays fast however deep
and survives items arriving, and cannot jump: no page numbers, and no bar
of letters that goes to "M". Not paging at all leaves the cost to every
client.

The orders W4 asks for decided it. A library is read by name, by date
added, by year, by last read, unread first and shuffled, and jumped through
by letter. A cursor needs one stable key per order and has none for a
shuffle; a jump to a letter is an offset by nature.

## Decision

Lists that grow with the library are read a part at a time, by `offset`
and `limit`. The answer holds the items and `total`, how many there are in
all under what was asked.

- `limit` is 100 when left out and 500 at most. A larger one is cut to
  500, not refused.
- Paged today: the series of a library (`GET /libraries/{id}/series`) and
  the books that name a person (`GET /people/{id}/books`).
- Lists bounded by one thing stay whole: the books of a series, of a group
  or of a reading list, a person's series, the libraries, the reading
  lists. Reading order inside a series or a list is one piece of work, and
  a client that shows it wants all of it.
- An order is always total: every sort ends in the name and then the id,
  so two requests for neighbouring parts never show an item twice or skip
  one while nothing changes.
- A shuffle is paged too. The client sends a `seed` and the server orders
  by a number worked out from the seed and the series' id, so the same
  seed gives the same shuffle on every page of it.
- The letters of a library (`GET /libraries/{id}/series/letters`) say, for
  each first letter some series has, how many series and the `offset` of
  the first one in name order, under the same narrowing. A client goes to
  a letter by asking for the part that holds that offset.

## Consequences

- `GET /libraries/{id}/series` answers `{ series, total }` where it
  answered a list. `/api/v1` is not frozen yet, so the change was made in
  place.
- A series added or removed between two requests can shift the parts by
  one: a series shows twice or is passed over. Asking again puts it right.
  For a library on one's own server this was judged a fair price for page
  numbers and letters.
- Deep offsets cost more than shallow ones. With `natsort` order on a few
  thousand rows in SQLite this is milliseconds; it is the first thing to
  look at if a library of hundreds of thousands of series is ever real.
- The web app shows a library 120 series to a page, with the page in the
  address, and a person's books 60 at a time with "Show more".
- The home lists keep their `limit` and a search keeps its "first n and
  whether there are more": neither is a list to page through.

## Alternatives considered

- **Cursors.** Right for a feed that is only ever read forward. A library
  is jumped through, turned around and shuffled.
- **Everything whole, and the client windows it.** The web app could draw
  only what is on screen, but it would still download and keep every
  series of a large library to show the first forty, and so would a phone.
