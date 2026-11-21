# Binge Server plan

Written 2026-11-15. A minimal comic server in Rust with a React UI, built in
small iterations. Each iteration ends with something you can run and look at.

## What it is

- A local server you point at your comic folders. Open source and free.
- The only clients are Binge's own: the web UI in this repo, then the iOS and
  Android readers.
- The base that the AI features will sit on later: MCP tools, page maps,
  translation tracks, organization. None of that is built in the first
  iterations, but three rules below exist because of it.

Not in scope: compatibility with other comic servers or with OPDS; inference
paid for by Binge.

## Rules that hold from the first commit

The reasons are in the [decision records](docs/adr/README.md).

1. **One binary.** The server embeds the built web UI and uses one SQLite file.
   Install is "download, run, open the browser".
2. **Library folders are read-only.** The server never writes, renames or
   deletes inside a library. Organization features later will ask for write
   access explicitly and show a dry run first.
3. **Identity is the content, not the path.** Each book gets an ID computed
   from its page images (the checksum and size of each image, which a CBZ or
   CBR already stores and which are computed for a PDF, hashed together with
   BLAKE3). Progress, and later page maps
   and tracks, attach to that ID, so moving or renaming a file loses nothing,
   and neither does adding a metadata file inside the archive. A file's size
   and modification time are kept only to notice changes.
4. **Folders are a hint, not the truth.** Series, order and metadata live in
   the database. The folder layout gives the first guess and can be corrected
   without touching the files.
5. **One API contract.** `/api/v1`, described by an OpenAPI document generated
   from the Rust code. The web client is generated from it, and the Swift and
   Kotlin readers use the same document.
6. **Derived data lives outside the library**, in the server's data directory:
   database, thumbnails, and later page maps and tracks.

## Stack

| Part | Choice | Why |
| --- | --- | --- |
| HTTP | axum on tokio | Standard, small, good ecosystem |
| Database | SQLite through sqlx, with migrations | One file, no service to run |
| Archives | `zip` for CBZ, `rars` for CBR, `hayro` for PDF | All written in Rust, so the server stays one binary. See [ADR 0011](docs/adr/0011-read-cbr-and-pdf-with-rust-libraries.md) |
| Metadata | `roxmltree` for ComicInfo.xml | Small and read-only |
| Images | `image` for sizes and cover thumbnails | Pages are served as stored, never re-encoded. The one exception is a PDF page that is not a scan, which is drawn |
| Hashing | `blake3` | Book identity |
| API description | `utoipa` | OpenAPI from the handlers |
| UI embedding | `rust-embed` | Single binary |
| Web | pnpm, Vite, React, TypeScript | |
| Web data and routes | TanStack Query and TanStack Router | Typed routes, cached fetches |
| Styling | Tailwind, themed from Binge's `design/tokens.json` | Same look as the apps, dark and light |

Versions are the latest stable at scaffold time, checked then rather than
assumed. The Rust version is the one `rust-toolchain.toml` names. One
exception: TypeScript stays on 5.9, because the generator for
the typed API client does not run on TypeScript 7 yet. Lint and formatting on
the web side use Biome.

## Layout

```
Cargo.toml            workspace
crates/
  core/               libraries, scanner, archives, pages, progress, accounts (no HTTP)
  server/             axum API, sign-in check, embedded UI, the `binge-server` binary
web/                  React app
migrations/           SQL
api/openapi.json      the API contract, generated
scripts/              build, dev, API generation, checks
docs/                 architecture and decision records
AGENTS.md             rules for changing the code
PLAN.md
```

`core` has no HTTP in it so the MCP server can reuse it later as a third crate.

## Data model, first version

- **library**: name, root path.
- **series**: the books that go by one name, in one library. Binge's own
  record, not a folder (ADR 0012).
- **series_group**: a part of a series: a volume, a limited series, the
  annuals, the collected editions, the one-shots, the appearances in other
  titles.
- **book_place**: where a book sits, by its identity: series, group, issue
  number, date published. Written once, at import, and after that changed
  only when the owner asks (ADR 0013).
- **book**: one file. Path, size, modified time, content hash, page count.
  Where it sits is in `book_place`, found by its content hash; a file that
  cannot be read has none and is in no series.
- **book_info**: what a file's ComicInfo.xml says about it: title, series,
  number, volume, date, writer, penciller, publisher, summary, language.
  It belongs to the file, not to the identity, so two files with the same
  pages can say different things.
