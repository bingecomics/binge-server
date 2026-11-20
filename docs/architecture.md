# Architecture

This describes the server as it is built today, after iteration 4, and names
the parts that are planned. [PLAN.md](../PLAN.md) has the iterations; the
[decision records](adr/README.md) have the reasons.

## What it is

Binge Server is a program a person runs at home, on a computer or a NAS, and
points at their comic folders. It serves those comics to Binge's own clients:
the web UI in this repository, and the iOS and Android readers. Later it also
serves them to the user's AI client through MCP.

It is one binary. The web UI is embedded in it, and all its state is one
SQLite file in a data directory.

## The parts

```
                    ┌──────────────────────── binge-server (one binary) ─────────────────────────┐
                    │                                                                            │
 Browser ───────────┤  Host check ──┬── /api ──► API routes ──► binge-core ──► data directory    │
 iOS, Android       │               │            (sign-in check, JSON)     │      (SQLite, covers)│
 (later)            │               │                                      ▼                     │
                    │               └── other ─► embedded web UI       library folders           │
                    │                            (files built by Vite) (read-only)               │
                    └────────────────────────────────────────────────────────────────────────────┘
```

| Part | Where | Role |
| --- | --- | --- |
| `binge-core` | `crates/core` | Domain logic and storage: the catalog of libraries, series and books, the scanner, archives, covers, pages, metadata, search, reading progress, and accounts. No HTTP |
| `binge-server` | `crates/server` | The HTTP API, the embedded UI, configuration, and the binary |
| Web UI | `web` | React app: Vite, TanStack Router and Query, Tailwind. Setup and signing in, browsing, search, the reader, and settings |
| API contract | `api/openapi.json` | Generated description of `/api/v1`, shared by every client |
| Schema | `migrations` | SQL applied at startup |
| Scripts | `scripts` | Build, development, API generation, checks |

`binge-core` has no HTTP in it on purpose. The MCP server planned for
iteration 6 will be a third crate on top of the same core, next to
`binge-server`.

### Inside `binge-server`

| Module | Role |
| --- | --- |
| `main.rs` | Reads the configuration, hands over to a newer version kept in the data directory when there is one, opens the database, listens, copies the database when a copy is due, shuts down on a signal or to start again as a newer version |
| `config.rs` | Command-line options and their environment variables |
| `app.rs` | Puts the application together: API, UI, Host check, request tracing |
| `api.rs` | Mounts the routes under `/api` and builds the OpenAPI description |
| `api/error.rs` | The one error shape, and how core errors turn into it |
| `api/auth.rs` | The check that a request shows who makes it, and the caller the handlers get from it |
| `api/health.rs`, `api/libraries.rs`, `api/browse.rs`, `api/reading.rs` | The routes for libraries and reading, grouped by what they are about |
| `api/people.rs` | The routes for the people books name, and for the owner to say who is who |
| `api/views.rs`, `api/settings.rs` | The routes for the views kept of a library, and for what each person chose in an app |
| `api/history.rs` | The routes for what the caller read, and for the owner to see who reads now |
| `api/kept.rs` | The routes for the pages the caller kept while reading |
| `api/scans.rs`, `api/backups.rs` | The owner's routes for what scans changed and files set aside, and for the copies of the database |
| `api/updates.rs` | The owner's routes for the server's own updates |
| `update.rs` | The server's own updates: looking for a newer version, fetching it, and asking for the start that runs it |
| `update/release.rs` | What a release says of itself, and the check that one of the keys built in signed it |
| `update/store.rs` | The versions kept in `updates/` in the data directory, each proven again before it is started |
| `update/fetch.rs` | The requests to where releases are published: the only place the server calls out |
| `update/handover.rs` | Starting another binary of the server in the place of the one that runs |
| `api/layout.rs` | The owner's routes that change series, groups and where books sit |
| `api/session.rs`, `api/account.rs`, `api/users.rs` | The routes for setup and signing in, for one's own password and tokens, and for the owner to manage users |
| `host.rs` | Refuses requests addressed to a host name the server does not know |
| `ui.rs` | Serves the embedded web UI |
| `build.rs` | At compile time, stages the built UI so it can be embedded |

### Inside `binge-core`

| Module | Role |
| --- | --- |
| `db.rs` | Opens SQLite and runs the migrations |
| `catalog.rs` | Libraries, series and books: adding, removing, listing |
| `import.rs` | Where a book the server sees for the first time is put |
| `layout.rs` | Changing series, groups and where books sit, only as asked |
| `scan.rs` | The scanner and the state of each library's scan |
| `reading.rs` | One book and its pages, page images, reading progress, and the lists of the home page |
| `accounts.rs` | Users, passwords, sessions, API tokens, and which libraries a user may read |
| `archive.rs` | Reads a comic file: its pages in order, their sizes, the images themselves, the book's identity. `archive/zip.rs`, `archive/rar.rs` and `archive/pdf.rs` each read one format |
| `comicinfo.rs` | Reads what a file says about itself in `ComicInfo.xml` |
| `search.rs` | Finds series, books, reading lists, collections and people by their words |
| `narrow.rs` | A library's series in an order, narrowed by what their books say, a part at a time |
| `people.rs` | The people books name, the names they go by, and making two of them one |
| `views.rs` | A library narrowed and ordered, kept under a name for Home and the menu |
| `settings.rs` | What each person chose in an app, as keys and values the server does not read |
| `history.rs` | What someone read, by day, month and writer, who is reading now, and how fast someone reads |
| `kept.rs` | The pages a reader kept while reading |
| `changes.rs` | What each scan changed: the files that arrived, moved, went and came back |
| `backup.rs` | Copies of the database in the data directory, made when asked and once a day |
| `thumbnail.rs` | Makes and stores cover images, and makes a page small |
| `covers.rs` | The covers people chose for a book, a series or a reading list |
| `lists.rs` | Reading lists: books of any series in a chosen order |
| `collections.rs` | Collections: series picked by hand, from any library |
| `sharing.rs` | What reading lists and collections share: whose a thing is, and who it is shared with |
| `cbl.rs` | Reads a CBL file and finds the book each of its entries means |
| `natural.rs` | Name order where "Issue 2" comes before "Issue 10", and a name with an accent sits with its letter |
| `fixtures.rs` | Makes small comics for tests. Compiled for tests only |

## How a request is answered

1. **Host check.** A request that names a host other than an IP address,
   `localhost`, or a name given with `--allowed-host` gets a 403. This blocks
   a web page from reaching the server under its own host name (DNS
   rebinding). See [ADR 0007](adr/0007-refuse-unknown-host-names.md).
