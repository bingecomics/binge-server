# 0009. First-run setup makes the owner; members read what they are given

## Status

Accepted on 2026-11-16. Built in iteration 3. Supersedes
[0008](0008-one-reader-until-accounts-exist.md).

## Context

Until iteration 3 the server had no accounts: it listened on its own machine
only, and everyone who opened it was the same reader. To be reached from an
iPad or by a second person it needs to know who is asking.

The plan fixes what accounts are for: an owner made at first-run setup, more
users with access to some libraries, and progress that is each reader's own.
It leaves open how the owner comes to exist, what the other users may do, and
how progress saved before accounts reaches the owner. The server must stay
"download, run, open the browser": no configuration file to write first, no
second service.

## Decision

**The owner is made in the browser, by whoever opens a new server first.**

- A server without an owner shows one page: name, password, create. `POST
  /setup` makes the owner and signs them in. It works once; after that it
  answers 409.
- `GET /health` says `needs_setup` while there is no owner, so a client can
  tell a new server from one it is not signed in to.
- The server still listens on `127.0.0.1` unless told otherwise. On the
  machine it runs on, the first to open it is the person who started it.
  Started with `--bind` on a network address before setup, it logs a warning
  that whoever opens it first becomes the owner.

**There is one owner, and members.**

- The owner reads every library and manages the server: libraries, scans,
  users.
- A member reads the libraries the owner gave them, listed one by one, and
  manages nothing but their own password and app tokens. A library added
  later is not theirs until the owner says so.
- A library a member may not read is, for them, not there: it is missing from
  lists, and its series, books, covers and pages answer 404, the same as an
  id that never existed. The rule is one SQL condition (`may_read` in
  `crates/core/src/catalog.rs`) that every query answering a person goes
  through.
- Only the owner is told where a library is on the server's disk.
- The owner makes each member's account and tells them the password. There
  is no sign-up page and no invitation by email: the server sends no mail.

**Progress is keyed by reader and book identity.**

- Migration 0004 makes a new `progress` table with the reader in its key, and
  renames the old one to `progress_before_accounts`. The migration runs when
  the server starts, before any owner exists, so the old rows wait there.
- Setup moves them to the owner in the same transaction that makes the
  owner, and empties the waiting table.
- Removing a user removes their progress, sessions and tokens. Removing a
  library still removes nobody's progress, as 0008 decided.

How a request shows who makes it is in
[0010](0010-sessions-for-browsers-tokens-for-apps.md).

## Consequences

- Nothing can be read or changed without signing in, except `/health`,
  `/setup` and `/sign-in`. Listening on the network is now a reasonable
  thing to turn on.
- A server started on a network address and left without setup can be
  claimed by anyone who reaches it. The window is from the first start to
  the setup, the log says so, and the remedy is to delete the data
  directory and start again. A NAS or a container, where the browser is
  always another machine, is the case where this window cannot be avoided.
- There is no way back into a server whose owner forgot the password, short
  of editing or deleting the database. A command that sets a password from
  the machine the server runs on is the natural fix and is not built.
- The owner cannot be removed, renamed or replaced, and there is no second
  administrator.
- The same book in two libraries is one identity. A member who may read one
  of the two shares their progress across both if they are later given the
  other, and is served pages only from a copy in a library they may read.
- Covers and pages are cached by the browser for a year. Someone who loses
  access to a library can still see the images their browser already holds.
- The API of `/api/v1` changed shape: `Library.path` may be missing, `Health`
  gained `needs_setup`, and progress is the caller's. This is the last
  iteration where that is allowed.

## Alternatives considered

- **Take the owner's name and password from options or the environment.**
  It closes the claiming window, and it puts a password in a command line or
  an environment, which the project forbids, and adds a step before "open
  the browser".
- **A setup code printed in the log.** Also closes the window. It is a
  secret in a log, and a step most people would meet on their own machine,
  where it protects nothing.
- **Allow setup from this machine only.** Right for a laptop and wrong for a
  NAS or a container, where the browser is never on the same machine.
- **Listen on the network by default.** It is what most people will want in
  the end. It would also open the claiming window on every first run, so it
  stays a choice the owner makes with `--bind`.
- **Give a member every library unless told otherwise.** Less to click in a
  household. It would hand a library added later to people nobody thought
  about, so access is a list, and an "all libraries" switch can be added on
  top of the list later.
- **Roles beyond owner and member** (administrators, read-only guests).
  Nothing needs them yet.
