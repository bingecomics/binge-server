#!/usr/bin/env bash
# Signs a macOS binary with a Developer ID certificate and has Apple notarize
# it, so that a Mac runs it without a warning. Run by the release workflow on
# a macOS runner; everything secret comes in the environment, never as an
# argument:
#
#   MACOS_CERTIFICATE           the Developer ID Application certificate with
#                               its key, a .p12 file, in base64
#   MACOS_CERTIFICATE_PASSWORD  the password of that file
#   MACOS_NOTARY_KEY            an App Store Connect API key, a .p8 file, in
#                               base64
#   MACOS_NOTARY_KEY_ID         the id of that key
#   MACOS_NOTARY_ISSUER         the issuer id the key belongs to
#
#   scripts/sign-macos.sh target/universal/binge-server
set -euo pipefail

if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "usage: scripts/sign-macos.sh <binary>" >&2
  exit 2
fi
binary="$1"
for name in MACOS_CERTIFICATE MACOS_CERTIFICATE_PASSWORD MACOS_NOTARY_KEY \
  MACOS_NOTARY_KEY_ID MACOS_NOTARY_ISSUER; do
  if [ -z "${!name:-}" ]; then
    echo "$name is not set. A macOS binary is not released unsigned: see docs/releasing.md." >&2
    exit 1
  fi
done

work="$(mktemp -d)"
keychain="$work/signing.keychain-db"
keychain_password="$(openssl rand -hex 24)"
tidy() {
  security delete-keychain "$keychain" 2>/dev/null || true
  rm -rf "$work"
}
trap tidy EXIT

# A keychain of its own, which goes when the script ends.
printf '%s' "$MACOS_CERTIFICATE" | base64 --decode >"$work/certificate.p12"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$work/certificate.p12" -P "$MACOS_CERTIFICATE_PASSWORD" \
  -f pkcs12 -k "$keychain" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_password" "$keychain" >/dev/null
security list-keychains -d user -s "$keychain"

identity="$(security find-identity -v -p codesigning "$keychain" |
  awk -F'"' '/Developer ID Application/ { print $2; exit }')"
if [ -z "$identity" ]; then
  echo "The certificate given is not a Developer ID Application certificate." >&2
  exit 1
fi

codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$identity" "$binary"
codesign --verify --strict --verbose=2 "$binary"

# Apple takes an archive, not a bare binary. A binary cannot carry the
# answer with it, so a Mac asks Apple the first time it runs it.
printf '%s' "$MACOS_NOTARY_KEY" | base64 --decode >"$work/notary.p8"
ditto -c -k --keepParent "$binary" "$work/notarize.zip"
xcrun notarytool submit "$work/notarize.zip" --wait \
  --key "$work/notary.p8" --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER"

echo "Signed and notarized."
