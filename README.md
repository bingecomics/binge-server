# Binge Server

A small comic server in Rust with a React UI, built for the Binge readers.
One binary, one SQLite file, the web UI embedded.

This is early work. Today you can point the server at folders of CBZ, CBR
and PDF files, browse their series and covers in the web UI, search them,
read them there, and pick a book up again where you left it. Everyone who
reads has an account, with their own place in each book, and apps connect
with tokens.

- [PLAN.md](PLAN.md): what comes next.
- [docs/architecture.md](docs/architecture.md): how the parts fit together.
- [docs/adr](docs/adr/README.md): the decisions behind it.
- [AGENTS.md](AGENTS.md): rules for anyone, person or AI agent, changing
  the code.
- [docs/releasing.md](docs/releasing.md): how a release is built and
  published.

## Run

Needs Rust and pnpm. `rust-toolchain.toml` names the Rust version, and
rustup installs it by itself.

```sh
scripts/build.sh
target/release/binge-server
```

Then open <http://127.0.0.1:8686>. The first visit asks for a name and a
password: that account is the owner, who adds libraries and other users.

| Option | Environment | Default | Meaning |
| --- | --- | --- | --- |
| `--data-dir` | `BINGE_DATA_DIR` | The platform's application data folder | Where the database and everything else the server writes lives |
| `--bind` | `BINGE_BIND` | `127.0.0.1` | Address to listen on |
| `--port` | `BINGE_PORT` | `8686` | Port to listen on |
| `--allowed-host` | `BINGE_ALLOWED_HOSTS` | None | Extra host name the server answers to, such as `nas.local`. Repeat it, or separate names with commas |

The server listens to this machine only. To read from other devices at
home, make the owner first, then start it with `--bind 0.0.0.0` and open
`http://<this machine's address>:8686` on the other device. It speaks plain
HTTP: do not put it on the internet without a TLS proxy in front.

The owner's password cannot be recovered yet, so keep it somewhere.

It answers requests addressed to an IP address or to `localhost`. To open it
under another name, allow that name with `--allowed-host`; other names get a
403. This stops a web page from reaching the server through a host name of
its own.

Behind a reverse proxy, have the proxy pass the `Host` header on as the
browser sent it (in nginx, `proxy_set_header Host $http_host;`) and allow
that name with `--allowed-host`. The server compares it with where a request
says it comes from, and without it refuses every change, signing in
included.

### With Docker

On a NAS, or anywhere Docker already runs, use the image instead: see
[docs/docker.md](docs/docker.md). Your comics are mounted read-only and the
server's data goes to a folder you choose.

### Is it up?

`binge-server health` asks the server running on this machine and exits
with 0 when it is well. Give it the same `--bind` and `--port`, or the
same environment, as the server. The Docker image uses it as its health
check.

## Develop

```sh
scripts/dev.sh      # server plus Vite with hot reload; open the Vite address
scripts/check.sh    # format, lints, tests, generated files, types
```

`scripts/dev.sh` keeps its database in `./data`. It takes no arguments; set
the environment variables from the table above instead, for example
`BINGE_PORT=9000 scripts/dev.sh`.

After changing the API in Rust, regenerate the description and the typed web
client, and commit both:

```sh
scripts/gen-api.sh
```

## Layout

| Path | Contents |
| --- | --- |
| `crates/core` | Domain logic and storage. No HTTP. |
| `crates/server` | The HTTP API, the embedded UI and the `binge-server` binary |
| `web` | React UI (Vite, TanStack Router and Query, Tailwind) |
| `migrations` | Database schema, applied at startup |
| `api/openapi.json` | The API contract, generated from the Rust code |
| `docs` | Architecture and decision records |

## Licence

Copyright (C) 2026 Flavio Oliveri

Binge Server is free software: you can redistribute it and modify it under
the terms of the GNU Affero General Public License, version 3, as published
by the Free Software Foundation. It comes with no warranty. The full text is
in [LICENSE](LICENSE), and
[the decision record](docs/adr/0019-the-server-is-under-the-agpl.md) says
why.

Pull requests from outside are not taken for now.
