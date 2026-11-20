# syntax=docker/dockerfile:1

# Builds the Binge Server image: the one binary, with the web UI embedded, on
# a small Debian. `scripts/docker.sh` runs this.
#
# The two build stages run on the machine that builds, whatever the image is
# for, and the server is cross-compiled. Building an Intel image on an Apple
# Silicon Mac this way takes minutes; compiling under emulation takes far
# longer.

# The web UI. Its files are the same for every platform.
FROM --platform=$BUILDPLATFORM node:26-bookworm-slim AS web
WORKDIR /src/web
RUN npm install --global pnpm@10.26.0
COPY web/package.json web/pnpm-lock.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# The server, compiled for the platform the image is for.
FROM --platform=$BUILDPLATFORM rust:1.100-bookworm AS server
ARG TARGETARCH
WORKDIR /src

# The C compiler and libraries for the target, for SQLite and for linking.
# When the image is for the machine that builds, what the base image has is
# enough.
RUN set -eu; \
    case "$TARGETARCH" in \
        amd64) machine=x86_64 ;; \
        arm64) machine=aarch64 ;; \
        *) echo "no build for $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    echo "$machine-unknown-linux-gnu" > /target; \
    if [ "$(dpkg --print-architecture)" != "$TARGETARCH" ]; then \
        apt-get update; \
        apt-get install --yes --no-install-recommends \
            "gcc-$(echo "$machine" | tr _ -)-linux-gnu" "libc6-dev-$TARGETARCH-cross"; \
        rm -rf /var/lib/apt/lists/*; \
    fi
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc

# The toolchain the project pins, with the target added to it.
COPY rust-toolchain.toml ./
RUN rustup target add "$(cat /target)"

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY migrations/ migrations/
# The server's build script embeds the built UI and looks at the UI's sources
# to tell whether that build is current.
COPY web/ web/
COPY --from=web /src/web/out/dist/ web/out/dist/
# What tells this build from another of the same version: the commit and the
# time, which `scripts/docker.sh` gives. The server's build script reads them.
ARG BINGE_BUILD_COMMIT=""
ARG BINGE_BUILD_TIME=""
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=binge-registry-$TARGETARCH \
    --mount=type=cache,target=/src/target,id=binge-target-$TARGETARCH \
    cargo build --release --locked --package binge-server --target "$(cat /target)" \
    && cp "target/$(cat /target)/release/binge-server" /binge-server

# The binary alone, for a release: a build that stops here and writes its
# result to a folder gives the Linux binary that the image runs.
#   docker build --target binary --output <folder> .
FROM scratch AS binary
COPY --from=server /binge-server /binge-server

FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="Binge Server" \
      org.opencontainers.image.description="A small comic server for the Binge readers." \
      org.opencontainers.image.source="https://github.com/bingecomics/binge-server" \
      org.opencontainers.image.licenses="AGPL-3.0-only"
COPY --from=server /binge-server /usr/local/bin/binge-server

# Everything the server writes goes to /data: mount a folder there to keep
# it. Mount comic folders anywhere else, read-only.
ENV BINGE_DATA_DIR=/data \
    BINGE_BIND=0.0.0.0 \
    BINGE_PORT=8686
VOLUME /data
EXPOSE 8686
# The server asks itself: no other program is needed in the image. It asks
# on BINGE_PORT, so a changed port is followed.
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD ["binge-server", "health"]
ENTRYPOINT ["binge-server"]
