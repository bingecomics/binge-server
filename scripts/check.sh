#!/usr/bin/env bash
# Runs every check the project has and stops at the first failure.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$1"; }

step "Rust: format"
cargo fmt --all -- --check

step "Rust: lints"
cargo clippy --all-targets -- -D warnings

step "Rust: tests"
cargo test

step "Web: install"
pnpm -C web install --frozen-lockfile

step "Generated API files are current"
scripts/gen-api.sh --check

step "Web: types"
pnpm -C web typecheck

step "Web: lint"
pnpm -C web lint

printf '\nAll checks passed.\n'