- **page**: index, entry name, width, height, byte size. Readers need the sizes
  to lay out spreads.
- **progress**: user, book identity, page, completed, updated at. One row
  per reader and book.
- **user**: name, password hash, whether they are the owner.
  **library_access**: the libraries a user who is not the owner may read.
- **session**, **api_token**: a hash of the secret, the user, when it was
  last used. A token also has a name.

A file is a book. Its series and its group come from import, which reads
the tag, then the file's name, then the folder, once. Series and books also
carry a search key: their words in lower case with the accents taken off.
Reading lists came with step W3 below; collections wait until something
needs them.

## Iterations

### 0. Skeleton (done 2026-11-15)

Workspace, both crates, the web app, and the plumbing between them.

- `GET /api/v1/health` returns name and version.
- Config: data directory, port, bind address. Binds to `127.0.0.1` by default
  until accounts exist.
- SQLite opens and runs migrations at start.
- Dev: Vite with a proxy to the server. Release: UI embedded in the binary.
- OpenAPI document served, TypeScript client generated from it.
- Checks: `cargo fmt`, `clippy`, `cargo test`, web typecheck and lint, one script
  that runs them all. Git repository initialized.

**Done when:** one command builds a binary that serves the Binge-themed shell
page showing the server version.

### 1. Libraries and scanning (done 2026-11-16)

- Add and remove a library by folder path from the UI.
- Scanner in two passes. Fast pass: walk folders, create series and books,
  count pages, compute each book's identity. Background pass: read page
  sizes, make cover thumbnails. Progress of the scan is visible in the UI.
- Rescan picks up added, changed and removed files. Every library is also
  scanned when the server starts.
- UI: sidebar with libraries, series grid with covers, series page with its
  books.

**Done when:** you point it at a real folder of CBZ files and browse covers.

Changed while building it:

- No hash of the whole file. It meant reading every byte of the library to
  learn what size and modification time already say. See
  [ADR 0004](docs/adr/0004-identify-books-by-page-content.md).
- A moved file is seen as one book removed and one added. Its identity and
  cover are the same, so nothing is recomputed; following a move without the
  remove and add is iteration 5.
- A file that cannot be read is listed with what is wrong with it, not hidden.
- The folder path is typed. A folder picker that browses the server's disk
  waits for accounts (iteration 3), since it would show the disk to anyone
  who can reach the port.
- Files directly in the library folder are books without a series, shown on
  the library page next to the series. The scanner does not guess series from
  file names; sorting a flat folder is left to an AI client through MCP,
  later.
- Lists are returned whole. Paging waits until the reader's needs are known,
  before the API freezes, when the Binge reader connects.

### 2. Reading and progress (done 2026-11-16)

- Page endpoint that serves the stored image with caching headers.
- Web reader: single page and two-page spread, keyboard and tap zones, fit
  modes, left-to-right and right-to-left, preload of the next pages.
- Progress saved per book. Home page with Continue Reading and Recently Added.

**Done when:** you read an issue in the browser, close it, and resume on the
same page.

Changed while building it:

- A page image is addressed by the book's identity and the page number, not
  by the book's id. What is under such an address never changes, so it is
  cached for good, and stays cached when the file moves. Before serving, the
  server checks that the file still holds those pages.
- A page is read whole into memory and sent as stored, not streamed in
  pieces. Pages are a few megabytes. The archive is opened for each page;
  keeping archives open between requests waits until it is shown to matter.
- There are no accounts yet, so the server has one reader and progress is
  keyed by the book's identity alone. See
  [ADR 0008](docs/adr/0008-one-reader-until-accounts-exist.md).
- Progress is the last page on screen. Opening a book saves nothing, turning
  a page does, and reaching the last page completes the book. A completed
  book opens at its start.
- Progress is kept when a book or its library is removed, so it is there
  when the same pages come back.
- Fit, one or two pages, and reading direction are settings of the browser,
  for every book. A direction per book waits for metadata; iteration 4
  reads metadata but not yet the direction.
- Two pages are shown only when the window is wider than it is tall. The
  cover and any page wider than tall stand alone.
- The last page does not lead to the next book of the series. Not built.
- The home lists are short (12 books each in the web UI). The API takes a
  `limit`; paging still waits, as for the other lists.

### 3. Accounts and tokens (done 2026-11-16)

- First-run setup creates the owner. Password login with a session cookie for
  the web UI.
