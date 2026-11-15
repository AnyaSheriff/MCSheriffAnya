#!/usr/bin/env bash
# Последняя проверка матрицы: настоящее обновление APK завершает процесс старой версии.
set -euo pipefail
out=apk-device-evidence/app-install
mkdir -p "$out"
collect() {
  for name in install-flow-before.json install-flow-after.json install-interruption-before.json install-interruption-after.json; do
    adb shell run-as ru.sheriffanya.mcsa cat "files/$name" > "$out/$name" 2> "$out/$name.stderr" || true
  done
  adb logcat -d > "$out/logcat.txt" 2>&1 || true
  if [[ ! -f "$out/receipt.json" ]]; then
    python3 - "$out" <<'PY'
import json,sys
from pathlib import Path
(Path(sys.argv[1])/'receipt.json').write_text(json.dumps({'status':'failed','scope':'actual system installer; no adb install fallback'},indent=2)+'\n')
PY
  fi
}
trap collect EXIT
adb shell dumpsys package ru.sheriffanya.mcsa > "$out/package-before.txt"
api=$(adb shell getprop ro.build.version.sdk | tr -d '\r')
if (( api >= 26 )); then
  adb shell appops set ru.sheriffanya.mcsa REQUEST_INSTALL_PACKAGES allow
else
  adb shell settings put secure install_non_market_apps 1
fi
# Сначала реальная гибель процесса с подготовленной, но НЕ подтверждённой сессией.
adb shell am instrument -w -e class ru.sheriffanya.mcsa.AppInstallFlowTest#prepareInterruptedSession -e appUpgradeApk /data/local/tmp/mcsa-upgrade.apk ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner | tee "$out/interruption-prepare-instrumentation.txt"
grep -q 'OK (1 test)' "$out/interruption-prepare-instrumentation.txt"
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' "$out/interruption-prepare-instrumentation.txt"
adb shell run-as ru.sheriffanya.mcsa cat files/install-interruption-before.json > "$out/install-interruption-before.json"
printf '%s\n' 'adb shell am force-stop ru.sheriffanya.mcsa' 'adb shell am start -W -n ru.sheriffanya.mcsa/.MainActivity' > "$out/interruption-commands.txt"
adb shell am force-stop ru.sheriffanya.mcsa
adb shell am start -W -n ru.sheriffanya.mcsa/.MainActivity > "$out/interruption-relaunch.txt"
adb shell am instrument -w -e class ru.sheriffanya.mcsa.AppInstallFlowTest#verifyInterruptedSessionAndCancel ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner | tee "$out/interruption-post-instrumentation.txt"
grep -q 'OK (1 test)' "$out/interruption-post-instrumentation.txt"
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' "$out/interruption-post-instrumentation.txt"
adb shell run-as ru.sheriffanya.mcsa cat files/install-interruption-after.json > "$out/install-interruption-after.json"
adb shell am instrument -w -e class ru.sheriffanya.mcsa.AppInstallFlowTest#prepareSystemInstall -e appUpgradeApk /data/local/tmp/mcsa-upgrade.apk ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner | tee "$out/prepare-instrumentation.txt"
grep -q 'OK (1 test)' "$out/prepare-instrumentation.txt"
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' "$out/prepare-instrumentation.txt"
adb shell run-as ru.sheriffanya.mcsa cat files/install-flow-before.json > "$out/install-flow-before.json"
python3 tools/android/app-install-ui.py "$out"
adb shell am start -W -n ru.sheriffanya.mcsa/.MainActivity > "$out/start-updated.txt"
adb shell am instrument -w -e class ru.sheriffanya.mcsa.AppInstallFlowTest#postInstallRetainsDataAndReleasesReservation ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner | tee "$out/post-instrumentation.txt"
grep -q 'OK (1 test)' "$out/post-instrumentation.txt"
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' "$out/post-instrumentation.txt"
adb shell run-as ru.sheriffanya.mcsa cat files/install-flow-after.json > "$out/install-flow-after.json"
python3 - "$out" <<'PY'
import hashlib,json,sys
from pathlib import Path
p=Path(sys.argv[1]);before=json.loads((p/'install-flow-before.json').read_text());after=json.loads((p/'install-flow-after.json').read_text());confirm=json.loads((p/'system-confirmation.json').read_text())
interrupted=json.loads((p/'install-interruption-after.json').read_text());interrupted_before=json.loads((p/'install-interruption-before.json').read_text())
assert interrupted['status']=='passed' and interrupted['cleanup'] and interrupted['data_retained'] and interrupted['host_idle']
assert interrupted['actual_retained_or_system_removed'] in {'session_retained_and_cancelled','system_removed_session_recovered_idle'}
assert interrupted['session_id']==interrupted_before['session_id'] and interrupted['pid']!=interrupted_before['pid']
assert before['installed']['code']==6 and before['target']==after['installed']['code']==7
assert before['session_present'] and before['pending_code']==7 and confirm['system_package']
assert before['candidate_sha256']==after['installed_apk_sha256']
assert before['installer_foreground_before_finish']==confirm['package']
assert before['installed']['signatures']==after['installed']['signatures']
assert before['installed']['first_install_time']==after['installed']['first_install_time']
assert before['retained']==after['retained'] and after['reservation_cleared'] and after['host_idle']
(p/'receipt.json').write_text(json.dumps({'status':'passed','api':after['api'],'instrumentation_tests':4,'actual_retained_or_system_removed':interrupted['actual_retained_or_system_removed'],'interrupted_session_cleanup':interrupted['cleanup'],'scope':'actual own same-key APK 6 to 7 through system-confirmed PackageInstaller; data/core/backups retained; not ARM hardware','files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(p.iterdir()) if f.is_file()}},indent=2)+'\n')
PY
