# 0001. Free local server, for Binge's own clients only

## Status

Accepted on 2026-11-15, decided by the owner.

## Context

Binge started as a reader app for other people's comic servers. A reader
alone is hard to sell: those servers have free readers already. The
features that would set Binge apart (translation tracks, reading with an AI
client, automatic organization) need things those servers do not offer, so
Binge needs a server of its own.

That leaves two questions: who pays for what, and which clients the server
talks to.

## Decision

Follow the model Plex uses.

- The local server is open source and free to run.
- Connecting it to Binge needs a paid subscription, Binge+.
- The server speaks only its own API, for Binge's own clients: the web UI in
  this repository and the Binge readers. It does not implement the API of
  any other comic server, and it does not implement OPDS.
- Binge does not host users' comics. The files stay on the user's machine.

## Consequences

- No third-party reader works with this server, and that is intended. The
  server is a reason to use the Binge reader, not a stand-in for another
  server.
- The API can be designed for the features Binge wants, without matching
  another product's data model.
- People who run another comic server today have to switch servers to get
  the new features. Their files do not need to change, because the server
  reads the folders as they are.
- Binge's cloud costs stay small: accounts and a relay, no storage of comics
  and no traffic for page images.

Still open, and not blocking:

- exactly what Binge+ unlocks, and whether plain reading in the web UI is free;
- how the reader app fits in the subscription.

The licence was open here too. It was decided on 2026-11-19: see
[ADR 0019](0019-the-server-is-under-the-agpl.md).

## Alternatives considered

- **Speak another server's API as well**, so existing readers work from day
  one.
  Turned down by the owner: the point is that people use the Binge reader and
  pay for it.
- **Host the comics as a service.** Turned down: storage and traffic would be
  the main cost, and hosting other people's comic files brings legal exposure.