- API tokens for apps, created and revoked in settings.
- Every endpoint except health and setup requires auth. The server can now bind
  to the network.
- More users with per-library access.
- Progress becomes each reader's own. What was saved before goes to the
  owner.

**Done when:** a second device on the network logs in and reads, and a revoked
token stops working.

Changed while building it:

- Setup is a page of the web UI, open to whoever reaches a server that has
  no owner. There is no setup code to copy from the log. See
  [ADR 0009](docs/adr/0009-first-run-setup-makes-the-owner.md).
- The server still listens on `127.0.0.1` unless `--bind` says otherwise.
  "Can bind to the network" means it is now safe to, not that it does: a
  new server reachable from the network would hand its owner account to
  whoever opens it first, so that stays a choice.
- Two roles and no more. The owner reads everything and manages libraries
  and users; a member reads the libraries the owner ticked for them. A
  library a member may not read is answered as not found.
- A browser is known by a session cookie, an app by a token. Only hashes of
  passwords, sessions and tokens are stored. See
  [ADR 0010](docs/adr/0010-sessions-for-browsers-tokens-for-apps.md).
- A token does what its user can, except manage accounts: tokens, passwords
  and users take a session. An app signs in with the password once, makes
  itself a token, and signs out; there is no single call that trades a
  password for a token.
- Each person makes and revokes their own tokens. The owner cannot see or
  revoke a member's tokens; removing the member is what disconnects their
  apps.
- There is no way to recover the owner's password. It is not sent anywhere
  and the server has no mail. A command that resets it from the machine the
  server runs on is not built.
- Progress saved before accounts went to the owner when the owner was made.
- The API description, `/api/v1/openapi.json`, is inside the check too.
- A folder picker for adding a library, put off in iteration 1 until
  accounts existed, is still not built. The path is typed.

### 4. Formats, metadata and search (done 2026-11-17)

- CBR and PDF files are books, like CBZ.
- What a file says about itself in ComicInfo.xml is read and kept: title,
  series, number, volume, date, writer, penciller, publisher, summary,
  language.
- Search over series and books, in the API and as a page of the web UI.

**Done when:** every comic in a real folder shows up, whatever its format,
and typing a writer's name finds their books.

Changed while building it:

- CBR is read by `rars` and PDF by `hayro`, both written in Rust. Nothing
  outside the binary is needed, and the licence worry about the usual RAR
  library is gone. See
  [ADR 0011](docs/adr/0011-read-cbr-and-pdf-with-rust-libraries.md).
- A file is read as what its first bytes say it is, not as what it is
  called. Many `.cbr` files are zips.
- A PDF page that is one scanned picture is handed out as that picture,
  untouched. Any other page is drawn at 2200 pixels on its longer side and
  sent as a JPEG.
- The same pictures give the same identity whether they sit in a CBZ, a CBR
  or a scanned PDF, so progress follows a book across formats.
- A PDF is read whole each time one of its pages is asked for. Keeping it
  open between requests waits until it is shown to matter, like for the
  other formats.
- A file protected with a password is listed as a book that cannot be read,
  and says why. So is a file that makes a reader crash; the scan goes on.
- Not made fast yet, because no file here needed it: a rar packed as one
  stream, and a scanned PDF that also carries recognized text (its pages
  are drawn, not handed out as they are).
- Metadata is shown, and used in one place (2026-11-17, after the first
  real library was in). A book card shows the number the file gives, with
  its title or else its writer. A book has a page of its own with
  everything its file says: summary, writer, penciller, publisher, date,
  volume, language. A series whose files name more than one volume is
  split by volume.
- Books still come in file name order, and series are still folders.
  Ordering by the number in the metadata was tried against the real library
  and left out: in the folders where every file is tagged it changed
  nothing, because the file names already carry the number, and in the
  folders where a few files are tagged it pulled those few out of their
  place.
- A series has a page that presents it (2026-11-17): the cover with the
  facts beside it, one button that goes on where the reader stopped, the
  cover's colours behind the top of the page, volumes as tabs, how many
  books are read, a way to see only the unread ones, and a summary taken
  from the first published book that has one, saying which book. Left out
  because the server does not know them: age rating, genres, tags,
  characters, publication status, related series, ratings and reviews.
  Left out on purpose: editing metadata, downloads, and books in pages of
  20.
