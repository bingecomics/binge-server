# 0007. Refuse requests for unknown host names

## Status

Accepted on 2026-11-16.

## Context

The server has no accounts until iteration 3. Until then it relies on
listening only on the machine it runs on.

That does not stop a web page. A page can point its own host name at
`127.0.0.1` or at an address on the home network (DNS rebinding). The
browser then treats the server as part of that page's site and lets the
page's scripts read it. From iteration 1 that would expose the list of
libraries, folder paths and page images, and allow adding a library.

Every such request carries the page's host name in its `Host` header.

## Decision

The server answers a request only when the host it names is one of:

- an IP address;
- `localhost`, or a name ending in `.localhost`;
- a name the owner allowed with `--allowed-host` or `BINGE_ALLOWED_HOSTS`.

Any other name gets a 403 with the code `host_not_allowed`. A request with
no `Host` at all is answered, because browsers always send one.

The check stays when accounts arrive. It costs almost nothing and still
protects the login and setup pages.

## Consequences

- Opening the server by IP address or as `localhost` works with no setup.
- Opening it by a name, such as `nas.local` or a name behind a reverse
  proxy, needs that name allowed once. The error message says so and names
  the option.
- A page on another site cannot reach the server by rebinding, on this
  machine or elsewhere on the network.
- The check does not replace accounts. Anyone on the network who can reach
  the port can still use the server until iteration 3.

## Alternatives considered

- **Rely on listening on `127.0.0.1` only.** That is what rebinding gets
  around.
- **Check the `Origin` header.** Browsers leave it out of ordinary page
  loads and image requests, so it cannot protect reads.
- **Require a token for everything now.** That is accounts, planned for
  iteration 3. Until then the web UI would have nowhere to get the token.
