# shellcheck shell=bash
# Sourced by the build scripts, from the repository's folder. Says what tells
# this build from another of the same version: the commit it is built from,
# with a + after it when files differ from that commit, and the time. The
# server shows both beside its version.
BINGE_BUILD_COMMIT="$(git rev-parse --short HEAD 2>/dev/null || true)"
if [ -n "$BINGE_BUILD_COMMIT" ] && [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  BINGE_BUILD_COMMIT="$BINGE_BUILD_COMMIT+"
fi
BINGE_BUILD_TIME="$(date +%s)"
export BINGE_BUILD_COMMIT BINGE_BUILD_TIME