- The server works out a series as a whole (`GET /series/{id}/overview`),
  so a reader app or an AI client gets the same answer as the web UI: total
  pages, how much the reader has read, the book to open next, and the
  publisher, years, writers and pencillers its books name. "Next" follows
  what the reader did last: an unfinished book, or the first unfinished one
  after the book they finished.
- With volumes, the series page shows one volume at a time, the one the
  reader is in. A folder of 800 books no longer draws 800 covers at once.
- How long a series takes to read is a guess of 20 seconds a page, made in
  the web UI. It is not measured from how the reader reads.
- ComicInfo.xml is read from CBZ and CBR. What a PDF says about itself is
  not read.
- Search asks that every word typed be somewhere in the name, title,
  series, writer, penciller or publisher. Capital letters and accents do
  not matter. It answers with at most 50 series and 50 books, and says when
  there are more; there is no ranking and no search inside summaries.
- Every list of books carries each book's whole metadata, summary
  included. Whether lists should leave the summary out is to be decided
  before the API freezes.
- The first start after this update reads every file again to pick up the
  metadata. Pages, covers and progress are kept.
- That first start showed a fault that was there since iteration 1: with
  two libraries scanned at once, one scan could fail with "database is
  locked". Writes now wait their turn.
- The Rust version is now pinned in `rust-toolchain.toml`, because the PDF
  library needs a newer compiler than the one the project started with.

### 5. Real libraries

- Docker image and release binaries for Linux, macOS and Windows,
  published.
- Updating from the web app with a button.

Changed on 2026-11-19, with the owner:

- **File watching is dropped.** New files are found by the scan at start
  and by a scan asked for. The old finish line, a week on the NAS without
  a manual rescan, goes with it.
- **Move detection needed no work.** A book's place and progress hang on
  its pages, and the record of a scan already says which files moved.
- **Where it is published.** The repository is `bingecomics/binge-server`
  on GitHub, in an organization the owner made from their personal
  account. Code and releases are public there. The licence is the AGPL
  ([ADR 0019](docs/adr/0019-the-server-is-under-the-agpl.md)).
- **Updating.** The owner presses a button; the server fetches the new
  binary, checks its signature against a key built into it, keeps it in
  the data directory and starts the newest checked one it finds there.
  The same in Docker, where a container cannot replace its own image.
  The look for a newer version asks an address outside the server, so it
  stays off until the owner turns it on.
- **Signing.** macOS binaries are signed with the owner's personal
  Developer ID. Windows binaries are not signed.

Built so far: the Docker image, for Intel and ARM, built here and carried
over as a file (2026-11-17, [docs/docker.md](docs/docker.md)); the
licence, and the image's health check, which is the binary asking itself
with `binge-server health` (2026-11-19); and the builds on GitHub, which
run the checks on every push and, on a tag, build the binaries and the
image and publish them (2026-11-19, [docs/releasing.md](docs/releasing.md)).
The Linux binaries need glibc 2.36 or newer, as the image has. The first
release, 0.1.0, was published on 2026-11-19 with the macOS binary signed
and notarized.

The update button (2026-11-19,
[ADR 0020](docs/adr/0020-the-server-updates-itself-from-signed-releases.md)).
Decided while building it:

- **The key that signs updates** is a secret of the repository, with a
  copy the owner keeps. A second, spare key is built into the server too
  and is kept by the owner only, for the day the first is lost.
- **What is signed** is one small file per release that names the binaries
  by their checksums, not each binary. The server checks it when it
  fetches a version and again every time it starts one.
- **A daily look, when the owner turns it on**, and a look whenever the
  owner asks. The owner is told in the About page and under the server's
  status in the side menu. The server never fetches or starts a version
  by itself.
- **Windows does not update itself.** A running program cannot take its
  own place there. It says when a newer version is out.
- **`--no-updates`** turns all of it off where the server is started.
- **The button names the version the owner was shown.** When another came
  out since, nothing is fetched and that one is shown.
- **A kept version that does not start is told to the owner**, on the
  About page, for as long as that server runs.
- **The keys a server trusts when it starts are the installed binary's.**
  After a lost key, releases stay signed with the spare; a new key reaches
  a server only with a newer binary or image.

The repository was opened on 2026-11-19, since a server fetches releases
without signing in. Release 0.2.0 is the first that carries the button.

Release 0.2.1 changes nothing but the number: it is the first a server can
be brought to with the button.

**Done when:** the server on the NAS is brought to a new release with the
button.

