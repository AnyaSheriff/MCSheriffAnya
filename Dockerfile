# syntax=docker/dockerfile:1
FROM rust:1.98.1-bookworm AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential cmake nasm perl pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .
ARG TARGETARCH
ARG TARGETVARIANT
ENV CARGO_TARGET_DIR=/build/target

# Cargo.lock не хранится в git: используем местный, а в чистой копии создаём.
# Готовый файл выносим из кэша, чтобы следующий этап мог его скопировать.
RUN --mount=type=cache,id=mcsa-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=mcsa-target-1.98.1-${TARGETARCH}${TARGETVARIANT},target=/build/target,sharing=locked \
    if [ ! -f Cargo.lock ]; then cargo generate-lockfile; fi \
    && cargo build --release --locked -j 2 --bin mcsheriffanya \
    && install -D target/release/mcsheriffanya /out/mcsheriffanya

FROM debian:bookworm-slim AS runtime

ARG VERSION=dev
LABEL org.opencontainers.image.title="MCSheriffAnya" \
      org.opencontainers.image.description="A Minecraft-compatible game server written from scratch in Rust, with Java and Bedrock support" \
      org.opencontainers.image.source="https://github.com/AnyaSheriff/MCSheriffAnya" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}"

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates tzdata libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 mcsa \
    && useradd --uid 10001 --gid mcsa --home-dir /data --no-create-home --shell /usr/sbin/nologin mcsa \
    && install -d -o mcsa -g mcsa /data

COPY --from=build /out/mcsheriffanya /usr/local/bin/mcsheriffanya
COPY LICENSE /usr/local/share/mcsheriffanya/LICENSE
# Проверяем зависимости именно в образе запуска, в том числе на arm64.
RUN deps="$(ldd /usr/local/bin/mcsheriffanya)" \
    && echo "$deps" \
    && ! echo "$deps" | grep -q 'not found'

USER 10001:10001
WORKDIR /data
VOLUME ["/data"]
EXPOSE 25565/tcp 19132/udp
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/mcsheriffanya"]
