#!/usr/bin/env bash
# Regenerates the API description and the typed web client from the Rust code.
# With --check, changes nothing and fails if either file is out of date.
set -euo pipefail
cd "$(dirname "$0")/.."

check=false
case "${1:-}" in
  "") ;;
  --check) check=true ;;
  *) echo "usage: scripts/gen-api.sh [--check]" >&2; exit 2 ;;
esac

# Render to a temporary file first: if the server does not compile, the
# committed description stays as it was instead of being left empty.
rendered="$(mktemp)"
trap 'rm -f "$rendered"' EXIT
cargo run -q -p binge-server -- openapi >"$rendered"

client=(pnpm -C web exec openapi-typescript ../api/openapi.json -o src/api/schema.d.ts)

if "$check"; then
  if ! diff "$rendered" api/openapi.json || ! "${client[@]}" --check; then
    echo "The generated API files are out of date. Run: scripts/gen-api.sh" >&2
    exit 1
  fi
else
  cat "$rendered" >api/openapi.json
  "${client[@]}"
fi
