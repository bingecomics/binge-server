# 0018. What a person chooses is kept as keys and values

## Status

Accepted on 2026-11-18, decided with step W6 of the plan and built the
same day.

## Context

The web app lets a person choose things that are about the app, not about
the library: the size of the covers, which rows Home has and in what
order. Until W6 they were kept in the browser, so they were lost in
another browser and on a phone. W6 asks for the server to keep them, so
that they follow the person.

The readers that connect later (iOS first, then Android) will have
choices of their own, and they will not be the web app's: a phone has no
cover slider. What the server keeps has to serve clients it has not seen.

## Decision

The server keeps, for each person, values under keys, and does not read
them.

- `GET /me/settings` answers with everything the caller chose,
  `PUT /me/settings/{key}` keeps a value and `DELETE` forgets it. A person
  reads and writes only their own, with a session or with a token.
- A key is 1 to 64 lower-case letters, digits, dots, dashes or
  underscores. A value is text of 8 KB at most, and a person keeps 100
  keys at most. The limits are there so that the table cannot be used to
  store something else.
- The server gives a value no meaning. The client that wrote it reads it,
  and treats a value it cannot read as if there were none. A client names
  its keys so that they do not meet another client's: the web app uses
  `cover-size` and `home.rows`.
- What a person chose goes when the person is removed.

What the reader does to a book is not a setting. Progress, read marks and
"Up next" are kept by the routes of reading, by the identity of the book,
and the server gives them meaning.

## Consequences

- A new choice in a client needs no migration, no route and no release of
  the server.
- The server cannot check a value, so it cannot repair one. A client has
  to be written to survive a value an older or newer version of itself
  wrote. The web app does: an arrangement of Home that names a row that
  no longer exists leaves it out, and a row it does not name goes last.
- Two clients that want the same choice have to agree on a key and on
  what its value means, outside the server. That is by design: the
  agreement belongs in the clients, which are both Binge's.
- Home's arrangement is the one place where a person's choice and the
  server's views meet
  ([0017](0017-a-view-is-a-library-narrowed-and-kept-by-name.md)): the
  server says which views are rows, and the person's `home.rows` says in
  what order and which are hidden.

## Alternatives considered

- **A column for each choice.** The server could then check each value.
  Every choice would be a migration and a change to the API, and after
  `/api/v1` freezes, a choice a new reader wants would wait for a server
  release.
- **Keep them in the browser.** What was there before, and still where
  the web reader's own choices are (fit, pages side by side, direction)
  until the reader is done in W5.
