#!/usr/bin/env bash
set -euo pipefail
mkdir -p apk-device-evidence
collect_debug() {
  adb logcat -d -v threadtime > apk-device-evidence/logcat.txt 2>&1 || true
  adb shell run-as ru.sheriffanya.mcsa cat files/instrument-native.log > apk-device-evidence/core.log 2>&1 || true
  for name in failure-screen.png failure-ui.xml failure-state.json device-test.json appearance-test.json backup-test.json backups.png appearance-light.png appearance-dark.png home-light.png home-dark.png appearance-system-dark.png appearance-system-light.png appearance-dynamic.png; do
    if adb shell run-as ru.sheriffanya.mcsa test -f "files/$name"; then
      adb exec-out run-as ru.sheriffanya.mcsa cat "files/$name" > "apk-device-evidence/$name" || true
    fi
  done
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
grep -q 'OK (9 tests)' apk-device-evidence/instrumentation.txt
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' apk-device-evidence/instrumentation.txt
adb shell run-as ru.sheriffanya.mcsa cat files/server/config/server.properties > apk-device-evidence/server.properties
adb shell run-as ru.sheriffanya.mcsa cat files/server/config/mcsa.properties > apk-device-evidence/mcsa.properties
adb shell run-as ru.sheriffanya.mcsa cat files/server/world/apk-persistence-marker > apk-device-evidence/world-marker.txt
adb shell run-as ru.sheriffanya.mcsa cat files/device-test.json > apk-device-evidence/device-test.json
adb shell run-as ru.sheriffanya.mcsa cat files/appearance-test.json > apk-device-evidence/appearance-test.json
for name in appearance-light appearance-dark home-light home-dark appearance-system-dark appearance-system-light; do
  adb exec-out run-as ru.sheriffanya.mcsa cat "files/$name.png" > "apk-device-evidence/$name.png"
done
if [ "$TEST_API" -ge 31 ]; then
  adb exec-out run-as ru.sheriffanya.mcsa cat files/appearance-dynamic.png > apk-device-evidence/appearance-dynamic.png
fi
adb shell run-as ru.sheriffanya.mcsa cat files/backup-test.json > apk-device-evidence/backup-test.json
adb exec-out run-as ru.sheriffanya.mcsa cat files/backups.png > apk-device-evidence/backups.png
grep -q 'max-players=7'   apk-device-evidence/server.properties
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
appearance=json.loads((p/'appearance-test.json').read_text())
assert appearance['api']==device['api']
assert all(appearance[k] for k in ['light','dark','system_dark','system_light','explicit_ignores_system','persists_recreation','configs_unchanged'])
assert appearance['dynamic_available']==(device['api']>=31) and appearance['dynamic_roundtrip']==appearance['dynamic_available']
assert device['live_theme_change_status']['version']['protocol']==777 and device['theme_change_configs_unchanged']
backup=json.loads((p/'backup-test.json').read_text())
assert backup['api']==device['api'] and backup['internal_backup_count']==2
assert all(backup[k] for k in ['internal_created','real_system_picker','external_zip_same_sha256','import_restored_marker','import_restored_settings','automatic_previous_copy'])
assert device['backup_saf_roundtrip']
result={'status':'passed','api':device['api'],'page_size':device['page_size'],'abi':'x86_64','instrumentation_tests':9,'scope':'actual installed native x86_64 core without plugins; UI/config/native lifecycle and background activity; not ARM device execution','files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(p.iterdir()) if f.is_file()}}
(p/'receipt.json').write_text(json.dumps(result,indent=2)+'\n')
PY
