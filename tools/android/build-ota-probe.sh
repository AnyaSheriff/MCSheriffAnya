#!/usr/bin/env bash
set -euo pipefail
ndk="${ANDROID_NDK_LATEST_HOME:-${ANDROID_NDK_HOME:-}}"
test -n "$ndk"
compilers="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
mkdir -p ota-probe-native
git rev-parse HEAD > ota-probe-native/source-head.txt
cp "$ndk/source.properties" ota-probe-native/ndk.txt
for abi in armeabi-v7a arm64-v8a x86_64; do
  case "$abi" in
    armeabi-v7a) triple=armv7a-linux-androideabi;;
    arm64-v8a) triple=aarch64-linux-android;;
    x86_64) triple=x86_64-linux-android;;
  esac
  out="android/ota-probe/runtime/$abi"
  mkdir -p "$out"
  flags=(-Wall -Wextra -Werror -O2 -fvisibility=hidden -Wl,-z,max-page-size=16384 -Wl,-z,common-page-size=16384)
  "$compilers/${triple}24-clang" "${flags[@]}" -fPIE -pie android/native/host.c -ldl -o "$out/libmcsa.so"
  "$compilers/${triple}24-clang" "${flags[@]}" -fPIC -shared -DPROBE_VALUE=37 android/ota-probe/core.c -o "$out/libmcsa_core.so"
  "$compilers/${triple}24-clang" "${flags[@]}" -fPIC -shared -DPROBE_VALUE=38 android/ota-probe/core.c -o "$out/libmcsa_core_v2.so"
  for file in "$out/"*.so; do
    "$compilers/llvm-readelf" --file-header --program-headers --dynamic "$file" > "ota-probe-native/$abi-$(basename "$file").txt"
  done
done
cp -r android/ota-probe/runtime ota-probe-native/runtime
(cd ota-probe-native && sha256sum runtime/*/*.so > SHA256SUMS)
