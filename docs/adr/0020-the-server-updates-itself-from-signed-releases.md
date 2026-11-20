# 0020. The server updates itself from signed releases

## Status

Accepted on 2026-11-19, decided with the owner.

## Context

A server on a NAS is updated by someone who has to open a terminal, fetch a
new image and recreate a container. Most people who run a comic server at
home do that rarely, and run an old version for months. The owner wants a
button in the web app instead.

Three things make that harder than replacing a file:

- In Docker, which is where most servers run, a container cannot change its
  own image. What it can write is the data directory.
- A server that fetches a program and runs it is the most dangerous thing
  the server does. Whoever can put a file where the server looks could run
  anything on the machine that holds the comics.
- Looking for a newer version asks an address outside the server, and the
  server otherwise never does that.

## Decision

**The installed binary is never replaced.** A version the server fetches is
kept in `updates/<version>/` in the data directory. A server that starts
looks there first and, when a version newer than itself is kept and proven,
runs it in its own place: the same process, with the program replaced. In
Docker the image's binary starts the newer one from the volume.

**A release is proven by a signature, and proven again at every start.** A
release has a file, `update.json`, that names its version and, for each
system, the archive to fetch and the checksum of the binary inside. The
build that makes the release signs that file with a key only Binge has.
The server has the public half built into it. Before a binary is run, at
the fetch and at every start after it, the server makes sure that one of
its keys signed the file and that the binary's checksum is the one the file
names. A version that fails is passed over and reported in the log, and the
server runs as the installed binary. It tells the owner, on the About page,
that a newer version is kept and was not started.

**Two keys are built in.** One signs releases and lives with the build, as
a secret of the repository. The other is a spare that was never there. If
the first is lost or stolen, releases are signed with the spare from then
on, and servers go on updating. Without the spare, every server out there
would have to be reinstalled by hand.

**The server only moves forward.** It fetches and starts a version only
when its number is higher than the one that runs. The number is inside the
signed file, so an old release cannot be passed off as a new one.

**The owner asks for everything.** Looking for a newer version is off until
the owner turns it on, and then happens once a day; the owner can also look
once, at any time. Fetching and starting a version happens only when the
owner presses the button. Nothing updates by itself. The button names the
version the owner was shown: when another came out since, the server
fetches nothing and shows that one.

**What is sent outside.** The requests go to GitHub, where the releases are
published. They carry the name and version of the server and nothing else.
This is the only part of the server that calls out
(`crates/server/src/update/fetch.rs`).

**A new version is tried before the server stops.** After the fetch the
server runs the new binary with `--version`. One that does not run on this
machine, or sits in a folder programs may not be run from, is refused while
the old server still runs.

**It can be turned off where the server is started.** With `--no-updates`
(`BINGE_NO_UPDATES=true`) the server never looks and never starts a version
from the data directory, for whoever decides the version where the server
is installed.

## Consequences

- An update is a button, in Docker too, and takes about as long as the
  download.
- The version that runs can be newer than the image or the binary that was
  installed. `/health` and the About page say the version that runs. The
  image's tag does not.
- A newer version brings the database up to date when it starts, and the
  installed binary cannot open that database any more. It does not have to,
  as long as `updates/` stays with the data directory: the two travel
  together. Removing `updates/` by hand after an update leaves a server
  that cannot start until a binary at least as new is installed.
- Installing a newer binary or image than what is kept makes the kept
  versions useless, and the server removes them when it starts.
- A release that does not start on someone's data is started again every
  time the installed binary starts, and the server stays down. The way out
  is to install a binary or image of a version newer than the kept one,
  which then runs in its place. Going back is not a way out once the
  database was brought up to date. So a release has to be tried on real
  data before it is tagged, and a bad one is followed by a fixed one, not
  withdrawn.
- What proves a kept version at a start is the binary that was installed,
  with the keys it was built with, for as long as it is installed. A key
  cannot be taken away from a server that is out there, and a new key
  reaches it only when a newer binary or image is installed. So after the
  first key is lost, releases stay signed with the spare for the servers
  installed before, and a new spare counts only for servers installed
  after it was added. Letting a kept version prove the next one with its
  own keys would lift this, at the cost of a chain of kept versions to
  keep and to trust. It was left out until a key has to change.
- The signing key is now something to guard. Whoever has it and can make a
  server fetch from them can run a program on that server. The key is in
  the repository's secrets and in the owner's own copy; the spare is in
  the owner's copy only.
- Windows servers do not update themselves: a running program cannot take
  its own place there. They say when a newer version is out, with a link.
  Releases have no build to update a server built by hand for another
  system either.
- A build for development does not update itself, so that working on the
  server never ends with a released build running in its place.
- The server depends on GitHub's release addresses
  (`/releases/latest/download/<file>` and `/releases/download/v<version>/<file>`).
  Publishing somewhere else later means a release that knows the new place
  first.
- A fork that publishes its own builds has to change the keys and the
  address, or its servers would update to Binge's builds.

## Alternatives considered

- **Replacing the binary in place.** The usual way outside Docker. It
  cannot work in a container, needs the binary's folder to be writable, and
  leaves nothing to go back to. Turned down for one way that works
  everywhere.
- **Having the server ask Docker for a new image.** It needs the Docker
  socket inside the container, which gives the server the whole machine.
- **Trusting the download because it comes over TLS from GitHub.** Then the
  account that publishes, and every certificate authority, could run a
  program on every server. With a signature, a release also has to be made
  by whoever holds the key.
- **Apple's signature as the proof.** Only the macOS binary has one, and
  Linux has nothing that checks it.
- **Checking the signature only when fetching.** The data directory can be
  written by whoever can write the volume. Checking at every start means a
  binary put there by someone else is never run.
