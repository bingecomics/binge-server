#!/usr/bin/env bash
# Runs the server and the web UI together for development.
# Open the Vite address it prints; API calls are forwarded to the server.
#
# Configure it with the server's environment variables. Vite reads BINGE_PORT
# too, so the two stay together: BINGE_PORT=9000 scripts/dev.sh
#
# BINGE_DEV_AS_TOKEN=1 scripts/dev.sh shows the app without signing in: Vite
# then sends the API token kept as BINGE_LOCAL_API_TOKEN in .env with every
# call. See web/vite.config.ts.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "$#" -gt 0 ]; then
  echo "scripts/dev.sh takes no arguments. Use environment variables instead," >&2
  echo "for example: BINGE_PORT=9000 scripts/dev.sh" >&2
  exit 2
fi

export BINGE_DATA_DIR="${BINGE_DATA_DIR:-$PWD/data}"

# Build first, so a compile error stops here instead of scrolling past Vite.
cargo build -p binge-server

trap 'kill 0' EXIT
cargo run -q -p binge-server &
server=$!
pnpm -C web dev &
web=$!

# Stop everything as soon as either one ends: a UI without its server (or the
# other way round) only hides the failure. `wait -n` would do this, but the
# bash that ships with macOS does not have it.
while kill -0 "$server" 2>/dev/null && kill -0 "$web" 2>/dev/null; do
  sleep 1
done
echo "The server or the web UI stopped; shutting the other one down." >&2
exit 1
