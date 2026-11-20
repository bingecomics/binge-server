#!/usr/bin/env bash
# Packs a built binary for a release: the binary, the licence and the README
# in one archive under target/dist. The archive's ending says its kind.
#
#   scripts/package.sh target/release/binge-server binge-server-1.2.3-macos.tar.gz
#   scripts/package.sh target/release/binge-server.exe binge-server-1.2.3-windows-x86_64.zip
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: scripts/package.sh <binary> <name>.tar.gz|<name>.zip" >&2
  exit 2
fi
if [ ! -f "$1" ]; then
  echo "There is no file at $1." >&2
  exit 1
fi
binary="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
archive="$2"
cd "$(dirname "$0")/.."

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
cp "$binary" LICENSE README.md "$staging/"
chmod 755 "$staging/$(basename "$binary")"

mkdir -p target/dist
packed="$PWD/target/dist/$archive"
rm -f "$packed"
case "$archive" in
  *.tar.gz)
    # macOS would add a file of its own beside each one otherwise.
    COPYFILE_DISABLE=1 tar -czf "$packed" -C "$staging" "$(basename "$binary")" LICENSE README.md
    ;;
  *.zip)
    cd "$staging"
    if command -v zip >/dev/null; then
      zip -q -9 "$packed" "$(basename "$binary")" LICENSE README.md
    else
      # A Windows runner has 7-Zip and no zip.
      7z a -bso0 -tzip "$packed" "$(basename "$binary")" LICENSE README.md
    fi
    ;;
  *)
    echo "The archive's name has to end in .tar.gz or .zip: $archive" >&2
    exit 2
    ;;
esac

echo "Packed target/dist/$archive"
