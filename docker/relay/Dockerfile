# syntax=docker/dockerfile:1
# Build: docker build -f docker/relay/Dockerfile -t kursal-relay .

FROM rust:bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake clang perl pkg-config protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ARG TARGETPLATFORM
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry-${TARGETPLATFORM} \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git-${TARGETPLATFORM} \
    --mount=type=cache,target=/src/target,id=cargo-target-${TARGETPLATFORM} \
    cargo build -p kursal-cli --release \
    && cp target/release/kursal-relay /kursal-relay

FROM scratch AS export
COPY --from=builder /kursal-relay /kursal-relay

FROM debian:bookworm-slim
ARG VERSION=0.0.0
LABEL org.opencontainers.image.source="https://github.com/KursalChat/Kursal" \
      org.opencontainers.image.description="Kursal relay node" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.title="kursal-relay" \
      org.opencontainers.image.version="${VERSION}"
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /data kursal
COPY --from=builder /kursal-relay /usr/local/bin/kursal-relay
USER kursal
WORKDIR /data
VOLUME /data

EXPOSE 4891/tcp 4891/udp 4892/tcp
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s \
    CMD curl -sf http://127.0.0.1:4892/health || exit 1
ENTRYPOINT ["kursal-relay", "--config", "/data/relay.toml"]
CMD ["--default-config"]
