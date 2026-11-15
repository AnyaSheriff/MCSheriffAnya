#!/usr/bin/env bash
set -euo pipefail
mkdir -p apk-device-evidence/app-update
collect_app_debug() {
  timeout 20s adb logcat -b all -d -v threadtime > apk-device-evidence/app-update/logcat-after.txt 2>&1 || true
  timeout 20s adb shell ps > apk-device-evidence/app-update/processes-after.txt 2>&1 || true
  timeout 20s adb shell dumpsys package ru.sheriffanya.mcsa > apk-device-evidence/app-update/package-after.txt 2>&1 || true
}
trap collect_app_debug EXIT
adb shell ps > apk-device-evidence/app-update/processes-before.txt
adb push apk-test/app-upgrade-x86_64.apk /data/local/tmp/mcsa-upgrade.apk
adb shell chmod 644 /data/local/tmp/mcsa-upgrade.apk
adb shell am instrument -w -r \
  -e appUpgradeApk /data/local/tmp/mcsa-upgrade.apk \
  -e class ru.sheriffanya.mcsa.AppUpdateManifestTest,ru.sheriffanya.mcsa.AppUpdatePackageTest,ru.sheriffanya.mcsa.AppUpdateArchiveTest,ru.sheriffanya.mcsa.SettingsSchemaTest,ru.sheriffanya.mcsa.AppInstallReservationTest \
  ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner \
  | tee apk-device-evidence/app-update/instrumentation.txt
python3 -B - <<'PY'
import hashlib,json,os,re
from pathlib import Path
root=Path('apk-device-evidence/app-update')
text=(root/'instrumentation.txt').read_text()
assert 'FAILURES!!!' not in text and 'INSTRUMENTATION_FAILED' not in text and 'Process crashed' not in text
assert re.search(r'OK \(\d+ tests?\)',text),text[-4000:]
record={};passed=[]
for line in text.splitlines():
    if line.startswith('INSTRUMENTATION_STATUS: '):
        key,_,value=line[len('INSTRUMENTATION_STATUS: '):].partition('=');record[key]=value
    elif line.startswith('INSTRUMENTATION_STATUS_CODE: '):
        code=int(line.partition(':')[2]);assert code>=0,('failed or skipped test',code,record)
        if code==0 and 'class' in record and 'test' in record:passed.append({'class':record['class'],'test':record['test']})
        record={}
expected={'AppUpdateManifestTest','AppUpdatePackageTest','AppUpdateArchiveTest','SettingsSchemaTest','AppInstallReservationTest'}
assert expected=={p['class'].rsplit('.',1)[-1] for p in passed},passed
assert len(passed)>=8,passed
apk=Path('apk-test/app-upgrade-x86_64.apk')
(root/'receipt.json').write_text(json.dumps({'api':int(os.environ['TEST_API']),'passed':passed,'fixture_sha256':hashlib.sha256(apk.read_bytes()).hexdigest(),'scope':'Real Android PackageManager archive and installed-signer checks, selected-core form schema, install reservation. Actual system installation checked separately.'},indent=2)+'\n')
PY
