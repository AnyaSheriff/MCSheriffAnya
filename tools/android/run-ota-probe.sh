#!/usr/bin/env bash
set -euo pipefail
mkdir -p ota-probe-device
collect() { adb logcat -d > ota-probe-device/logcat.txt 2>&1 || true; }
trap collect EXIT
adb install -r ota-probe-apk/app.apk
adb install -r ota-probe-apk/test.apk
adb shell am instrument -w ru.sheriffanya.mcsa.otaprobe.test/androidx.test.runner.AndroidJUnitRunner | tee ota-probe-device/instrumentation.txt
grep -q 'OK (1 test)' ota-probe-device/instrumentation.txt
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' ota-probe-device/instrumentation.txt
adb exec-out run-as ru.sheriffanya.mcsa.otaprobe cat files/probe-result.json > ota-probe-device/probe-result.json
python3 - <<'PY'
import json,os
from pathlib import Path
p=Path('ota-probe-device/probe-result.json');d=json.loads(p.read_text())
assert d['api']==int(os.environ['TEST_API']) and d['abi']=='x86_64'
assert d['bundled_exit']==d['private_v1_exit']==37 and d['private_v2_exit']==38
assert d['invalid_exit']==126 and d['writable_bits']==0 and not d['gameplay_tested']
assert d['private_v1_sha256']!=d['private_v2_sha256']
print('PASS: two real read-only private modules, unchanged APK, invalid module rejected')
PY