Done on 2026-11-19: the server on the NAS went from 0.2.0 to 0.2.1 with the
button, in the same container. The page said nothing clear afterwards, so
it now says that the server was updated. Release 0.2.2 carries that.

Also built here (2026-11-17), ahead of the AI work it is for: import and
the layout from the database
([ADR 0012](docs/adr/0012-series-and-groups-are-a-layout-over-the-files.md)).
A series is the books that go by one name, wherever their files are, with
volumes and specials as groups, and a book keeps its place when its file is
moved, renamed or retagged. The one group of specials was split the same
day into three kinds: annuals, one-shots and appearances in other titles.
Import tells an annual or a collected edition by its `Format` tag and puts
any other special in the one-shots. Every kind of group behaves alike: a
number, a name, both or neither, the same reading order, the same page.

Then the operations that change it, as routes for the owner
([ADR 0013](docs/adr/0013-the-layout-changes-only-as-asked.md)): create,
rename and delete a series or a group, and set a book's place. Each does
exactly what was asked or refuses with the reason; nothing is merged, made
or removed on the side. Not yet: doing any of it in the web app beyond
deleting an empty series or group, and the MCP tools that will call the
same operations. An AI client changes the layout directly, as the
owner; nothing waits for approval.

The series page follows (2026-11-17): its volumes, limited series, loose
issues, annuals, collected editions, one-shots and appearances are tabs,
a group is a card where a tab has several, and each group has a page of its own with its books and
the reader's progress
through them. The server works a group out as a whole, as it does a series
(`GET /groups/{id}/overview`), so every client continues a volume the same
way.

### The web app, next (W1 to W6)

Written 2026-11-18: what the web app needs before the AI work, and
nothing that breaks the rules above.
These steps are lettered so that iteration 6 stays the MCP server, as the
decision records call it. They come before it: the owner decided so on
2026-11-18. W6 is built before W5, the reader coming last of the six: the
owner decided that on 2026-11-18 too, once W4 was in.

What Binge already does well: a series with several volumes shows its
volumes and its one-shots as groups, where one flat list would say
"Issue #1" once for every volume. And progress is keyed to a book's pages,
not to a record of a file.

#### W1. The frame and home

Built so far (2026-11-18): a bar at the top of every page with the name,
the search box, the settings and who is signed in, and the settings as
pages of their own with their own menu. Home has its four rows, each one
line of covers that goes on sideways: `GET /up-next` and
`GET /recently-updated` are new. "Remove from this row" is on the cards
of "Up next" since W2 (`DELETE /up-next/{series_id}`).

- Home keeps "Continue reading" for the books the reader is inside.
- Home gains "Up next": for each series where the reader has finished a
  book, has none open, and has books left, the first unread book in
  reading order, newest reading first.
- Two things on top of that. A series the reader has not
  touched for 30 days leaves the row, and comes back for 7 days when a
  book is added to it. "Remove from this row" is forgotten the next time
  the reader reads from the series.
- Home gains "Recently updated": series that got a book, newest first,
  apart from "Recently added", which stays a row of books.
- The server works each row out, as it does a series, so every client
  shows the same rows.

- A heading that repeats what the frame already says goes: "Home" over
  Home, "Books" over the only grid of a page, "Search" over the search,
  "Settings" over a page of settings, and a library's name where the menu
  beside the page shows it (a phone keeps it). Each stays in the page for
  a screen reader. Done 2026-11-18. A library's folder is now only in the
  settings, and what can be done to a library is in a menu on its page.
- A row's title leads to the whole of it, and the reader chooses which
  rows Home shows and in which order.

**Done when:** finishing an issue puts the next one on Home without
opening the series.

#### W2. Read state, and acting on many

Built (2026-11-18): everything below. A reading list's cover comes with
the lists, in W3. One cover is kept for a book or a series, not several to
pick from. The owner chooses it: a picture from the menu of the
book's or the series' page, or a page from the reader's menu.
`POST /read-marks` marks books, groups and series read or unread in one
call, series and groups carry `unread_count`, every card and page has its
menu, and cards are picked with a tick, Shift, or a resting finger, with
the bar at the bottom. The record of reading sittings that W6 asks for is
kept from now on (`sitting`). Marking a whole series or group asks no
question first.

- Mark read and unread on a book, a group and a series. Marking a group or
  a series writes each of its books.
- A menu on every card and page for what can be done with it.
- Selecting many: a tick on the card, shift-click for a range, a long
  press on a touch screen, and a bar that stays in sight with what can be
  done to all of them. Once one card is selected, a click on a cover
  selects.
