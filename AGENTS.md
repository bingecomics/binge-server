# Binge Server

A small comic server in Rust with a React UI, built for the Binge readers. One
binary, one SQLite file, the web UI embedded.

Read these before changing anything that is not local to one file:

- [PLAN.md](PLAN.md): the iterations, what is built and what comes next.
- [docs/architecture.md](docs/architecture.md): how the parts fit together.
- [docs/adr](docs/adr/README.md): the decisions behind it and why.

## Commands

| Command | What it does |
| --- | --- |
| `scripts/check.sh` | Format, lints, tests, generated files, types. Run it before saying work is done |
| `scripts/dev.sh` | Server plus Vite with hot reload. Takes no arguments; configure with `BINGE_PORT` and the other environment variables |
| `scripts/build.sh` | Builds the UI, then `target/release/binge-server` with the UI embedded |
| `scripts/gen-api.sh` | Regenerates `api/openapi.json` and `web/src/api/schema.d.ts` from the Rust code |
| `scripts/docker.sh` | Builds the Docker image and saves it as a file under `target/docker`. `BINGE_PLATFORM=linux/amd64` builds for an Intel NAS |

Needs Rust and pnpm. `rust-toolchain.toml` names the Rust version, and
rustup installs it by itself.

## Product rules

These come from the owner and are not open for redesign in passing. Each has a
decision record.

- The only clients are Binge's own. Do not add compatibility with other
  comic servers or with OPDS.
- The server does no AI inference and Binge does not pay for any. AI work is
  done by the user's own AI client through MCP tools the server provides.
- The server never writes, renames or deletes inside a library folder. Features
  that reorganize files must ask for write access explicitly and show a dry run
  first.
- A book is identified by its page content, not its path. Progress and anything
  derived from a book attach to that identity.
- Everything the server derives or stores lives in its data directory, never in
  the library.
- The local server stays installable as "download, run, open the browser": one
  binary, no other service to run.

## Code rules

- `crates/core` holds domain logic and storage and has no HTTP in it, so a
  later MCP server can reuse it. `crates/server` holds the HTTP API, the
  embedded UI and the binary.
- Every API error is an `ApiError` with an `ErrorCode`. Handlers return it;
  do not answer errors as plain text or with another shape.
- In `crates/core`, an error's `Display` is for the log and its
  `for_people()` is the sentence shown to the person. Write that sentence
  once, there, not again in the API or the web app.
- The API contract is generated from the Rust handlers. After changing a
  route or a type it exposes, run `scripts/gen-api.sh` and commit both
  generated files. Never edit these by hand:
  - `api/openapi.json`
  - `web/src/api/schema.d.ts`
  - `web/src/routeTree.gen.ts` (written by the router plugin when Vite runs)
- `/api/v1` freezes when the iOS reader connects (the last step of the
  plan). Until then it may change; after that, changes are additive.
- A migration that has shipped in a release is never edited. Add a new one.
- The web app calls the server only through the typed client in
  `web/src/api/client.ts`.
- Colours, radii and type sizes in `web/src/styles.css` mirror the reader's
  design tokens. Use the theme names (`bg-surface`, `text-fg-muted`), not
  literal colours.
- TypeScript stays on 5.9 until the API client generator supports
  TypeScript 7. Lint and formatting on the web side use Biome
  (`pnpm -C web format`).
- `unsafe` is forbidden in the workspace, and clippy warnings fail the checks.

## Verifying

- `scripts/check.sh` must pass.
- For a change to how the server answers requests, also run the built binary
  and check the real response. For a UI change, look at it in a browser in
  light and dark and at phone width.
- Say what you verified and what you did not. A passing test suite is not a
  check of the running server.

## Data and privacy

- Never commit comics, covers, or any file from a real library. Tests create
  the archives they need.
- Never put keys, tokens or passwords in source, fixtures, command arguments or
  logs.
- `data/` is the development data directory and is ignored by git.

## Git and publishing

- Conventional commits (`feat:`, `fix:`, `docs:` and so on), lower case,
  imperative, subject of 50 characters or fewer.
- Work on a feature branch. Commit only when asked, never push on your own, and
  never skip hooks.
- Binge is the owner's personal product. Anything published or deployed (a
  remote repository, release, container registry, domain, cloud account) uses
  the owner's personal accounts only, never an employer's or another
  organization's, not even as a fallback. Ask which account before the first
  use of any of them.
