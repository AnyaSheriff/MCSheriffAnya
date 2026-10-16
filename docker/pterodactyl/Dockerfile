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
LABEL org.opencontainers.image.title="MCSheriffAnya (Pterodactyl / Pelican)" \
      org.opencontainers.image.description="Minecraft-compatible Rust server for hosting panels" \
      org.opencontainers.image.source="https://github.com/AnyaSheriff/MCSheriffAnya" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}"

# Панели требуют имя container и /home/container; UID 999 — выбор образа.
# https://pelican.dev/docs/eggs/creating-a-custom-yolk/ — "a user named container".
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends bash ca-certificates tzdata libgcc-s1 tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 999 container \
    && useradd --uid 999 --gid container --home-dir /home/container --create-home --shell /bin/bash container

COPY --from=build /out/mcsheriffanya /usr/local/bin/mcsheriffanya
COPY LICENSE /usr/local/share/mcsheriffanya/LICENSE
COPY --chmod=755 docker/pterodactyl/entrypoint.sh /entrypoint.sh
# Проверяем зависимости именно в образе запуска, в том числе на arm64.
RUN deps="$(ldd /usr/local/bin/mcsheriffanya)" \
    && echo "$deps" \
    && ! echo "$deps" | grep -q 'not found'

ENV USER=container HOME=/home/container LANG=C.UTF-8
USER container
WORKDIR /home/container
EXPOSE 25565/tcp 19132/udp
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "-g", "--"]
CMD ["/entrypoint.sh"]