- A series card and a group card say how many books are unread.
- The cover of a book, a series or a reading list can be chosen: any page
  of a book, or a picture that is given. The choice lives in the data
  directory.
- No "read" button over the cover on hover: it does not exist on a
  touch screen, and it is easy to hit by mistake.

**Done when:** a volume is marked read from its card, and ten books picked
on a page are marked unread in one go.

#### W3. Reading lists

The biggest gap: the owner reads from reading lists.

Built (2026-11-18): everything below. A list belonged to the server
([ADR 0014](docs/adr/0014-reading-lists-belong-to-the-server.md)): the
owner made, imported and changed lists, and everyone read them. Step P1
made a list its maker's. The import
keeps nothing until the list is made: the file is read into a plan, a fix
is kept as a rule, and the file is sent again. "Add to a reading list…" is
in every menu and in the bar of what is picked. Also done here: a book of
the annuals is called "Annual #1", since its number counts the annuals.
Left for later: several CBL files in one go, a page that lists the rules
kept, and the next book of a series in the reader (W5).

- A reading list is a name, a summary, and books in an order. The books
  are held by their identity, so a list keeps its books when files move.
- The page of lists: four covers together as one, the span of dates of
  its books, how many books.
- The page of a list: one row per book with its cover,
  number, title, series, writer, penciller, date, pages and the reader's
  progress; one button that goes on where the reader stopped; the order
  changed by dragging, or by typing a position on a phone.
- Reading from a list follows the list: the next book after the last page
  is the list's next, not the series'. The reader's address carries the
  list.
- Import of a CBL file as a dry run. Each entry is matched to a series,
  its volume and a number, trying in turn the exact name, the name with
  the volume, the name without its article, and the name without words
  such as "TPB" or "Omnibus". The table says for each
  entry what it matched and why, the misses are fixed by hand, and
  nothing is made until the owner confirms. A fix is kept as a rule for
  the next import.
- The page of a book says which reading lists it is in.
- The routes are the ones the MCP tools will call, so an AI client can
  make a reading list too.

Decided while building it: a list belongs to the server, not to a
person, and hides the books a reader may not see.

**Done when:** the CBL of a crossover is imported, its misses fixed, and
reading it goes from one series to the next without leaving the reader.

#### W4. Finding things

Built (2026-11-18): everything below. Two things were decided while
building it. Long lists are read a part at a time, by offset, so that a
library has page numbers and letters to jump by
([ADR 0015](docs/adr/0015-long-lists-are-read-a-part-at-a-time.md)): a
library shows 120 series to a page. And a person is the names they go by
([ADR 0016](docs/adr/0016-a-person-is-the-names-they-go-by.md)): the
server makes one person of spellings that differ only in capitals, accents
or punctuation, and the owner merges the rest. A credit on a series or a
book leads to its library narrowed by that person, and from there to the
person's own page; in a reading list, whose books are of several
libraries, it leads to the person's page at once. Left for later: the MCP
tools that propose names and merges, a page that lists every person, and
narrowing by more than writers and pencillers.

- A library can be sorted (name, added, year, last read, unread first,
  random) and narrowed: unread, in progress or read; publisher; year;
  writer. Each value can be asked for or ruled out. What is
  chosen is in the address, so it can be reloaded and shared.
- A bar of letters that jumps through the library, with the letters that
  have no series greyed, counted under what the library is narrowed by.
- The publisher, the year and each credit on a series or a book lead to
  the library narrowed by them.
- The search box shows what it finds as it is typed, grouped as series,
  books, reading lists and people; Enter still leads to the page with
  everything.
- A page for a writer or a penciller: their series and books.
- Paging of lists is decided here, since this is where long lists arrive.
- The size of the covers is the reader's: a slider, one value for the
  whole app, kept in the browser, not shown on a phone. Built
  2026-11-18: it is in the bar at the
  top, on windows from 1024 px, from 6 to 13 rem. Since W6 the server
  keeps it for each person. Covers are smaller
  until one is chosen: six in a row where there were five, and eight at
  the smallest.
- A person has other names, and two people can be made one: files spell
  the same writer several ways, some with a number after the name.
  They are kept in the data directory, and an AI client can propose them
  through MCP.
- A list of the books the library holds twice. Binge knows them by
  their pages, not by a hash of the file, so a book that was retagged is
  found too.

