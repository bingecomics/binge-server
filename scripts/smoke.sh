#!/usr/bin/env bash
# Starts a built binary on an empty data directory and makes sure that it
# answers, that it says it is well, and that the web app is inside it. For a
# binary about to be released.
#
#   scripts/smoke.sh target/release/binge-server
set -euo pipefail

if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "usage: scripts/smoke.sh <binary>" >&2
  exit 2
fi
binary="$1"
port=18686
data="$(mktemp -d)"

"$binary" --data-dir "$data" --port "$port" >"$data/server.log" 2>&1 &
server=$!
stop() {
  kill "$server" 2>/dev/null || true
  wait "$server" 2>/dev/null || true
  rm -rf "$data" || true
}
trap stop EXIT

for _ in $(seq 1 50); do
  if "$binary" --port "$port" health >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done
if ! "$binary" --port "$port" health; then
  echo "The server did not come up. What it said:" >&2
  cat "$data/server.log" >&2
  exit 1
fi

page="$(curl --silent --show-error --fail "http://127.0.0.1:$port/")"
if grep -q "was not built into this binary" <<<"$page" || ! grep -q "<script" <<<"$page"; then
  echo "The web app is not inside this binary." >&2
  exit 1
fi
echo "It answers, and the web app is inside it."
