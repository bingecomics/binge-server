#!/usr/bin/env bash
# Signs a release's update.json, which is what makes a running server
# trust the release. The signature goes beside it as update.json.minisig.
#
#   UPDATE_SIGNING_KEY=... scripts/sign-update.sh files/update.json
#   scripts/sign-update.sh --try files/update.json
#
# UPDATE_SIGNING_KEY holds the secret key, as minisign wrote it to its
# file. The signature has to be one the server accepts: it is checked
# against the key in crates/server/update-keys/release.pub, which is built
# into the server.
#
# With --try no secret is read: the file is signed with a key made on the
# spot and thrown away, to show that signing works where this runs. No
# server accepts that signature.
set -euo pipefail

try=false
if [ "${1:-}" = "--try" ]; then
  try=true
  shift
fi
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "usage: scripts/sign-update.sh [--try] <update.json>" >&2
  exit 2
fi
file="$1"
trusted="$(cd "$(dirname "$0")/.." && pwd)/crates/server/update-keys/release.pub"

keys="$(mktemp -d)"
trap 'rm -rf "$keys"' EXIT
umask 077

if "$try"; then
  minisign -G -W -p "$keys/key.pub" -s "$keys/key" >/dev/null
  trusted="$keys/key.pub"
elif [ -z "${UPDATE_SIGNING_KEY:-}" ]; then
  echo "UPDATE_SIGNING_KEY is not set. A release is not published without the signature servers update by." >&2
  exit 1
else
  printf '%s\n' "$UPDATE_SIGNING_KEY" >"$keys/key"
fi

rm -f "$file.minisig"
minisign -S -s "$keys/key" -m "$file" -x "$file.minisig" \
  -t "Binge Server $(sed -n 's/^  "version": "\(.*\)",$/\1/p' "$file")" >/dev/null

if ! minisign -V -q -p "$trusted" -m "$file" -x "$file.minisig"; then
  echo "The signature is not one the server accepts: UPDATE_SIGNING_KEY is not the key of crates/server/update-keys/release.pub." >&2
  rm -f "$file.minisig"
  exit 1
fi
if "$try"; then
  echo "Signing works here. This signature is from a key made for the try."
else
  echo "Signed, and the server's key accepts it."
fi
