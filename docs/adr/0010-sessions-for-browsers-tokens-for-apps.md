# 0010. Sessions for browsers, tokens for apps

## Status

Accepted on 2026-11-16. Built in iteration 3.

## Context

With accounts ([0009](0009-first-run-setup-makes-the-owner.md)), every
request has to show who makes it. Two kinds of client make requests: the web
UI in a browser, which loads page images with plain `<img>` elements, and
apps such as the iOS reader, which should not keep a password.

The server usually runs on a home network over plain HTTP, opened by IP
address. It has no mail, no second factor, and nobody to call when a secret
leaks. Whatever it stores can end up in a backup of the data directory.

## Decision

**Passwords.** A password is stored as an Argon2id hash with its own salt
and the library's default cost. It is at least 8 characters; nothing else is
asked of it.

**Sessions, for browsers.** `POST /sign-in` checks a name and password and
sets a cookie, `binge_session`, holding 256 random bits. The cookie is
`HttpOnly`, `SameSite=Lax` and limited to `/api`. A session ends when it is
signed out, when its user's password changes, or after 30 days without use.

**Tokens, for apps.** A token is 256 random bits behind the prefix `binge_`,
sent as `Authorization: Bearer`. It is made and revoked under Settings, has a
name and a "last used" time, and does not expire by itself. Its secret is
shown once, when it is made.

**Only hashes are stored.** The database keeps a BLAKE3 hash of each
session's and token's secret, never the secret. A fast hash is enough,
because the secrets are random and cannot be guessed. Passwords and secrets
are not logged: the request types that carry them do not print them, and the
request log has no headers or bodies in it.

**Only `Bearer` counts as a token.** An `Authorization` header of another
scheme, such as the `Basic` a proxy in front of the server asks browsers
for, is as if it were not there: the request stands on its cookie.

**A token cannot manage accounts.** It can do what its user can, except
change the password, make or revoke tokens, and manage users. Those take a
session, which takes the password. A token that leaks can be revoked and
cannot have made itself a successor.

**One check, around the routes.** A layer in `crates/server/src/api/auth.rs`
wraps every route except `/health`, `/setup` and `/sign-in`, and refuses a
request that shows nothing good with 401 and the code `unauthorized`.
Handlers get the caller from it. A test walks every route in the API
description and asks for the 401, so a route added later is covered without
anyone remembering to.

**Guessing is slowed for everyone at once.** At most four passwords are
checked at a time, and a wrong one holds its place for a second. More
attempts than that are answered 429 at once. A name nobody has takes as long
to refuse as a wrong password. A check keeps its place until it and its
wait are over; closing the connection does not give the place back.

**Changes must come from the server's own pages.** A request that changes
something and brings no token is refused when the browser says another
origin made it (`Sec-Fetch-Site`, or `Origin` against `Host`). This is on
top of `SameSite`, which does not tell apart two ports of one machine. For
the same reason every answer forbids showing it in a frame
(`X-Frame-Options` and `frame-ancestors`), so a page on another port cannot
put the signed-in app under someone's clicks.

## Consequences

- The cookie is not marked `Secure`, because the server speaks plain HTTP.
  On the home network, someone who can read the traffic can read the cookie,
  as they can read the password when it is typed. Remote access is meant to
  go through a TLS proxy or, later, Binge+.
- Without `Sec-Fetch-Site`, which browsers send only over HTTPS or to
  `localhost`, the origin of a request is compared with the `Host` header.
  A proxy in front of a plain-HTTP server has to pass `Host` on as the
  browser sent it, port included, or every change is refused, signing in
  too. The README says so.
- Names are compared in lower case, by a column of their own, because
  SQLite's own folding knows only A to Z. Two spellings of one letter that
  differ in how Unicode composes them are still two names.
- Every request costs one indexed lookup to find its session or token.
  "Last used" is written at most once a minute, not on every page image.
- An app gets its token in one of two ways: the person makes one under
  Settings and enters it, or the app signs in with the name and password,
  makes itself a token, and signs out. A single request that trades a
  password for a token would be shorter and can be added without changing
  what is there.
- Changing a password ends sessions and leaves tokens. To cut off an app,
  revoke its token. The owner cannot see or revoke a member's tokens; to cut
  a member off entirely the owner removes them.
- The limit on guessing is shared: someone hammering the sign-in page can
  make signing in fail with 429 for everyone else while they do it. People
  already signed in are not affected.
- A browser older than `Sec-Fetch-Site` behind a proxy that rewrites `Host`
  is refused changes. Such a proxy should pass `Host` through.
- The API description (`/api/v1/openapi.json`) now needs signing in, like
  everything else. The same file is in the repository.

## Alternatives considered

- **One kind of credential.** A browser cannot attach a token to an `<img>`
  request, and a token in page scripts can be stolen by any script. An app
  with only a cookie would have nothing a person can name and revoke.
- **Signed, stateless tokens (JWT).** No lookup per request, and no way to
  revoke one before it expires without the lookup coming back. Revoking is
  the point.
- **Store the secrets, or encrypt them.** Then a copy of the database is a
  set of working credentials.
- **Limit guessing per address or per name.** Per address fails behind a
  proxy, where everyone has one address. Per name lets anyone lock a person
  out by guessing at their name.
- **Let tokens do everything their user can.** Simpler, and a leaked token
  could then change the password and lock its owner out.
