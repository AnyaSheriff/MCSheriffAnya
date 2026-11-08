#!/usr/bin/env bash
# Настоящее ядро в cdylib; desktop Cargo и источники рабочего checkout не меняются.
set -euo pipefail
ndk="${ANDROID_NDK_LATEST_HOME:-${ANDROID_NDK_HOME:-}}"
test -n "$ndk"
compilers="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
source_head="$(git rev-parse HEAD)"
source_root="$(mktemp -d "${RUNNER_TEMP:-/tmp}/mcsa-core-source-XXXXXX")"
git archive HEAD | tar -xf - -C "$source_root"
python3 tools/android/core-manifest.py "$source_root"
mkdir -p android-core-native
printf '%s\n' "$source_head" > android-core-native/source-head.txt
cp "$source_root/Cargo.android-original.toml" android-core-native/Cargo.original.toml
cp "$source_root/Cargo.toml" android-core-native/Cargo.core.toml
cp "$ndk/source.properties" android-core-native/ndk.txt
rustc -Vv > android-core-native/rust.txt
export CARGO_TARGET_DIR="$PWD/target/android-core-dylib"
export RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384'
for abi in armeabi-v7a arm64-v8a x86_64; do
  case "$abi" in
    armeabi-v7a) triple=armv7-linux-androideabi; compiler_triple=armv7a-linux-androideabi; features=(--no-default-features --features android-core);;
    arm64-v8a) triple=aarch64-linux-android; compiler_triple="$triple"; features=(--features android-core);;
    x86_64) triple=x86_64-linux-android; compiler_triple="$triple"; features=(--no-default-features --features android-core);;
  esac
  rustup target add "$triple"
  compiler="$compilers/${compiler_triple}24-clang"
  linker_var="CARGO_TARGET_$(printf '%s' "$triple" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  export "$linker_var=$compiler" "CC_${triple//-/_}=$compiler" "CXX_${triple//-/_}=$compilers/${compiler_triple}24-clang++" "AR_${triple//-/_}=$compilers/llvm-ar"
  mkdir -p "android-core-native/$abi"
  "$compiler" -O2 -Wall -Wextra -Werror -fvisibility=hidden -fPIE -pie -Wl,-z,max-page-size=16384 -Wl,-z,common-page-size=16384 android/native/host.c -ldl -o "android-core-native/$abi/libmcsa.so"
  "$compilers/llvm-readelf" --file-header --program-headers --dynamic --dyn-syms "android-core-native/$abi/libmcsa.so" > "android-core-native/elf-host-$abi.txt"
  for revision in 1 2; do
    export MCSA_CORE_BUILD_ID="$source_head-r$revision"
    cargo build --manifest-path "$source_root/Cargo.toml" --release --lib --target "$triple" "${features[@]}" 2>&1 | tee "android-core-native/build-$abi-r$revision.log"
    if [ "$revision" = 1 ]; then destination="android-core-native/$abi"; else destination="android-core-native/updates/2/$abi"; fi
    mkdir -p "$destination"
    cp "$CARGO_TARGET_DIR/$triple/release/libmcsa_android_core.so" "$destination/libmcsa_core.so"
    "$compilers/llvm-readelf" --file-header --program-headers --dynamic --version-info --dyn-syms "$destination/libmcsa_core.so" > "android-core-native/elf-core-$abi-r$revision.txt"
  done
done
cp "$source_root/Cargo.lock" android-core-native/Cargo.lock
python3 tools/android/verify-core-native.py android-core-native
(cd android-core-native && sha256sum */*.so updates/2/*/*.so Cargo.lock Cargo.original.toml Cargo.core.toml source-head.txt ndk.txt rust.txt receipt.json > SHA256SUMS)
