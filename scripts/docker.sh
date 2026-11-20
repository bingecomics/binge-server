#!/usr/bin/env bash
# Builds the Docker image and saves it as a file that can be copied to
# another machine and loaded there, with no registry in between.
#
# It builds for this machine unless BINGE_PLATFORM says otherwise. For a NAS
# with an Intel or AMD processor, from any machine:
#
#   BINGE_PLATFORM=linux/amd64 scripts/docker.sh
#
# On the other machine: docker load -i binge-server-<version>-<arch>.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "$#" -gt 0 ]; then
  echo "scripts/docker.sh takes no arguments. Use BINGE_PLATFORM instead," >&2
  echo "for example: BINGE_PLATFORM=linux/amd64 scripts/docker.sh" >&2
  exit 2
fi

platform="${BINGE_PLATFORM:-linux/$(docker version --format '{{.Server.Arch}}')}"
arch="${platform#linux/}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
image="binge-server:$version"
file="target/docker/binge-server-$version-$arch.tar.gz"

source scripts/build-mark.sh
docker build --platform "$platform" --tag "$image" --tag binge-server:latest \
  --build-arg "BINGE_BUILD_COMMIT=$BINGE_BUILD_COMMIT" \
  --build-arg "BINGE_BUILD_TIME=$BINGE_BUILD_TIME" .

mkdir -p target/docker
docker save "$image" binge-server:latest | gzip > "$file"

echo
echo "Built $image for $platform"
echo "Saved $file ($(du -h "$file" | cut -f1 | tr -d ' '))"
