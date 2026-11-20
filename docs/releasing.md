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
| A tag `v1.2.3` is pushed | Builds everything, signs the macOS binary, pushes the image and publishes the release |
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

The workflow refuses a tag that does not match the version in `Cargo.toml`.
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

## Secrets the release needs

Set once, by the owner, in the repository's settings under **Secrets and
variables**, **Actions**. They are only read by a build for a tag. Without
them a release stops at the macOS binary, which is never published
unsigned.

| Secret | What it is |
| --- | --- |
| `MACOS_CERTIFICATE` | The Developer ID Application certificate with its private key, exported as a `.p12` file, in base64 |
| `MACOS_CERTIFICATE_PASSWORD` | The password given when that file was exported |
| `MACOS_NOTARY_KEY` | An App Store Connect API key, a `.p8` file, in base64 |
| `MACOS_NOTARY_KEY_ID` | The id of that key |
| `MACOS_NOTARY_ISSUER` | The issuer id shown with the key |

The certificate and the key are from the owner's personal Apple Developer
account, as everything Binge publishes is. To put a file in base64 on a Mac
without it passing through the terminal's history or the screen:

```sh
base64 -i certificate.p12 | pbcopy
```

The image is pushed with the token GitHub gives the workflow. It needs no
secret.

## The first release

- The image's package is private when it is first pushed, even from a
  public repository. Make it public once, in the organization's
  **Packages**, **binge-server**, **Package settings**.
- While the repository is private, the builds use the organization's
  monthly minutes, and a macOS minute counts as ten. A public repository
  builds for free.

## Not there yet

- A signature over the files that the server checks before it updates
  itself. It comes with the update button.
- Linux binaries for systems older than the ones above.
