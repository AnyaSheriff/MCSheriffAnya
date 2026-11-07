#!/usr/bin/env bash
set -euo pipefail
mkdir -p apk-device-evidence
collect_debug() {
  adb logcat -d -v threadtime > apk-device-evidence/logcat.txt 2>&1 || true
  adb shell run-as ru.sheriffanya.mcsa cat files/instrument-native.log > apk-device-evidence/core.log 2>&1 || true
}
trap collect_debug EXIT
adb shell getprop ro.build.version.sdk > apk-device-evidence/api.txt
adb shell getprop ro.product.cpu.abi > apk-device-evidence/abi.txt
test "$(tr -d '\r\n' < apk-device-evidence/api.txt)" = "$TEST_API"
test "$(tr -d '\r\n' < apk-device-evidence/abi.txt)" = x86_64
adb install -r apk-test/app-x86_64-debug.apk
adb install -r apk-test/app-debug-androidTest.apk
if [ "$TEST_API" -ge 33 ]; then
  adb shell pm grant ru.sheriffanya.mcsa android.permission.POST_NOTIFICATIONS
fi
# Только собственная свежая тестовая установка в отдельном эмуляторе.
adb shell am instrument -w ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner | tee apk-device-evidence/instrumentation.txt
grep -q 'OK (5 tests)' apk-device-evidence/instrumentation.txt
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' apk-device-evidence/instrumentation.txt
adb shell run-as ru.sheriffanya.mcsa cat files/server/config/server.properties > apk-device-evidence/server.properties
adb shell run-as ru.sheriffanya.mcsa cat files/server/config/mcsa.properties > apk-device-evidence/mcsa.properties
adb shell run-as ru.sheriffanya.mcsa cat files/server/world/apk-persistence-marker > apk-device-evidence/world-marker.txt
adb shell run-as ru.sheriffanya.mcsa cat files/device-test.json > apk-device-evidence/device-test.json
grep -q 'max-players=7' apk-device-evidence/server.properties
grep -q 'retained' apk-device-evidence/world-marker.txt
collect_debug
trap - EXIT
adb shell am start -W -n ru.sheriffanya.mcsa/.MainActivity
adb shell screencap -p /sdcard/mcsa-apk-home.png
adb pull /sdcard/mcsa-apk-home.png apk-device-evidence/home.png
python3 - <<'PY'
from pathlib import Path
import hashlib,json,os
p=Path('apk-device-evidence')
device=json.loads((p/'device-test.json').read_text());assert device['api']==int(os.environ['TEST_API']) and device['page_size'] in [4096,16384] and 'x86_64' in device['supported_abis']
result={'status':'passed','api':device['api'],'page_size':device['page_size'],'abi':'x86_64','instrumentation_tests':5,'scope':'actual installed native x86_64 core without plugins; UI/config/native lifecycle and background activity; not ARM device execution','files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(p.iterdir()) if f.is_file()}}
(p/'receipt.json').write_text(json.dumps(result,indent=2)+'\n')
PY
