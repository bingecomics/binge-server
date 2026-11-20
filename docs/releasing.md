# Releasing

A release is a version of the server that people can download: a binary for
Linux, macOS and Windows, and the Docker image. GitHub builds all of it from
a tag. Nothing is built or uploaded by hand.

## What runs when

Two workflows live in `.github/workflows`.

**Checks** (`checks.yml`) runs on every push to `main` and on every pull
request:

- `scripts/check.sh`, the same checks as on a developer's machine.
- The licences of everything that ships: the Rust crates against
  `deny.toml`, the web app's packages with `scripts/check-web-licences.sh`.
  A dependency under a licence that does not go with the AGPL fails here
  ([ADR 0019](adr/0019-the-server-is-under-the-agpl.md)).

**Release** (`release.yml`) builds the files of a release. It runs in three
cases:

| When | What it does |
| --- | --- |
| A tag `v1.2.3` is pushed | Builds everything, signs the macOS binary and the file servers update by, pushes the image and publishes the release |
| A pull request changes how a release is built | Builds everything and publishes nothing, so a broken build is found before a tag |
| It is started by hand | The same as for a pull request |

## Making a release

1. Set the new version in `Cargo.toml` (`[workspace.package]`), run
   `cargo check` so `Cargo.lock` follows, and merge that.
2. Tag that commit on `main` with the same number and push the tag:

   ```sh
   git tag v1.2.3
   git push origin v1.2.3
   ```

The workflow refuses a tag that does not match the version in `Cargo.toml`,
and a tag for which one of the secrets below is not set, before it builds
anything.

Servers update themselves to what is tagged, and cannot go back once the
new version has brought their database up to date. Run the version on a
real library before tagging it.
When it ends, the release is on the repository's Releases page and the
image is at `ghcr.io/bingecomics/binge-server`, as `1.2.3` and as `latest`.

## What a release holds

| File | For |
| --- | --- |
| `binge-server-<version>-linux-x86_64.tar.gz` | Linux on Intel and AMD |
| `binge-server-<version>-linux-aarch64.tar.gz` | Linux on ARM |
| `binge-server-<version>-macos.tar.gz` | macOS, one binary for Apple Silicon and Intel |
| `binge-server-<version>-windows-x86_64.zip` | Windows |
| `SHA256SUMS` | The checksum of each file above |
| `update.json` | For servers that update themselves: the version and, for Linux and macOS, the archive and the checksum of the binary inside |
| `update.json.minisig` | The signature over `update.json` |

Each archive holds the binary, `LICENSE` and `README.md`. GitHub adds the
source of the tag by itself.

Things to know about them:

- **The Linux binaries are the ones the image runs.** They are taken out of
  the image's own build (`docker build --target binary`), so a downloaded
  binary also runs inside the image. They need glibc 2.36 or newer: Debian
  12, Ubuntu 24.04, Fedora 37 and later. On an older system, use the image.
- **The macOS binary is signed and notarized**, so a Mac runs it without a
  warning. A Mac asks Apple about it the first time it runs, which needs a
  connection.
- **The Windows binary is not signed.** Windows warns about it the first
  time it is run.
- **Every binary is started once before it is packed**
  (`scripts/smoke.sh`): it has to answer, say it is well, and have the web
  app inside it.
- **Servers update themselves from these files.** A running server
  fetches `update.json` and its signature from the latest release, and
  then the archive that file names
  ([ADR 0020](adr/0020-the-server-updates-itself-from-signed-releases.md)).
  So the names of the files and the addresses GitHub gives them are part
  of what servers out there rely on. A release that is published is not
  changed afterwards; a mistake is fixed with the next version.
- **Anyone can check a release by hand** with
  [minisign](https://jedisct1.github.io/minisign/) and the key in
  `crates/server/update-keys/release.pub`:
  `minisign -V -p release.pub -m update.json`.

## Secrets the release needs

Set once, by the owner, in the repository's settings under **Secrets and
variables**, **Actions**. They are only read by a build for a tag. Without
them a release stops: the macOS binary is never published unsigned, and
a release is never published without the signature servers update by.

| Secret | What it is |
| --- | --- |
| `MACOS_CERTIFICATE` | The Developer ID Application certificate with its private key, exported as a `.p12` file, in base64 |
| `MACOS_CERTIFICATE_PASSWORD` | The password given when that file was exported |
| `MACOS_NOTARY_KEY` | An App Store Connect API key, a `.p8` file, in base64 |
| `MACOS_NOTARY_KEY_ID` | The id of that key |
| `MACOS_NOTARY_ISSUER` | The issuer id shown with the key |
| `UPDATE_SIGNING_KEY` | The secret key that signs `update.json`, as minisign wrote it to its file |

The certificate and the key are from the owner's personal Apple Developer
account, as everything Binge publishes is. To put a file in base64 on a Mac
without it passing through the terminal's history or the screen:

```sh
base64 -i certificate.p12 | pbcopy
```

The image is pushed with the token GitHub gives the workflow. It needs no
secret.

### The keys that sign updates

Two key pairs were made with `minisign -G -W`. Their public halves are in
`crates/server/update-keys` and are built into every server:

| Key | Its secret half |
| --- | --- |
| `release.pub` | In `UPDATE_SIGNING_KEY`, and in the owner's own copy |
| `spare.pub` | In the owner's own copy only. Never in the repository's secrets |

The build signs with the first and makes sure the server's key accepts
the signature before anything is published (`scripts/sign-update.sh`).

The spare is for the day the first is lost or stolen. From that day
releases are signed with it, and servers go on updating. By hand:

```sh
minisign -S -s update-signing-spare.key -m update.json
```

To keep the build signing, the spare takes the first key's place: its
secret half goes into `UPDATE_SIGNING_KEY`, its public half into
`release.pub`, and a new spare is made for `spare.pub`.

What a server trusts when it starts is decided by the binary that was
installed, not by the version it updated to: that binary checks the kept
version at every start, with the keys it was built with. So:

- A server installed before the change accepts the old spare for as long
  as it is installed, and updates only to releases signed with it. The
  new spare counts for servers installed after it was added.
- A server that runs a version built without a stolen key refuses that
  key when it fetches. The binary that was installed still accepts it at
  a start, from whoever can also write the data directory. Installing a
  newer binary or image ends that.

To set the secret without the key passing through the screen or the
shell's history, from the folder that holds the owner's copy:

```sh
gh secret set UPDATE_SIGNING_KEY --repo bingecomics/binge-server < update-signing-release.key
```

## The first release

- The image's package is private when it is first pushed, even from a
  public repository. Make it public once, in the organization's
  **Packages**, **binge-server**, **Package settings**.
- While the repository is private, the builds use the organization's
  monthly minutes, and a macOS minute counts as ten. A public repository
  builds for free.

## Not there yet

- Linux binaries for systems older than the ones above.
- Windows servers that update themselves.