Left for later: genres, tags, characters and age ratings. The server does
not read them from the files yet.

**Done when:** "unread Vertigo from the nineties" is three clicks and an
address that can be bookmarked.

#### W5. The reader

Built (2026-11-19), after W6: everything below. Decided while building
it. The reader follows where the book was opened from: a reading list,
the book's group when it was opened from the group's page, and otherwise
its series from one group to the next. At the end of what it follows it
says so. A finished book, and one left alone for 90 days, asks where to
start, with a look that records nothing as one of the answers; "Read
again" and the next book opened from inside the reader ask nothing. A
reading that is not recorded says so in the bar at the top, and can be
recorded from the page it is on. What is changed in the reader's settings
while reading a series is kept for that series alone, for the latest 60
series, and can be made the usual or let go; the server keeps both for
each person ([ADR 0018](docs/adr/0018-what-a-person-chooses-is-kept-as-keys-and-values.md)),
so they follow the person to another browser. A small picture of a page
is made each time it is asked for and kept by whoever asked, not on the
server's disk. A kept page hangs on the book's identity like progress,
one person keeps 5,000 at most, and a kept page opens for a look. The
pace is the seconds a page took over the reader's latest 100 sittings
that went between 3 and 300 seconds a page; until those turned 20 pages
the time left is told at the fixed rate, as before.

Left for later: keeping pages and the pace as MCP tools, a note on a kept
page, and a pace that knows a jump through the book from reading it (a
sitting records its first and last page, so a long jump counts as pages
read fast).

It already has one and two pages, fit modes, both directions, keys, tap
zones and preloading.

- At the last page, the next book: a first press says which book comes
  next, a second one opens it. "Next" follows where the reader
  came from: the series, the group or the reading list.
- Reading without recording, for a look that should not move the
  progress.
- Settings remembered for a series once they are changed while reading
  it, over the browser's own.
- A strip of page thumbnails to jump with.
- Opening a finished book, or one left untouched for a long time, asks
  whether to start again.

Left out: endless scroll for webtoons. This is a comic server.

- A page can be kept: a mark on it while reading, a place that shows all
  the kept pages, by series.
- The time left in a series at the reader's own pace, measured from what
  they read, not from a fixed rate.

**Done when:** a volume is read from first issue to last without leaving
the reader.

#### W6. Rows of one's own, activity, and what was read

Built (2026-11-18): everything below but two things. The notice of a
newer version is left out: it asks an address outside the server, there
is no place yet where releases are published, and which account that
place is under is the owner's to say first; it came with the update
button of iteration 5. And the MCP tool that makes a
view comes with step 6: until then an AI client makes one with the
owner's API token and `POST /views`, which is the route the tool will
call, and that is how the step's "done when" was checked.

Decided while building it. A view is a library narrowed and ordered, kept
under a name; the owner makes them and everyone who reads the library
sees them
([ADR 0017](docs/adr/0017-a-view-is-a-library-narrowed-and-kept-by-name.md)).
What a person chooses in an app is kept as keys and values the server
does not read
([ADR 0018](docs/adr/0018-what-a-person-chooses-is-kept-as-keys-and-values.md)):
the web app keeps the size of the covers and the arrangement of Home so.
Every row of Home can be moved or put away, the libraries always end the
page. A scan that changed a library leaves a record of the files that
arrived, moved, went and came back, the latest 50 of a library; its first
scan records everything as arrived. A copy of the database is made every
day into `backups/` in the data directory unless the owner turns that
off, and the latest 7 are kept. Who is reading now is in the list the
icon of the bar opens, for the owner. What was read is at `/history`, a
year at a time.

Left for later: views of a member's own and views over more than one
library; putting a copy of the database back from the app (it is done by
hand, with the server stopped); and the reader's own choices (fit, pages
side by side, direction), which stayed in the browser until W5.

- A saved filter can be a row on Home or an entry in the menu: one
  thing, two places. The owner, or an AI client through MCP, makes them.
- An icon in the bar at the top for what the server is doing: scans
  running, and files that could not be read.
- A page of what the reader has read: a calendar of reading days, books
  and pages by month, most read writers. It needs a record of
  each reading session, which progress alone does not give; the record
  should start with W2 so there is history to show.

- What the scans changed, as a list to look back on: books that arrived,
  moved, went and came back. The files that could not be read can be set
  aside one by one once seen.
- What a person chooses in the app, such as the rows of Home and the size
  of covers, kept by the server so it follows them to another browser.
