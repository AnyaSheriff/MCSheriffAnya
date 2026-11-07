#!/usr/bin/env bash
# Собственное ядро, последовательно: ARM64 с плагинами; x86_64 для эмулятора без них.
set -euo pipefail
ndk="${ANDROID_NDK_LATEST_HOME:-${ANDROID_NDK_HOME:-}}"
test -n "$ndk"
ndk_tools="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
mkdir -p apk-native
cp "$ndk/source.properties" apk-native/ndk.txt
git rev-parse HEAD > apk-native/source-head.txt
rustc -Vv > apk-native/rust.txt
export RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384'
for abi in arm64-v8a x86_64; do
  if [ "$abi" = arm64-v8a ]; then
    triple=aarch64-linux-android
    features=()
  else
    triple=x86_64-linux-android
    features=(--no-default-features)
  fi
  rustup target add "$triple"
  compiler="$ndk_tools/${triple}26-clang"
  test -x "$compiler"
  linker_var="CARGO_TARGET_$(printf '%s' "$triple" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  export "$linker_var=$compiler"
  export "CC_${triple//-/_}=$compiler"
  export "CXX_${triple//-/_}=$ndk_tools/${triple}26-clang++"
  export "AR_${triple//-/_}=$ndk_tools/llvm-ar"
  cargo build --release --target "$triple" "${features[@]}" 2>&1 | tee "apk-native/build-$abi.log"
  mkdir -p "apk-native/$abi"
  cp "target/$triple/release/mcsheriffanya" "apk-native/$abi/libmcsa.so"
  "$ndk_tools/llvm-readelf" --file-header --program-headers --dynamic --version-info "apk-native/$abi/libmcsa.so" > "apk-native/elf-$abi.txt"
done
cp Cargo.lock apk-native/Cargo.lock
python3 tools/android/verify-native.py apk-native
(cd apk-native && sha256sum arm64-v8a/libmcsa.so x86_64/libmcsa.so Cargo.lock source-head.txt ndk.txt rust.txt > SHA256SUMS)