2. **`/api` and everything below it** goes to the API. The API module owns
   that whole space: known routes are handled, anything else is a JSON 404.
   Every error, including the ones the web framework produces by itself (a
   wrong method, a body that does not parse), leaves as an `ApiError` with a
   code from a fixed list. Every route except `/health`, `/setup` and
   `/sign-in` is behind the check described under
   [Accounts and signing in](#accounts-and-signing-in).
3. **Everything else** goes to the UI:
   - a file that exists is served; files under `assets/` have a content hash
     in their name and are cached for a year, everything else is revalidated
     on each load;
   - a path that can only be a file (under `assets/`, or with an extension)
     and does not exist is a 404;
   - any other path gets the app's entry page, because the client-side router
     owns those URLs. This is why reloading `/settings` works.

## The API contract

The Rust handlers are the source. Everything else is generated from them:

```
handlers in api/   ──► api/openapi.json ──► web/src/api/schema.d.ts ──► typed client in the web app
                              │
                              └──► Swift and Kotlin clients (when the readers connect)
```

`scripts/gen-api.sh` regenerates both files, and `scripts/check.sh` fails when
either is out of date. The server also serves the description at
`/api/v1/openapi.json`. See
[ADR 0006](adr/0006-one-api-contract-generated-from-code.md).

The routes today:

| Route | What it does |
| --- | --- |
| `GET /health` | The server's name, version and build (commit and time), whether the database answers, and whether it still needs its owner. Open |
| `POST /setup` | First-run setup: make the owner and sign them in. Open, and works once |
| `POST /sign-in`, `POST /sign-out` | Start a session with a name and password (open); end it |
| `GET /me` | Who makes the request: name, role, libraries |
| `PUT /me/password` | Change one's own password. Session only |
| `GET /tokens`, `POST /tokens`, `DELETE /tokens/{id}` | One's own API tokens: list, make, revoke. Session only |
| `GET /users`, `POST /users`, `PATCH /users/{id}`, `DELETE /users/{id}` | The owner manages the other users and what they may read. Session only |
| `GET /libraries`, `POST /libraries` | List the libraries the caller may read, with the state of their scan; the owner adds one by folder path |
| `GET /libraries/{id}`, `DELETE /libraries/{id}` | One library; the owner removes it from the server (files are not touched) |
| `POST /libraries/{id}/scan` | The owner starts a scan. Answers at once; the scan runs in the background |
| `GET /libraries/{id}/series` | A part of the series of a library, in the order asked for and narrowed by read state, publisher, year, writer and penciller |
| `GET /libraries/{id}/series/letters` | The first letters the names of those series have, each with how many and where its first series is |
| `GET /libraries/{id}/choices` | What a library's series can be narrowed by: its publishers, decades, writers and pencillers |
| `GET /libraries/{id}/duplicates` | For the owner: the books the library holds in more than one file |
| `GET /libraries/{id}/unreadable` | The files of a library that could not be read, each with the reason. They are in no series. With `set_aside=true`, for the owner, the ones set aside instead |
| `PUT /books/{id}/set-aside`, `DELETE` | The owner sets a file that could not be read aside, so that it is no longer listed or counted; takes it back |
| `GET /libraries/{id}/scans` | For the owner: the scans that changed what the library holds, the latest first, each with how many files arrived, came back, moved and went |
| `GET /scans/{id}/changes` | For the owner: a part of the files one scan found changed, all or of one `kind`, each with the book it is now when the library still holds it |
| `POST /libraries/{id}/series` | The owner adds a series with no books |
| `GET /series/{id}`, `GET /series/{id}/books` | One series; its books |
| `PATCH /series/{id}`, `DELETE /series/{id}` | The owner renames a series; deletes one that holds no books |
| `POST /series/{id}/groups` | The owner adds a group with no books to a series |
| `PATCH /groups/{id}`, `DELETE /groups/{id}` | The owner gives a group another number or name; deletes one that holds no books |
| `PUT /books/{id}/place` | The owner puts a book in a series, a group and at a number |
| `GET /series/{id}/overview` | One series seen whole: what its books say about it together, how far the caller is through it, and the book to open next |
| `GET /groups/{id}/overview`, `GET /groups/{id}/books` | One group of a series seen whole, as a series is: its series, what its books say about it together, how far the caller is through it, and the book to open next; its books |
| `GET /covers/{cover}` | A cover image |
| `PUT /books/{id}/cover`, `PUT /series/{id}/cover`, `PUT /reading-lists/{id}/cover` | The owner makes a page of a book the cover of a book; of a series; of a reading list of their own |
| `PUT /books/{id}/cover/picture`, and the same for `/series` and `/reading-lists` | The owner gives a picture as the cover: the body is the picture's file |
| `DELETE /books/{id}/cover`, and the same for `/series` and `/reading-lists` | The owner takes the chosen cover back |
| `GET /books/{id}` | One book, with how far it has been read |
| `GET /books/{id}/pages` | The book's pages in order, with their sizes |
| `PUT /books/{id}/progress` | Save how far the book has been read |
| `POST /read-marks` | Mark books as read or as unread: the books named, and every book of the groups, series and reading lists named |
| `GET /pages/{identity}/{number}` | A page image, as the file stores it |
| `GET /continue-reading`, `GET /up-next` | Books started and not finished; the book that follows a finished one |
| `DELETE /up-next/{series_id}` | Take a series off the caller's `/up-next` until they read from it again |
| `GET /recently-updated`, `GET /recently-added` | The series that got a book last; the newest books |
| `GET /me/reading?year=` | What the caller read in a year: the days they read on, each month, the writers they read most, and the first year there is anything of |
| `GET /reading-now` | For the owner: who has turned a page in the last half hour, and of which book |
| `GET /views`, `POST /views` | The views of the libraries the caller may read; the owner keeps one: a library, an order and a narrowing under a name |
| `PATCH /views/{id}`, `DELETE /views/{id}` | The owner changes a view's name, what it asks or where it shows; deletes it |
| `GET /me/settings` | Everything the caller chose in an app, by key |
| `PUT /me/settings/{key}`, `DELETE` | Keep a value under a key; forget it |
| `GET /server/backups`, `POST`, `PATCH` | For the owner: the copies of the database and whether one is made every day; make one now; turn the daily copy on or off |
| `GET /server/update`, `PATCH`, `POST`, `POST /server/update/look` | For the owner: the version that runs, the newer one the last look found and how far an update is; turn the daily look on or off; bring the server to the newer version, named in the request; look now |
| `GET /search?q=` | The series, books, reading lists, collections and people that hold every word of `q` |
| `GET /people` | A part of the people the books name, the ones most books name first; with `q`, those a name of whom holds its words |
| `GET /people/{id}` | One person the books name, with every name they go by |
| `GET /people/{id}/series`, `GET /people/{id}/books` | The series that hold a book that names them; a part of those books |
| `GET /books/{id}/credits` | The people a book names as writers and pencillers |
| `PATCH /people/{id}` | The owner shows a person by another of their names |
| `POST /people/{id}/merge` | The owner makes other people one with this one |
| `POST /people/{id}/names`, `DELETE /people/{id}/names/{name}` | The owner gives a person one more name; makes a name a person of its own again |
| `GET /reading-lists`, `POST /reading-lists` | The reading lists the caller sees: their own and the ones shared with them; anyone makes one of their own, with books in it or none |
| `GET /reading-lists/{id}`, `PATCH`, `DELETE` | One list seen whole, as a series is; the one whose list it is gives it another name and summary; deletes it |
| `PUT /reading-lists/{id}/sharing` | The one whose list it is says who sees it: everyone, the people named, or nobody else |
| `GET /reading-lists/{id}/books`, `POST` | Its books in its order; the one whose list it is adds books, groups, series or other lists at its end |
| `DELETE /reading-lists/{id}/books/{book_id}` | The one whose list it is takes a book out of it |
| `PUT /reading-lists/{id}/books/{book_id}/position` | The one whose list it is moves a book of it to a position among the books shown |
| `GET /books/{id}/reading-lists`, `GET /series/{id}/reading-lists` | The reading lists the caller sees that a book is in; that hold a book of a series |
| `GET /collections`, `POST /collections` | The collections the caller sees: their own and the ones shared with them; anyone makes one of their own, with series in it or none |
| `GET /collections/{id}`, `PATCH`, `DELETE` | One collection; the one whose it is gives it another name and summary; deletes it |
| `PUT /collections/{id}/sharing` | The one whose collection it is says who sees it: everyone, the people named, or nobody else |
| `GET /collections/{id}/series`, `POST` | Its series by name; the one whose collection it is adds series |
| `DELETE /collections/{id}/series/{series_id}` | The one whose collection it is takes a series out of it |
| `GET /series/{id}/collections` | The collections the caller sees that a series is in |
| `POST /reading-lists/import` | Asks what a CBL file would make: the body is the file. Nothing is made |
| `POST /reading-lists/import/rules`, `DELETE /reading-lists/import/rules/{id}` | Keeps what the caller says an entry of a CBL file means, for their imports from now on; forgets it |
| `GET /users/names` | Everyone on the server by name and id, for any caller: who something can be shared with |

Lists that grow with the library are read a part at a time, by `offset`
and `limit` (100 when left out, 500 at most), and answer with the `total`:
the series of a library and the books that name a person. The rest are
returned whole. See
[ADR 0015](adr/0015-long-lists-are-read-a-part-at-a-time.md). The four
home lists take a `limit`, and a search answers with at most 50 of each
kind and says when there are more. Every book the
API returns carries the caller's `progress`, and `info`, what its file says
about itself, when the file says anything. Every series and group carries
`unread_count`, how many of its books the caller has not read to the end,
and so does every reading list. A library's folder path is in
the answer for the owner only.

## Accounts and signing in

The decisions are in [ADR 0009](adr/0009-first-run-setup-makes-the-owner.md)
and [ADR 0010](adr/0010-sessions-for-browsers-tokens-for-apps.md).

**People.** A new server has no users. The first person to open it makes the
owner, in the browser; `/health` says `needs_setup` until then. There is one
owner, who reads every library, adds and removes libraries, and manages the
other users. Everyone else is a member, who reads the libraries the owner
ticked for them. To a member, a library they may not read does not exist:
its series, books, covers and pages are answered 404, and it is absent from
every list. That is one SQL condition in `catalog.rs`, used by every query
that reads the catalog for someone.

**Showing who you are.** A browser signs in with a name and password and
gets a session cookie. An app sends an API token as
`Authorization: Bearer`. A person makes tokens under Settings, one per app,
sees each secret once, and revokes them there. The database holds an
Argon2id hash of each password and a BLAKE3 hash of each session's and
token's secret, never the values.

**The check.** The API's routes are in three rings, put together in
`api.rs`:

| Ring | Routes | What a request needs |
| --- | --- | --- |
| Open | `/health`, `/setup`, `/sign-in` | Nothing |
| Signed in | Everything else | A session or a token that is still good |
| In person | `/me/password`, `/tokens`, `/users` | A session. A token is refused with 403, so one that leaks cannot change the account or make itself a successor |

The check is a layer around the routes, not a line in each handler, so a
new route is inside it unless it is put with the open ones on purpose. The
ring also writes what it asks into the API description (nothing, a session
or a token, a session only), so the contract and the check cannot say
different things. A test walks every route of the description and expects
of each what the description says: 401 without a sign-in, 403 for a token
on the in-person routes. Owner-only routes ask for the `Owner` in their
arguments and answer 403 to anyone else.

Around all of it, a request that changes something and relies on the cookie
is refused when the browser says a page of another origin made it. Signing
in is slowed for everyone at once: four password checks at a time, a second
of waiting after a wrong one, and 429 beyond that. A check keeps its place
until it is over, also when the client hangs up.

**In the web UI.** The root route asks `/health` and `/me` before showing
anything: a server without an owner shows the setup page, a visitor who is
not signed in shows the sign-in page, and otherwise the app appears. The
address stays as it was, so signing in lands where the link pointed. A 401
from any later request brings the sign-in page back.

**The frame around every page** but the reader's is a bar at the top and a
menu beside the page. The bar has the name, which leads Home, the search
box, what the server is doing, the settings, and who is signed in: that
last one opens what the person read, their account, their tokens and the
way out. What the server is doing is an icon that opens a short list:
each library with the state of its scan, for the owner how many of its
files could not be read, with the way to them, and who is reading now.
The icon carries a dot while a scan runs or a file waits to be looked
at. What is typed in the box leads to
`/search?q=…`, which shows what was found; Ctrl+K or ⌘K leads to the box
from anywhere. On every page but the settings', a window at least 1024 px
wide also has a slider there for the size of the covers: one size for every grid
and row, from 6 to 13 rem, kept by the server for each person
(`cover-size`, see "What a person chose" below). The menu lists
Home, the reading lists, the collections, the kept pages, the views kept
for the menu and the libraries. On a settings page it is the settings' own menu
instead, a page for each part: `/settings/account`,
`/settings/tokens`, and for the owner `/settings/users`,
`/settings/libraries`, `/settings/views` and `/settings/server`. A narrow
window has no room beside the page: there the settings' menu is a row
above it, the reading lists, the views of the menu and the libraries
are reached from Home, and the collections from the menu of who is signed
in.

**Menus and picking several.** Every card of a book, a group, a series or
a reading list has a menu in the corner of its cover, and so has the page of each, of a
library too: what can be done with the thing is there, and not on buttons
over the cover. All menus are one component (`components/Menu.tsx`), drawn
with the browser's `popover` so that a menu shows whole over a row that
scrolls. A card also has a tick. Ticking one card starts picking: from then
on a click on a cover picks instead of opening, a click with Shift picks
every card of the same list from the one picked last, and a bar at the
bottom of the window says how many are picked and what can be done to all
of them. Its "Select all" takes the list the last card was picked in, since
a page can hold several lists of different kinds. On a touch screen a
finger resting on a card starts it. What is picked is kept outside
the components (`selection.ts`), so picking one card draws that card again
and not the page, and it is let go when the page changes. What goes wrong
with something done from a menu or the bar is said over the page for a few
seconds (`problems.ts`).

## Libraries and scanning

A library is a folder, and each `.cbz`, `.cbr` or `.pdf` file in it or in
the folders inside it is a book. Which series a book is in, and where in
it, is not read from the folders: it is the server's own record, described
under "Series, groups and places" below.

A scan runs in the background, one per library, in two passes:

1. **Finding.** Walk the folder, in the order of the names in it and not
   in the order the file system lists them, so that a library gives the
   same catalog on every machine. A file whose size and modification time
   match what the database has is skipped. For the others, read the archive's
   index (no image is decompressed, a few files at a time) to get the pages
   in order and the book's identity, read its `ComicInfo.xml` if it has one,
   and store them. A book the library has not seen before is given its
   place. Then remove the books whose files are gone.
2. **Covers and sizes.** For each book whose pages are new or changed, read
   the width and height of every page, usually from the first bytes of the
   image, and make a cover from the first page. A few books at a time, over
   all libraries together. A file that changed around the same pages, as
   when a tool adds metadata to it, keeps its sizes and cover.

A scan starts when a library is added, when the server starts, and when asked
for. Its state (phase, files found, books done) is kept in memory and is part
of every library the API returns; the web UI asks again every second while a
scan runs, and refreshes the series and books on screen each time.

What the scanner is careful about:

- **It only reads.** Nothing in a library folder is written, renamed or
  deleted. See [ADR 0005](adr/0005-library-folders-are-read-only.md).
- **A folder that cannot be read removes nothing.** If the library folder is
  missing (a drive not connected) the scan fails and the catalog stays as it
  was. If only part of it could not be read, books missing from the scan are
  kept and the scan says so.
- **A file that cannot be read is still listed**, with what is wrong with it,
  apart from the series: it has no identity to keep a place by.
- **Identity is the content.** A book's identity is a hash of its page
  images' checksums and sizes, so it is the same after a move or rename. See
  [ADR 0004](adr/0004-identify-books-by-page-content.md). A moved file is
  a removal and an addition to the catalog, and gets the same identity;
  the record of the scan says it moved.

**What a scan changed** is written down when the finding pass ends, by
`crates/core/src/changes.rs`, for the owner to look back on
(`GET /libraries/{id}/scans`). A scan that found everything as it was
leaves no record, and the latest 50 of a library are kept. Each file that
changed is one of four kinds:

- `moved`: a file arrived with the pages of one that went in the same
  scan. Of several that arrive with those pages, the one that kept the
  file's name is the one that moved, and the others are copies.
- `back`: a file arrived whose book the library held before and has no
  other file of now.
- `arrived`: any other file that arrived, a copy of a book that is here
  included, and a file that could not be read.
- `gone`: a file that went and did not arrive elsewhere.

A change keeps the path and the identity of the book, not the book: the
book it is today is looked up when the changes are read, so a file that
went and came back two scans later says so. The first scan of a library
records everything as arrived.

**A file that could not be read** can be set aside by the owner once seen
(`PUT /books/{id}/set-aside`). It then leaves the list and the library's
`unreadable_count`, which is what the icon in the bar at the top counts,
and is counted in its `set_aside_count`. A file that changes and is read
again is listed again if it still cannot be read. Both counts, and the
list of the files set aside, are the owner's: the server leaves the
counts out for anyone else and refuses them the list.

### Formats

The server reads three kinds of file. Which one a file is comes from its
first bytes, not from its name: a `.cbr` that is really a zip is read as a
zip. The name only decides when the first bytes say nothing. The reasons
for the libraries are in
[ADR 0011](adr/0011-read-cbr-and-pdf-with-rust-libraries.md).

| Format | Read with | Pages | A page's checksum and size |
| --- | --- | --- | --- |
| CBZ (zip) | `zip` | The image files, in natural name order | Stored in the archive's index |
| CBR (rar) | `rars` | The same | Stored in the archive's headers; computed by unpacking in the oldest archives, which do not store one |
| PDF | `hayro` | Every page of the document, in order | Computed, over the picture a page is or over what the page draws |

A PDF page that is nothing but one JPEG picture, upright and in its own
proportions, is a scan: the server hands out that JPEG as it is in the
file. Any other page is drawn on white, 2200 pixels on its longer side, and
sent as a new JPEG. A PDF is read into memory whole, since it cannot be
understood in part.

The identity rule is the same for all three, so the same pictures give the
same book whether they are in a CBZ, a CBR or a scanned PDF.

A file protected with a password is listed as a book that cannot be read.
So is a file that makes one of the readers crash: the crash is caught where
files are read, and ends neither the scan nor the request.

For a PDF page that is drawn, the `byte_size` in the page list is the size
of what the page is made from, not of the picture sent.

### What a file says about itself

A CBZ or CBR may hold a `ComicInfo.xml`, written by tagging tools. The scan
reads it and keeps title, series, number, volume, year, month, day, writer,
penciller, publisher, summary and language in `book_info`, one row per
file. It belongs to the file, not to the identity: two files with the same
pages may say different things, and retagging a file changes what is kept
at the next scan without touching its pages, cover or progress.

The API gives it as a book's `info`. It is a copy that only follows the
file: nobody edits it. The server reads it once more, to place a book it has
not seen before, and never again for that. The web UI shows it in two
places:

- There is one card (`components/Card.tsx`) for a book, a series, a group
  and a reading list, on every page: a cover, the name on two lines at most, one line
  under it that tells it from its neighbours, and a last line that sits at
  the foot of the card, so a row of cards ends on one line whatever their
  names take. How far a book was read is a bar on the cover's lower edge,
  the same place on every card.
- A book is called by its series and number, as comics are: "The Flash
  #12", with its title under that. The number is the place's and not the
  tag's, and the series' name comes with the book (`series_name`). On the
  page of its own series the series is already said, so the number stands
  alone: "#12". A book with no number goes by its title, with its series
  under it, or the year where the series is already said. With no title
  either it goes by its file's name, said as a reader would: without what
  is in brackets, the zeros that pad its number, or the `SP05` some
  libraries put after the number to mark a special. The name as the file
  has it is never what a card, the reader or a book's page calls a book: it
  is among the facts on the book's page, and it is all a file that could
  not be read has. That is only how the name is shown: what a file's name
  means for where the book sits is read by the server alone. The title is
  always on the card, never behind a hover, which a phone does not have.
- A book's own page, `/books/{id}`, says everything the file gives, next to
  the button that opens the reader.

### Series, groups and places

Which series a book is in is the server's own record, kept apart from the
folders and from the tags
([ADR 0012](adr/0012-series-and-groups-are-a-layout-over-the-files.md)).

- A **series** is the books that go by one name. Names are compared by
  their letters and digits, without case, accents, punctuation or spaces,
  and a library holds each name once.
- A **group** is a part of a series: a volume, a limited series, the
  annuals, the collected editions, the one-shots, or the appearances in
  other titles. Import makes volumes, annuals, collected editions and
  one-shots; limited series and appearances are made through the API. A
  book may also sit directly in its series. Every kind of group is the same
  thing with another name: it may have a number, a name, both or neither,
  a series may hold several of a kind, and each has the same page. Import
  finds a numbered group by its number, whatever name the owner gave it
  since, and with no number uses the group that has no name.
- A **place** is a book's series, its group and its issue number, with the
  date it was published. It is kept by the book's identity in `book_place`
  and nowhere else: lists join each file to its place by library and
  identity, and changing a place is one write.

`crates/core/src/import.rs` gives a book its place the first time a library
sees it. Each part comes from the tag, else from the file's name, else from
the folders:

- The name is read the way comic files are commonly named:
  `Teen Titans v1 001 (1966-02)` is series "Teen
  Titans", volume 1, number 1. What is in brackets is left out. `SP01`
  marks a special.
- A name that gives no number and no volume says nothing about its series,
  so the book takes its folder's name. A folder named only after a volume
  (`Volume 2`) gives the volume, and the folder above it the series. A file
  in the library's own folder is a series of its own.
- A book is a special, no issue of a run, when its name carries the
  marker, when its `Format` tag names one (annual, one-shot, TPB and the
  like), or when it has neither a number nor a volume. The
  `Format` tag says which group a special goes in: the annuals, of its
  volume when it gives one, or the collected editions. Any other special
  goes in the one-shots. No word in a name is taken for it, and import
  never calls a book an appearance in another title: that is the owner's
  to say. A book that is no special goes in its volume when it has one,
  and otherwise directly in the series.
- Import gives a place only what the owner's routes would take: a tag's
  number or series name that is too long is passed over for the file's
  name, and a day its month does not have is dropped from the date. A
  place read from the API can always be written back.

After that the file is not asked again. A book that was moved, renamed or
retagged is found by its identity and sits where it did. A series whose
files are all gone stays, listed with no books, and keeps its books'
places, so they return to them. A file that cannot be read has no identity
to keep a place by, so it is in no series: the library lists such files
apart, each with what is wrong with it, until a scan can read them.

After import, the layout changes only through the owner's routes in
`api/layout.rs`, which call `crates/core/src/layout.rs`
([ADR 0013](adr/0013-the-layout-changes-only-as-asked.md)). Each does
exactly what was asked or refuses with the reason:

- A series or group is created, renamed or deleted. A name another series
  of the library has is refused, and so is a group the series already has.
  Nothing is merged. The answer is a 409 whose `conflicts_with` is the id
  of the one in the way.
- A book's place is set whole: series, group, number and date published.
  The series and group are named by id and must be there; what the request
  leaves out, the book does not have.
- Only a series or group that holds no books is deleted, and nothing is
  deleted for being empty. An empty one is listed like any other.

Books are listed in reading order: the volumes by number, then the limited
series, the books in no group, the annuals, the collected editions, the
one-shots, and the appearances in other titles. Inside a group, whatever
its kind, they go by issue number, and the books without one after them by
the date they were published, the undated ones last. File name order
settles the rest.

In the web app a series page shows its parts as tabs when it has several:
volumes, limited series, the issues in no group, annuals, collected
editions, one-shots and appearances, each with how many it holds. Every
kind is shown the same way. A tab with several groups shows them as cards,
and a card opens the group's own page at `/series/{id}/groups/{id}`, with
its books and how far the reader is through them. A tab that is one group
shows that group's books at once, under the group's name or number when it
has one, since a card would only repeat the tab's name. So do the books of the group with no name and no number, under the
cards of the named ones: they are the rest of the kind. A series with one
part has no tabs.
What a group page shows comes from the server, as a series page's does:
see below.

### A series as a whole

No file describes a series, so `crates/core/src/overview.rs` puts one
together from the series' books, for one reader. `GET /series/{id}/overview`
gives it:

- From what the files say: the publisher most books name, the earliest and
  latest year, the writers and pencillers with the most named first, and a
  summary taken from the first book in reading order that has one, with
  that book. The series' groups come with it.
- From the reader's progress, as `read`: books read to the end out of those
  that can be read, pages read out of all pages, and `next`, the book to
  open to go on. `next` follows what the
  reader did last: a book left unfinished is the one; after a finished book
  it is the first unfinished book behind it, or at the end of the series the
  first unfinished one anywhere; someone who read nothing, or everything,
  starts at the first book in reading order. Files that cannot be read are
  passed over. And `seconds_left`, how long the pages not yet read will
  take at the reader's own pace, once that is known: see "Sittings" below.

A group is seen whole the same way. `GET /groups/{id}/overview` gives the
group, its series, and everything above worked out by the same code over
the group's books alone: the publisher, years, credits and summary are
those of that group's books, and `next` follows what the reader did last in
that group, whatever they read elsewhere. So a volume goes on from where it
was left, while its series may point at another volume. The web app shows a
group's page with the same header as a series' page.
`GET /groups/{id}/books` lists the group's books in reading order. Each
group also names its `cover`: that of its first book, in reading order,
that has one.

One book stands for the series: the first in reading order, which is the
first one published as far as the places say. Its cover is the series'
cover, it is where an unread series starts, and the summary is its own when
it has one. A one-shot or an appearance stands for a series only when the
series has nothing else.

This lives in the core, not in the web UI, so a reader app or an MCP tool
answers "what do I read next" the same way. The time left is measured
here too. One estimate is still made in the web UI: how long pages take
at 20 seconds a page, which it shows for a book or a series as a whole,
and for the time left until the reader's pace is known.

**What comes after a book** is answered here as well, for a reader to go
on at a book's last page. `GET /books/{id}/next` follows where the book
was opened from: the reading list named by `list`; with `group`, the
book's group of its series, which ends with the group; and otherwise its
series, on from one group to the next. It answers with what is `followed`
(`list`, `group` or `series`) and the `next` book there, the first after
it that can be read. Nothing is followed from a book in no series opened
from no list, or one that is not in the list named. A book the server
holds twice is one book, whichever file was opened.

### Search

`GET /search?q=` takes up to 8 words and answers with the series, the
books, the reading lists and the people that hold every one of them, among
what the caller may read. A series and a reading list are found by their
name. A book is found by its file name and by the title, series, writer,
penciller and publisher its file gives. A person is found by any of the
names they go by, the ones with the most books first. `limit` asks for
fewer of each kind than the 50 it gives at most: the search box of the web
app shows five of each as it is typed, and Enter leads to the page with
everything.

Each series and book has a `search_key`: those texts in lower case with the
accents taken off, written by the scan. A search folds its words the same
way and asks for each as a part of the key, so "asterix" finds "Astérix"
and half a word is enough. There is no ranking: series and books come in
name order.

**Covers** are JPEG files of at most 480 by 720, stored under the book's
identity in `thumbnails/` in the data directory. Because the name is the
content, a cover never changes: it is served with a one-year cache lifetime,
and two copies of the same book share one. A cover is made once and never
deleted, not even when its file or its library is removed, so a book that
comes back finds its cover again. Nothing clears the covers of books that
are gone for good yet.

**Chosen covers.** The owner can give a book, a series or a reading list of their own another cover: any
page of any book, or a picture (a JPEG, PNG, WebP or GIF file of 20 MB at
most). The server makes a cover of it as it does of a first page and keeps
it in `covers/` in the data directory, under the hash of the cover itself,
so it too never changes under its name. `book_cover`, `series_cover` and
`reading_list_cover` say which cover was chosen for what, and `cover` then names it, with
`cover_chosen` true. A book's choice hangs on its identity: every copy
shows it, and it outlives the book as progress does. A series shows the
cover chosen for it, or else the cover of its first book, chosen or not; a
group shows that of its first book. One cover is kept for each, not
several to pick from. The file of a cover that nothing shows any more is
removed when another takes its place or it is taken back, and when its
reading list is deleted; the one of a series that is deleted stays behind. Unlike the covers the server makes, a
picture that was given cannot be made again from the library.

In the web app the choices are in the menu of a book's page and of a
series' page ("Choose a cover…", which asks for a picture, and "Use the usual cover"), and the
reader has a menu that makes a page on screen the cover of the book, of
its series, or of the reading list it is read from when that is the owner's own. Only the owner sees them. In the core they are
`Catalog::choose_cover` and `Catalog::forget_cover`, for a tool of the MCP
server to call.

### Finding series in a library

`crates/core/src/narrow.rs` answers `GET /libraries/{id}/series`: the
series of a library the caller may read, narrowed, in an order, a part at
a time.

**Narrowing.** A series is narrowed by what its books say: it is of a
publisher, a year or a person when one of its books is. Each thing has
values asked for and values ruled out (`publisher` and `not_publisher`,
and so on, each given as often as needed). Inside one thing the values
asked for are alternatives: Vertigo or Image. The things themselves all
hold at once: Vertigo, and the nineties, and unread. A value ruled out
leaves out every series that has a book of it.

- `read`: `unread` is a series the caller has not opened a book of,
  `in_progress` one they have started that still has books to read, and
  `read` one with no book left.
- `publisher`: as the files spell it.
- `year`: a year as `1993` or ten of them as `1990s`. A book's year is the
  one its place gives, or else the one its file says.
- `writer`, `penciller`: a person, by id. Every name the person goes by
  counts.

`GET /libraries/{id}/choices` lists what there is to narrow by, with how
many series each value has: the publishers, the decades, the writers and
the pencillers, the ones most series have first.

**Order.** `sort` is `name` (the usual), `added` (the series that got a
book last, first), `year` (the oldest first), `last_read` (what the caller
read last, first), `unread` (the series with books left first) or
`random`, which takes a `seed` so that every page is of the same shuffle.
`reverse` turns any of them around. What has no value comes last, and
every order ends in the name and the id, so it is total.

**Letters.** `GET /libraries/{id}/series/letters` takes the same
narrowing and answers with the first letters the names have, folded as
search folds them: `A` to `Z`, and `#` for everything else. Each comes
with how many series and the offset of its first one in name order, which
is what a client asks for to go to that letter.

**Held twice.** `GET /libraries/{id}/duplicates` lists, for the owner,
the books a library holds in more than one file: files with the same
identity, which is the hash of their pages
([ADR 0004](adr/0004-identify-books-by-page-content.md)), so a copy that
was renamed or tagged again is found too. Each file comes with its path
inside the library folder. The server only reports: it deletes nothing
([ADR 0005](adr/0005-library-folders-are-read-only.md)).

In the web app all of this is in the address of the library's page:
`/libraries/3?read=unread&publisher=Vertigo&year=1990s`. A list is
written as its values one after another under one name, by the router's
`stringifySearch` in `web/src/narrowing.ts`, which is also where an
address is read back and checked. So a narrowed library can be reloaded,
bookmarked and shared, and going back undoes one choice. The publisher,
the years and the people on the page of a series, a group or a book are
links to the library narrowed by them.

### People

Files name writers and pencillers as text, and spell one person several
ways. `crates/core/src/people.rs` keeps people as the names they go by
([ADR 0016](adr/0016-a-person-is-the-names-they-go-by.md)): `person`,
`person_name` for each spelling, and `book_credit` for a book that names a
spelling for a job. The scan writes the credits as it reads a file's
`ComicInfo.xml`. Spellings that differ only in capitals, accents, spaces
or punctuation are one name; the owner merges the rest
(`POST /people/{id}/merge`), and can take a name back out. Books scanned
before people existed get their credits when the server starts, from what
the database already holds.

A person is there for a caller when a book they may read names them.
`GET /people/{id}` gives the name they are shown by, how many books name
them as a writer and as a penciller, how many series hold those books,
and every name. Their series come whole, in name order; their books come
a part at a time, from the first that came out.

### Reading lists

A reading list is a name, a summary and books of any series in an order.
It belongs to the person who made it: anyone makes one, only they change
it, and they share it with the people they choose or with everyone
([ADR 0021](adr/0021-a-reading-list-is-its-makers-to-share.md)).

- **Whose it is and who sees it.** `reading_list.user_id` is the maker.
  A caller sees their own lists, and of the others those shared with
  everyone (`shared_with_all`) or with them (`reading_list_share`) that
  hold a book they may read. One SQL condition, `lists::may_see`, says
  so everywhere a list is read: the lists themselves, their books, their
  covers, search, and marking a list as read. A list carries `made_by`,
  `mine`, `shared_with_all` and, for its maker only, `shared_with`. A
  change to someone else's list is refused with 403 when the caller sees
  it and answered 404 when they do not. The owner of the server is
  treated as anyone else here. A name is unique among one person's
  lists.
- **What a caller sees of it.** A list holds book identities, each once. A
  caller is shown the ones that are on the server now and in a library they
  may read, in the list's order, and every number the list carries is
  counted among those: `book_count`, `unread_count`, the years its books
  span. A book the server holds twice is shown once.
- **Order.** Each book has a `position`. `PUT …/position` takes a position
  among the books the caller is shown, from 0, and the server puts the book
  before the one shown there now, or after the last. Books that are not
  shown keep their neighbours. New books go to the end.
- **Naming what to add.** Adding to a list, making one with books in it,
  and `POST /read-marks` name books the same way: `books`, `groups`,
  `series` and `lists`, each a list of ids. For a reading list the order
  is the order named: the books, then each group, series and list in the
  order it is read.
- **Seen whole.** `GET /reading-lists/{id}` answers as a series' overview
  does: what its books say together, how far the caller is through it, and
  the book to open next, which follows what the caller did last among the
  list's books. Its `summary` is the list's own, with no book named: what
  one of its books says is not about the list.
- **Cover.** The one chosen for it, or else the cover of its first book.
  `covers` names the ones to show together as one until a cover is chosen:
  those of its first books, four different ones at most, and none once a
  cover was chosen. The web app shows them when there are four. A chosen
  cover is shown to everyone who sees the list, whichever book its page is
  from. Choosing one is the owner's, for a list of their own.

**Importing a CBL file** is a dry run. A CBL file is XML that names a list
and its entries, each a series, a number, a volume and a year.
`POST /reading-lists/import` takes the file (5 MB and 5,000 entries at
most) and answers with a plan, a line for each entry. Nothing is made and
nothing of the file is kept.

For each entry the server looks for a series in turn by a rule the caller
kept, by the exact name, by the name with the volume ("Saga (2012)", "Saga
v2012"), by the name without its article, and by the name without words
such as "TPB", "HC", "Omnibus" or "Annual" and without what is in brackets.
Names are compared by the key series' names have. It stops at the first way
that finds a book with the entry's number. Among several books with the
number it narrows: by the kind of group a word of the entry names (the
annuals for "Annual", the collected editions for "TPB"), else the issues
of a run before the specials; then by the volume; then by the year. A
series that is one book with no number is that book whatever the entry's
number. Each line says what was found: the `book` and `how`, or
`no_series`, or `no_issue` with the series that was found, or `several`
with the books that fit.

A miss is put right by a rule: `POST /reading-lists/import/rules` keeps,
for the entry given, the series its name means or the book it is. Each
person has their own rules. They are
tried first, at this import and every one after, and a line found by one
carries its `rule_id`, so a wrong one can be forgotten. The file is then
sent again for a new plan, and the list is made with `POST /reading-lists`
from the books the plan found, as any list is.

In the web app the lists are at `/lists`, each a card with four covers as
one, the years its books span and how many it holds, and Home has a row of
them, the ones being read first. When other people shared lists with the
person, the page has two parts, "Yours" and "Shared with you", and a card
of someone else's list says whose it is. A list's page (`/lists/{id}`) has the
header a series has and under it one row for each book: its place, cover,
name, title, writer, penciller, date, and how far it was read. Under the
facts of the header the maker reads who sees the list ("Only you", "Shared
with Ana and Luis", "Shared with everyone"), which opens the dialog that
changes it, also in the menu as "Share…"; anyone else reads whose list it
is. On a server of one person nothing about sharing is shown. The maker
changes the order by dragging a row, or from a row's menu by typing a
place, which is the way on a touch screen. "Add to a reading list…" is in
the menu of every book, group, series and list and in the bar of what is
picked, for everyone; one dialog, mounted in the frame, asks which of the
person's own lists or the name of a new one. `/lists/import` shows the plan as a table with a button on each
line to say what the entry is. A book's page says which lists it is in.

### Collections

A collection is a name, a summary and series picked by hand from any
library, in no order of their own
([ADR 0022](adr/0022-a-collection-holds-series-and-is-shared-as-a-list-is.md)).

- **What it holds.** `collection_series` links a collection to the record
  of a series. A series stays in it under another name and with other
  books, and leaves it when it is deleted or its library is removed. A
  caller is shown the series of libraries they may read, by name, and
  `series_count`, `book_count` and `unread_count` are counted among
  those.
- **Whose it is and who sees it.** As a reading list: `collection.user_id`
  is the maker, `shared_with_all` and `collection_share` say who else
  sees it, and one SQL condition, `collections::may_see`, says so
  everywhere a collection is read. Someone it is shared with sees it only
  while it holds a series they may read. The part the two kinds share is
  `sharing.rs`: whether a thing is the caller's, the 403 and the 404 of a
  change to someone else's, and who it is shared with.
- **Cover.** `covers` names those of its first series, four different
  ones at most, and `cover` the first. None is chosen for it.

**In the web app.** `/collections` shows a person's own collections and,
apart from them, the ones shared with them. `/collections/$collectionId`
shows one: its series as the cards every page uses, whose it is or who
sees it, and for the maker the menu that renames, shares and deletes it.
The maker takes a series out from the menu of its card there. "Add to a
collection…" is in the menu of a series and in the bar of what is picked
while only series are picked; one dialog, mounted in the frame, asks
which of the person's own collections or the name of a new one. The
dialogs that name and share a collection are the ones a reading list
uses. A series' page says which collections it is in and which reading
lists hold a book of it.

## Reading and progress

**Pages.** `GET /books/{id}/pages` lists a book's pages with the width and
height the scan recorded, which is what a reader needs to lay out two pages
side by side before any image has arrived.

A page image is addressed by the book's identity and the page's number:
`/pages/{identity}/{number}`. The identity is the content, so what is under
that address never changes. It is served with a one-year, immutable cache
lifetime and an entity tag, and a request that names the tag of a page the
catalog knows is answered with 304 without opening the file. The address stays the same when the file
is moved or renamed.

To serve a page, the server finds the files that have that identity, opens
the first one, and computes the identity again from the archive's index,
which costs nothing next to reading the image (for a PDF it means reading
the whole file, each time). Only when it matches is the page read, whole,
and sent exactly as stored: never decoded, never re-encoded. The one
exception is a PDF page that is not a scan, which is drawn. So a file that was retagged since the last scan still gives its
pages; a file that now holds other pages is refused with a 409 that says to
scan again; a file that is gone is a 404. When the same pages are in the
catalog twice, any copy that can be read will do.

`/pages/{identity}/{number}/thumbnail` gives the page as a small picture:
a JPEG that fits 320 by 480 pixels, for a strip of pages and for the card
of a kept page. It is the one place a page is decoded and encoded again.
The server makes it from the page each time it is asked and keeps no
copy; it is cached for good like the page, with a tag of its own, so
whoever asked keeps it. A page the server cannot decode has none.

**Progress.** How far a book has been read is one row per reader and
identity: the last page that was on screen, whether that was the last page
of the book, and when. It has no link to the `book` table, so it is still
there after the file moves, after its drive was away, and after its library
was removed and added again. Two files with the same pages share it. Each
reader has their own; it goes when their account is removed. What was read
before the server had accounts became the owner's when the owner was made.

**Marks.** `POST /read-marks` marks books as read or as unread for the
caller: books by their ids, and every book of the groups and series named,
in one statement. A book marked read gets the progress of someone who just
reached its last page; one already read keeps the time it was read. A book
marked unread loses its progress, as if never opened. Ids that are not
there or not the caller's to read are passed over, and the answer says how
many books changed. One request names 10,000 ids at most. In the core it is `Catalog::mark_read`, which a tool of
the MCP server can call as it is.

**Kept pages.** A reader keeps a page to find it again:
`PUT /pages/{identity}/{number}/kept`, and `DELETE` to let it go. Like
progress it is a row per reader, identity and page number with no link to
the `book` table (`kept_page`), so it stays with the page when the file
moves and is shared by two files with the same pages. One person keeps
5,000 at most. `GET /books/{id}/kept-pages` gives the numbers kept in a
book, and `GET /me/kept-pages` all of them a part at a time, each with
its book: series by series in the order of their names, the books of a
series as they are read, books in no series last. A page whose book is
not on the server now is left out until the book is back. It is
`crates/core/src/kept.rs`.

**Sittings.** Progress says where a reader is in a book and forgets where
they were. So each save of progress also goes on a record of when they
read: a `sitting` is one reader with one book with no pause of more than
half an hour between two saves, with when it started and ended and the
first and last page saved. The web reader saves a page when it is turned
to, so the page a book was opened at is not in the record. Marks write
none.

The sittings also say how fast someone reads. `Catalog::pace` is the
seconds a page took over their latest 100 sittings that went between 3
and 300 seconds a page: faster was leafing through the book, slower was a
book left open. It is not believed until those sittings turned 20 pages.
The overviews multiply it by the pages not yet read (`seconds_left`). A
sitting knows its first and last page only, so a long jump through a book
counts as pages read fast.

**What was read** is told from the sittings, by
`crates/core/src/history.rs`. `GET /me/reading?year=` answers with the
days of a year the caller read on, each month, the ten writers they read
most, and the sums of the year; each counts the books read from, the pages
gone through and the time spent. The pages of a sitting are from its first
page to its last, whichever way it went. A day is the reader's own day:
the caller says how many minutes their clock is ahead of UTC
(`minutes_ahead`), and a sitting belongs to the day it started on. It is
one distance for the whole year, and the web app sends today's: where
clocks change with the season, a sitting begun within an hour of midnight
in the other season is told on the day beside its own. Telling every day
right needs the name of the reader's time zone in place of a distance,
which is not built. A book
only marked as read adds nothing, since a mark writes no sitting. The web
app shows it at `/history`: a square for every day of the year, fuller the
more pages, the pages of each month as bars, and the writers, each leading
to their page.

`GET /reading-now` tells the owner who is reading: for each person whose
last sitting ended within the half hour that ends a sitting, the book and
the page.

`GET /continue-reading` lists the books that were started and not finished,
the one read last first, each identity once. `GET /recently-added` lists the
readable books the server saw last, and `GET /recently-updated` the series
by the newest of their books.

`GET /up-next` lists what to read after a finished book. For each series the
caller read from, it takes the book they read last: when that one is
finished and the first unfinished book after it in reading order has not
been opened, that book is offered. A book that was opened is in
`/continue-reading` instead, and a series with nothing left offers nothing.
The series read last comes first. A series leaves the list 30 days after it
was last read from, and a book added to it brings it back for 7 days. A
series the caller took off the list (`DELETE /up-next/{series_id}`) stays
off, also when it gets a book, until they read from it again: what is kept
is when it was taken off, which counts only while it is later than their
last reading. The
server works this out, as it does the "next" of a series, so every client
shows the same row.

Home shows the four lists in that order, each as one line of covers that
goes on sideways, with the reading lists and the views that are rows after
"Up next", and the libraries under them all. Each person arranges it: the
order of the rows and which are put away, kept by the server (`home.rows`).
A row with nothing in it is not shown, and the libraries always end the
page.

**The web reader** is the page at `/read/{book id}`. It fills the window,
without the bar at the top or the menu beside the page.

- It shows one page, or two side by side when that is chosen and the window
  is wider than it is tall. In pairs, the cover stands alone and so does any
  page wider than tall.
- Pages are sized to fit the window, its width, its height, or not at all.
- The left and right arrow keys, Space, Page Up and Down, Home and End turn
  pages; the left and right thirds of the window do too, and the middle
  shows or hides the bars. Reading right to left swaps the sides. The up and
  down arrows scroll a page taller than the window.
- Once the screen being read has arrived, the next two are loaded ahead and
  the one before is kept.
- It opens where the book was left. A finished book, and one left alone
  for 90 days, first asks where to start: again from the first page, on
  from where it was left, or only a look. "Read again" on a series, a
  group or a list (`again` in the address) starts a finished book over
  with no question, and so does the reader for the book it goes on to.
- Opening saves nothing; each page turn saves the last page on screen, one
  request at a time, the latest page winning. A save that got no answer is
  tried again for a while, and the reader says so while its place is not
  being saved.
- A reading can be left unrecorded (`unrecorded` in the address, "Read
  without recording" in a book's menu, "Only look", or the settings of
  the reader): turning pages then saves nothing, so it moves no progress
  and writes no sitting. The bar at the top says so, and pressing that
  records from the page on screen.
- The bar at the top keeps the page on screen, or lets it go. Kept pages
  are listed at `/kept`, by series, each opening its book on that page
  for a look (`page` in the address, from 1).
- The bar at the bottom opens a strip of the book's pages as small
  pictures to jump by, with the pages on screen and the kept ones marked.
  A picture is asked for when it comes into view.
- Closing goes back to where the book was opened from. A book opened by its
  address goes to its series, or to its library when it is in no series.
- The address says where the book was opened from: `list` with a reading
  list's id, `group` from the page of its group. Going on from the last
  page offers the book that follows there, which the server names
  (`GET /books/{id}/next`), and going on once more opens it in the same
  place. So a volume, a series or a list is read to its end without
  leaving the reader, and closing still leads back to where the first
  book was opened from. At the end of what is followed the reader says
  so.
- Fit, one or two pages and direction are the person's, kept by the
  server (see "What a person chose"): what they usually read with under
  `reader`, and under `reader.series` what they changed while reading a
  series, which that series then keeps, for the 60 series changed last.
  The settings of a series can be made the usual ones or let go. What a
  browser kept before the server did is read until the first change.

### Views

A view is a library in an order and narrowed, kept under a name:
"Unread Vertigo of the nineties". See
[ADR 0017](adr/0017-a-view-is-a-library-narrowed-and-kept-by-name.md).

- **The owner's to make, everyone's to see.** `POST /views` takes a
  library, a name, what is `asked` and where it shows. Whoever may read
  the library sees the view. An AI client makes one with the owner's API
  token until the MCP tool exists.
- **What it asks** has the fields `GET /libraries/{id}/series` takes:
  `sort`, `reverse`, `seed`, and each thing to narrow by with its `not_`
  twin. A client shows a view by asking that route with them. A view
  narrows by 100 values at most.
- **Where it shows.** `on_home` makes it a row on Home, with its first
  series and the way to the rest; `in_menu` makes it an entry in the menu
  beside the page. The web app marks the entry while the library shows
  exactly what the view asks, on any page of it.
- **In the web app** the owner keeps what a library's page shows with
  "Keep view", and changes or deletes views at `/settings/views`.

### What a person chose

`crates/core/src/settings.rs` keeps, for each person, values under keys
that the server does not read. See
[ADR 0018](adr/0018-what-a-person-chooses-is-kept-as-keys-and-values.md).
A key is 1 to 64 lower-case letters, digits, dots, dashes or underscores,
a value is 8 KB of text at most, and a person keeps 100 at most. The web
app keeps four: `cover-size`, the size of the covers in rem; `reader` and
`reader.series`, how the reader shows books, as JSON; and
`home.rows`, the arrangement of Home as JSON with the `order` of its rows
and the ones `hidden`. A row is named `continue`, `up-next`, `lists`,
`updated`, `added` or `view:` and a view's id. A row the arrangement does
not name, as a view made since, comes after the row it would follow by
itself.

## How it is built and run

**Release.** `scripts/build.sh` runs Vite, which writes the UI to
`web/out/dist`. Then cargo compiles the server; its build script copies that
folder to where the compiler embeds it. The result is
`target/release/binge-server`, with no files beside it.

If the UI has not been built, the build script embeds a placeholder page so
the server still compiles. A release build warns when the UI is missing or
older than its sources.

**Docker.** `scripts/docker.sh` builds an image from the `Dockerfile`: the
same binary on a small Debian, listening on the container's network, with
`/data` as its data directory. The build stages run on the machine that
builds and cross-compile the server, so an image for an Intel NAS builds in
minutes on an ARM machine. The script also saves the image as a file under
`target/docker`, to carry to another machine without a registry. Comic
folders are mounted read-only. See [docker.md](docker.md).

The image's health check is the binary itself: `binge-server health`
(`crates/server/src/health_check.rs`) asks `GET /api/v1/health` of the
server on this machine, where `--bind` and `--port` say it listens, and
exits with 1 when nothing answers, when what answers is not a Binge
server, or when the database does not answer. So the image needs no
other program to ask with.

**Releases.** GitHub builds them from a tag: the binaries for Linux, macOS
and Windows and the image, each binary started once before it is packed.
The Linux binaries are taken out of the image's own build, so they are
the ones the image runs. The same builds run without publishing on a pull
request that changes them, and every push runs `scripts/check.sh` and a
check of the licences of what ships. See [releasing.md](releasing.md).

**Updating.** The owner brings the server to a newer release from the
About page ([ADR 0020](adr/0020-the-server-updates-itself-from-signed-releases.md)).
The parts, in `crates/server/src/update`:

1. *Looking.* The server fetches `update.json` and its signature from the
   latest release on GitHub and reads the version in it. It does this when
   the owner asks, and once a day while the owner has that on. It is off
   at first: this is the only request the server makes to the outside.
2. *Fetching.* The button names the version the owner was shown. When
   another came out since, nothing is fetched and that one is shown.
   Otherwise the server fetches the archive the file names for its system, compares its checksum, takes the binary out,
   compares that checksum too, and runs it with `--version` to make sure
   it starts here. Then it keeps the binary with the file and the
   signature in `updates/<version>/` in the data directory.
3. *Starting again.* The server stops taking requests, gives the ones
   under way ten seconds, closes the database, and starts the installed
   binary again in its own process.
4. *Handing over.* A server that starts looks in `updates/` before it
   opens anything. For the newest version there that is newer than itself
   it checks the signature and the binary's checksum again, and runs that
   binary in its own place, with the options it was started with. So the
   process whoever started the server knows (Docker, a service, a
   terminal) stays the same one. A version that is not proven is passed
   over with a warning, and the owner is told that a newer version is kept
   and was not started (`not_started`). Versions the running one has
   caught up with are removed.

`GET /server/update` says where all this stands, and the web app follows
an update with it until another version answers, then loads itself again:
the web app is part of the binary. Windows servers and builds for
development look but do not fetch, and `--no-updates` turns all of it off.

**Development.** `scripts/dev.sh` runs the server and Vite together. The
browser talks to Vite, which reloads on changes and forwards `/api` to the
server.

**Configuration.** Options and environment variables, listed in the
[README](../README.md): data directory, bind address, port, allowed host
names. The server listens on `127.0.0.1` unless told otherwise; `--bind
0.0.0.0` opens it to the network.

## Data

- **Data directory.** Holds `binge.db`, `thumbnails/`, `covers/` and
  `backups/`, and later page maps and translation tracks. Chosen with `--data-dir`; defaults
  to the platform's application data folder. A scan rebuilds the catalog and
  the covers it makes; the accounts, tokens, reading progress, chosen
  covers and reading lists are lost with it.
- **Database.** SQLite in WAL mode with foreign keys on. Migrations run at
  startup. SQLite has one writer at a time; every transaction that writes
  takes that place when it starts and waits its turn for it, so two
  libraries scanned together do not refuse each other. The id of a library, series or book is never given out again
  after it is removed, because clients keep ids in addresses. Tables: `library`, `series` (one per name), `series_group` (a part of a series, such as a volume or its annuals), `book_place` (where a book sits, by its identity), `book` (one per
  file: path, size, modification time, page count, identity, and what is
  wrong with it if it cannot be read), `book_info` (what the file's
  `ComicInfo.xml` says), `page` (entry name, byte size, width, height), `user` (name, password hash, whether they are the owner),
  `library_access` (which libraries a member may read), `session` and
  `api_token` (a hash of the secret, its user, when it was last used),
  `progress` (one row per user and book identity: page, completed, when),
  `sitting` (one row per stretch of reading: user, book identity, when it
  started and ended, first and last page), `kept_page` (a page a user
  kept: book identity, page number, when), `off_up_next` (a series a user
  took off their "Up next", and when), `book_cover`, `series_cover` and
  `reading_list_cover` (the cover chosen for a book identity; for a series;
  for a reading list), `reading_list` (whose it is, a name, a summary and
  whether everyone sees it), `collection`, `collection_series` and
  `collection_share` (the same for a collection and its series), `reading_list_share` (a person a list is
  shared with), `reading_list_book` (a book identity and its position in
  a list), `cbl_rule` (what a person said an entry of a CBL file means),
  `person` (someone the books name, and the name they are shown by),
  `person_name` (one way of writing a person's name),
  `book_credit` (a book names a spelling as its writer or penciller),
  `kept_view` and `kept_view_value` (a view: a library, a name, an order
  and where it shows; one value it asks for or rules out),
  `user_setting` (a value a person keeps under a key),
  `scan_record` and `scan_change` (a scan that changed a library; one
  file it found arrived, back, moved or gone),
  and `setting` for the server's own key and value pairs. A `book` also
  says when it was set aside, if it could not be read and was.
- **Copies of the database.** `crates/core/src/backup.rs` copies the
  database into `backups/` in the data directory with `VACUUM INTO`, which
  gives one whole file while the server keeps running. A copy is written
  under another name and renamed when done, so a half-written one is
  never taken for a copy. Copies are named `binge-YYYYMMDD-HHMMSS.db` by
  the time in UTC, the latest 7 are kept, and only files named so are
  ever removed. The server makes one when the newest is a day old: it
  looks five minutes after it starts and every hour after. The owner can
  turn that off and make one at any time (`/server/backups`). A copy
  holds no covers, which a scan makes again, and sits on the same disk as
  the database: it guards against a mistake, not against a disk that
  fails. Putting one back is done by hand, with the server stopped.
- **Library folders.** Read, never written. See
  [ADR 0005](adr/0005-library-folders-are-read-only.md).

## Security today

- **Every request shows who makes it**, except `/health`, setup and signing
  in. See [Accounts and signing in](#accounts-and-signing-in).
- **Only hashes are stored**: Argon2id for passwords, BLAKE3 for the random
  secrets of sessions and tokens. Passwords and secrets are not logged; the
  request log has no headers or bodies in it.
- **The Host check** above, the refusal of changes asked for by pages of
  another origin, and headers that keep the server's pages out of frames.
- **The server listens on this machine only by default.** With `--bind` it
  listens on the network, which is what accounts are for.

What it does not do:

- **No TLS.** The server speaks plain HTTP, and the session cookie is not
  marked `Secure`. Someone who can read the network's traffic can read
  passwords, cookies and tokens as they pass. Reaching it from outside the
  home is meant to go through a TLS proxy, or later Binge+.
- **Setup is open to whoever comes first.** A new server bound to the
  network belongs to the first person who opens it, and it logs a warning
  when started that way. Make the owner before changing `--bind`.
- **The owner's password cannot be recovered.** There is no mail and no
  reset command yet.
- **The owner can add any folder the server's account can read** as a
  library. The owner's account is as strong as its password.

## Planned, not built

Listed here so the parts above make sense. Details are in
[PLAN.md](../PLAN.md).

- **AI features (6 and later).** The server does no inference. It stores the
  files and the results, breaks the work into small steps, and exposes those
  steps as MCP tools. The user's own AI client does the reading and
  translating. See
  [ADR 0002](adr/0002-ai-work-runs-in-the-users-ai-client.md).
- **Binge+ (9).** A paid cloud service for accounts and remote access. The
  comics themselves never leave the user's server.
- **The Binge readers, last.** The iOS reader connects once the server has
  its features, and `/api/v1` freezes then.

## Decision records

| | Decision |
| --- | --- |
| [0001](adr/0001-free-local-server-for-binge-clients-only.md) | Free local server, for Binge's own clients only |
| [0002](adr/0002-ai-work-runs-in-the-users-ai-client.md) | AI work runs in the user's AI client |
| [0003](adr/0003-one-binary-rust-sqlite-embedded-ui.md) | One binary: Rust, SQLite, embedded web UI |
| [0004](adr/0004-identify-books-by-page-content.md) | Identify books by page content |
| [0005](adr/0005-library-folders-are-read-only.md) | Library folders are read-only |
| [0006](adr/0006-one-api-contract-generated-from-code.md) | One API contract, generated from the code |
| [0007](adr/0007-refuse-unknown-host-names.md) | Refuse requests for unknown host names |
| [0008](adr/0008-one-reader-until-accounts-exist.md) | One reader until accounts exist (superseded by 0009) |
| [0009](adr/0009-first-run-setup-makes-the-owner.md) | First-run setup makes the owner |
| [0010](adr/0010-sessions-for-browsers-tokens-for-apps.md) | Sessions for browsers, tokens for apps |
| [0011](adr/0011-read-cbr-and-pdf-with-rust-libraries.md) | Read CBR and PDF with libraries written in Rust |
| [0012](adr/0012-series-and-groups-are-a-layout-over-the-files.md) | Series and groups are a layout over the files |
| [0013](adr/0013-the-layout-changes-only-as-asked.md) | The layout changes only as asked |
| [0014](adr/0014-reading-lists-belong-to-the-server.md) | Reading lists belong to the server (superseded by 0021) |
| [0015](adr/0015-long-lists-are-read-a-part-at-a-time.md) | Long lists are read a part at a time |
| [0016](adr/0016-a-person-is-the-names-they-go-by.md) | A person is the names they go by |
| [0017](adr/0017-a-view-is-a-library-narrowed-and-kept-by-name.md) | A view is a library narrowed, kept by name |
| [0018](adr/0018-what-a-person-chooses-is-kept-as-keys-and-values.md) | What a person chooses is kept as keys and values |
| [0019](adr/0019-the-server-is-under-the-agpl.md) | The server is under the AGPL |
| [0020](adr/0020-the-server-updates-itself-from-signed-releases.md) | The server updates itself from signed releases |
| [0021](adr/0021-a-reading-list-is-its-makers-to-share.md) | A reading list is its maker's, to share |
| [0022](adr/0022-a-collection-holds-series-and-is-shared-as-a-list-is.md) | A collection holds series, and is shared as a list is |
