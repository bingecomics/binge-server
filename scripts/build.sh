#!/usr/bin/env bash
# Builds the web UI, then the server binary with the UI embedded in it.
set -euo pipefail
cd "$(dirname "$0")/.."

pnpm -C web install --frozen-lockfile
pnpm -C web build
source scripts/build-mark.sh
cargo build --release

echo
echo "Built target/release/binge-server"
