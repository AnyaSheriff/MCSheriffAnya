#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
JDK=/usr/lib/jvm/java-25-openjdk/bin
mkdir -p dist
BUILD_DIR=$(mktemp -d dist/build.XXXXXX)
trap 'rm -rf -- "$BUILD_DIR"' EXIT
"$JDK/javac" --release 17 -encoding UTF-8 -Xlint:all -Werror \
  -d "$BUILD_DIR/classes" src/ru/sheriffanya/launcher/*.java
"$JDK/jar" --create --file "$BUILD_DIR/server.jar" \
  --main-class ru.sheriffanya.launcher.Launcher -C "$BUILD_DIR/classes" .
mv -- "$BUILD_DIR/server.jar" dist/server.jar
echo 'Готово: tools/jar-launcher/dist/server.jar (Java 17+)'