- For the owner: who is reading what now, a copy of the database made on
  a schedule into the data directory, and a notice when a
  newer version is out. That last one asks an outside address, so it is
  off until the owner turns it on.

**Done when:** a row made by an AI client appears on Home.

#### Left out on purpose

- Removing duplicate pages and importing files: both write inside the
  library.
- Sync with Kobo and KOReader, OPDS, and reading-tracker sites: no clients
  but Binge's own.
- Ratings, reviews and recommendations from outside services.
- Editing what a file says about itself. Binge edits the layout.
- Sending books by mail to an e-reader, notes and highlights in text
  books, and other servers as entries in the menu.

Two ideas belong to the MCP server and to page maps, not to the web app:
how series relate to each other (sequel, prequel, spin-off), which an AI
client could fill in, and a table of contents a reader makes for a book,
which for a collected edition is the list of the issues inside it.

### Lists and collections of one's own (P1)

Written 2026-11-20, with the owner, who put this step before the MCP
server. It is lettered for the same reason as the web app's steps:
iteration 6 stays the MCP server. What is written here is what is being
built; it grows as the parts land.

#### P1. Reading lists of one's own, and sharing

Built (2026-11-20): everything below
([ADR 0021](docs/adr/0021-a-reading-list-is-its-makers-to-share.md), which
replaces 0014).

- A reading list belongs to the person who made it, and any member makes
  one, adds to it and imports one from a CBL file. The lists that were
  there became the owner's, shared with everyone.
- A list is shared with the people chosen, or with everyone on the
  server. It stands as "Only you", "Shared with Ana and Luis" or "Shared
  with everyone", and the one it is shared with sees whose it is. Only the
  one who made it changes it or who it is shared with, and anyone shares
  what is theirs. On a server of one person nothing about sharing is
  shown.
- A list is shown without the books of libraries the person does not
  read, and its count is of what they see.
- What a person fixes by hand in a CBL import is kept as their own rule.

#### P1. Collections

Built (2026-11-20): everything below
([ADR 0022](docs/adr/0022-a-collection-holds-series-and-is-shared-as-a-list-is.md)).

- A collection is a name, a summary and series picked by hand from any
  library, shown by name. It belongs to the person who made it and is
  shared as a reading list is.
- It holds the record of each series: a series stays in it under another
  name and leaves it when it is deleted.
- "Add to a collection…" is in the menu of a series and in the bar of what
  is picked while only series are picked.
- Search finds a collection by its name. A series' page says which
  collections it is in and which reading lists hold a book of it.

#### P1. Want to read

Built (2026-11-20): everything below
([ADR 0023](docs/adr/0023-want-to-read-is-a-persons-own-and-leaves-out-what-was-read.md)).

- Each person has one list of the series they want to read. It is theirs
  alone and is not shared.
- A series is put there from its menu, and several at once from the bar of
  what is picked. It is taken off from the same menu.
- It is a row of Home and a page of its own, the last put there first.
- A series read to the end is left out, and is back by itself when it has
  a book that was not read.

### Later, in outline only

These follow the web app steps and P1 above.

6. MCP server: read-only library tools, the operations that change the
   layout, page images, the current reading session.
7. Page maps and tracks: storage, region detection, the tools an AI client uses
   to submit a translation, lettering, track switching in the reader.
8. Organization tools with write access and dry runs.
9. Binge+ cloud: accounts, relay for remote access.

### Last. The Binge reader connects

Moved to the end on 2026-11-17: the server gets its features first, and the
reader connects to a server that already has them.

Work in the iOS reader repository, not here.

- "Binge Server" as a third server type: connect, browse, read, resume.
- `/api/v1` is frozen at this point. Changes after it are additive.

**Done when:** an issue started in the browser resumes on the iPad.

## Open decisions

None of these block iteration 0.

- **Licence.** Decided on 2026-11-19: AGPL, version 3 only, and no code
  from outside for now
  ([ADR 0019](docs/adr/0019-the-server-is-under-the-agpl.md)).
- **What is free.** Whether plain reading in the web UI stays free or needs
  Binge+.
- **Names.** Decided on 2026-11-19: the repository is
  `bingecomics/binge-server` and the binary is `binge-server`.
- **The frame.** Today it is a bar at the top and a menu at the side.
  Another way: Home, Browse and Lists in the bar and no menu at the side,
  closer to Netflix, with a bar of tabs at the bottom on a phone.
