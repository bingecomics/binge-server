#!/usr/bin/env bash
# Writes update.json for a release, beside its archives: the version and,
# for each system a running server can bring itself to it on, the archive
# to fetch and the checksum of the binary inside. A server trusts a binary
# by this file once it is signed (scripts/sign-update.sh).
#
#   scripts/update-manifest.sh 1.2.3 files
set -euo pipefail

if [ "$#" -ne 2 ] || [ ! -d "$2" ]; then
  echo "usage: scripts/update-manifest.sh <version> <folder of the archives>" >&2
  exit 2
fi
version="$1"
cd "$2"

sum() {
  if command -v sha256sum >/dev/null; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

unpacked="$(mktemp -d)"
trap 'rm -rf "$unpacked"' EXIT

# Windows is not here: a server cannot take its own place there while it
# runs, so it does not update itself.
builds=()
for platform in linux-x86_64 linux-aarch64 macos; do
  archive="binge-server-$version-$platform.tar.gz"
  if [ ! -f "$archive" ]; then
    echo "The release has no $archive." >&2
    exit 1
  fi
  mkdir "$unpacked/$platform"
  tar -xzf "$archive" -C "$unpacked/$platform" binge-server
  binary="$unpacked/$platform/binge-server"
  builds+=("$(printf '    {"platform": "%s", "archive": "%s", "archive_sha256": "%s", "binary_sha256": "%s", "binary_size": %s}' \
    "$platform" "$archive" "$(sum "$archive")" "$(sum "$binary")" "$(wc -c <"$binary" | tr -d ' ')")")
done

{
  printf '{\n  "version": "%s",\n  "files": [\n' "$version"
  for index in "${!builds[@]}"; do
    if [ "$index" -gt 0 ]; then
      printf ',\n'
    fi
    printf '%s' "${builds[$index]}"
  done
  printf '\n  ]\n}\n'
} >update.json

cat update.json
