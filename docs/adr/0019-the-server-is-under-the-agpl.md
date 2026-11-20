# 0019. The server is under the AGPL

## Status

Accepted on 2026-11-19, decided by the owner before the repository is made
public.

## Context

[ADR 0001](0001-free-local-server-for-binge-clients-only.md) says the local
server is open source and free to run, and left the licence open. The
repository is about to be public, and a public repository with no licence
gives nobody the right to run or change the code.

Two things about Binge shape the choice:

- Binge+ is a paid cloud service next to the free server. A licence that
  lets anyone take the server and offer a changed one as a closed service
  works against that.
- The readers and the Binge+ cloud are separate programs, written by the
  owner, and are not open source.

## Decision

The server is licensed under the GNU Affero General Public License,
version 3 and no later version (`AGPL-3.0-only`).

- It covers everything in this repository: both crates, the web app that
  is embedded in the binary, the scripts and the documents.
- The licence text is in `LICENSE`. The package manifests name it.
- Code from outside is not taken for now. A pull request from someone else
  is not merged until there is an agreement for them to sign that lets the
  owner keep full rights over the whole.

## Consequences

- Anyone may run the server, read it and change it, for free.
- Whoever gives a changed server to others, or lets others use one over a
  network, has to offer them its source under the same licence.
- The owner holds the copyright of all of it and is not bound by the
  licence: the same code can go into a product that is not open.
- **What Binge+ sells cannot be a check inside the server.** Anyone may
  remove such a check and say so. What is paid for has to be something the
  server cannot do alone, in the cloud or in the readers.
- The readers and the Binge+ cloud talk to the server over its API. They
  are not derived from it, and the licence does not reach them.
- Every dependency that ships has to allow it. On the day of the decision
  all of them did: the Rust crates in the binary are under MIT, Apache-2.0,
  BSD, Zlib, Unicode and similar licences, one under MPL-2.0, and the web
  app's are under MIT, ISC and the Unlicense. A new dependency under a
  licence that does not go with the AGPL (a GPL-2.0-only one, or one that
  is not open source) cannot be added. Nothing checks this yet; a check in
  CI is to come with the release builds.
- "Version 3 only" means a later version of the licence does not apply by
  itself. The owner can still choose one.

## Alternatives considered

- **MIT or Apache-2.0.** The most freedom for others,
  including to build a closed product or service on the server. Turned
  down for that reason.
- **GPL-3.0.** It asks for the source only when the program
  is given to someone, not when it is run for them as a service.
- **A source-available licence** that forbids competing use. It is not open
  source, which ADR 0001 says the server is.
