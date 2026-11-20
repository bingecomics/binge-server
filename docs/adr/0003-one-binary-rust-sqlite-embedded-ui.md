# 0003. One binary: Rust, SQLite, embedded web UI

## Status

Accepted on 2026-11-15.

## Context

The server runs on whatever people have at home: a laptop, a small NAS, a
mini PC. Most of them are not system administrators. Each extra thing to
install or keep running is a reason to give up.

Comparable servers need a runtime, such as the JVM or .NET, and are
commonly installed through Docker.

## Decision

- The server is written in Rust and ships as one executable.
- HTTP uses axum on tokio.
- All state is one SQLite file, accessed through sqlx, with migrations that
  run at startup.
- The web UI is a React app built by Vite and embedded in the executable.
- The code is a workspace of two crates. `binge-core` holds domain logic and
  storage and has no HTTP. `binge-server` holds the API, the UI and the
  binary.

Installing is: download, run, open the browser.

## Consequences

- No runtime, database server or web server to install. A Docker image is a
  convenience, not a requirement.
- SQLite allows one writer at a time. That is enough for a household, and it
  rules out running several server processes on one database.
- The UI must be built before a release build of the server. The build
  script embeds a placeholder when it is missing and warns in release builds.
- UI and server are always the same version, so the UI never has to cope
  with an older or newer API.
- Backing up the server's state means copying one folder.
- Rust is slower to write than Kotlin or TypeScript. The return is low
  memory use and fast image handling on small machines, and one language for
  the server and the later MCP server.

## Alternatives considered

- **PostgreSQL.** Better for many writers, but a service to install and
  maintain. Not justified for one household.
- **UI served separately** from the API. Two things to deploy and to keep at
  matching versions.
- **Kotlin or .NET**, like the existing servers. A runtime to install and a
  larger memory footprint.
