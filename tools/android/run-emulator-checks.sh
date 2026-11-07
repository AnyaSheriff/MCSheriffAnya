#!/usr/bin/env bash
set -euo pipefail
mkdir -p apk-device-evidence
adb shell getprop ro.build.version.sdk > apk-device-evidence/api.txt
adb shell getprop ro.product.cpu.abi > apk-device-evidence/abi.txt
adb shell getconf PAGE_SIZE > apk-device-evidence/page-size.txt
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
grep -q 'max-players=7' apk-device-evidence/server.properties
grep -q 'retained' apk-device-evidence/world-marker.txt
adb logcat -d -v threadtime > apk-device-evidence/logcat.txt
adb shell am start -W -n ru.sheriffanya.mcsa/.MainActivity
adb shell screencap -p /sdcard/mcsa-apk-home.png
adb pull /sdcard/mcsa-apk-home.png apk-device-evidence/home.png
python3 - <<'PY'
from pathlib import Path
import hashlib,json,os
p=Path('apk-device-evidence')
result={'status':'passed','api':int(os.environ['TEST_API']),'abi':'x86_64','instrumentation_tests':5,'scope':'actual installed native x86_64 core without plugins; UI/config/native lifecycle; not ARM device execution','files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(p.iterdir()) if f.is_file()}}
(p/'receipt.json').write_text(json.dumps(result,indent=2)+'\n')
PY
