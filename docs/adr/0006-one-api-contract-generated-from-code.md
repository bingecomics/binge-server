# 0006. One API contract, generated from the code

## Status

Accepted on 2026-11-15. The typed error codes were added on 2026-11-16.

## Context

Three clients will talk to this server, written in three languages: the web
UI in TypeScript, the iOS reader in Swift and the Android reader in Kotlin.
If each one describes the API by hand, they drift apart from the server and
from each other.

## Decision

- The API lives under `/api/v1` and is described by one OpenAPI document.
- That document is generated from the Rust handlers and their types. It is
  not written by hand.
- It is committed as `api/openapi.json`. The web client's types are
  generated from it, and the Swift and Kotlin clients will be.
- The checks fail when the committed document or the generated web types no
  longer match the code.
- Every error has one shape, `ApiError`: a code from a fixed list and a
  message for people. Clients decide what to do from the code and the HTTP
  status, never from the message.
- `/api/v1` may change freely until the iOS reader connects in iteration 4.
  From then on it is frozen, and changes are additive.

## Consequences

- Changing a handler or a type changes the contract in the same commit, and
  the diff of `api/openapi.json` shows reviewers what clients will see.
- A change that breaks a client shows up as a type error in that client
  instead of a failure at run time.
- After the freeze, a breaking change means a `/api/v2`, not an edit.
- The web build depends on a generator for the typed client. That generator
  does not run on TypeScript 7 yet, which is why the web app stays on
  TypeScript 5.9.
- Whatever the framework answers on its own (a wrong method, a rejected
  body) has to be rewritten into `ApiError`. One middleware does that for
  everything under `/api`.

## Alternatives considered

- **Write the OpenAPI document first** and generate the server from it.
  The description would lead, but the handlers have to be written either
  way, and describing them where they are keeps one source instead of two.
- **Hand-written clients.** Three places to keep in step with the server by
  discipline alone.
