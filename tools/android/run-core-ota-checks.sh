#!/usr/bin/env bash
set -euo pipefail
# Существующая установка после обычных9проверок. Здесь НЕТ adb install/clear.
mkdir -p apk-device-evidence/ota
tls_directory="$(mktemp -d "${RUNNER_TEMP:-/tmp}/mcsa-ota-tls-XXXXXX")"
openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj /CN=mcsa-ci-untrusted -addext subjectAltName=IP:10.0.2.2 -keyout "$tls_directory/test-key.pem" -out "$tls_directory/cert.pem" > /dev/null 2>&1
python3 tools/android/serve-ota-tls-test.py "$tls_directory" > apk-device-evidence/ota/untrusted-tls-server.log 2>&1 &
tls_pid=$!
collect() {
  adb logcat -d -v threadtime > apk-device-evidence/ota/logcat.txt 2>&1 || true
  for name in ota-test.json ota-security.json ota-native.log ota-ready.png ota-updated.png; do
    if adb exec-out run-as ru.sheriffanya.mcsa cat "files/$name" > "apk-device-evidence/ota/.partial-$name" 2>/dev/null; then
      mv "apk-device-evidence/ota/.partial-$name" "apk-device-evidence/ota/$name"
    else rm -f "apk-device-evidence/ota/.partial-$name"; fi
  done
}
cleanup() {
  collect
  kill "$tls_pid" 2>/dev/null || true
  wait "$tls_pid" 2>/dev/null || true
  rm -rf -- "$tls_directory"
}
trap cleanup EXIT
python3 - <<'PY'
import socket,time
for attempt in range(50):
    try:
        with socket.create_connection(('127.0.0.1',26447),timeout=1):break
    except OSError:time.sleep(.1)
else:raise RuntimeError('TLS fixture did not listen')
PY
adb shell am instrument -w -e class ru.sheriffanya.mcsa.CoreUpdateTest ru.sheriffanya.mcsa.test/androidx.test.runner.AndroidJUnitRunner | tee apk-device-evidence/ota/instrumentation.txt
grep -q 'OK (2 tests)' apk-device-evidence/ota/instrumentation.txt
! grep -qE 'FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed' apk-device-evidence/ota/instrumentation.txt
cleanup
trap - EXIT
python3 - <<'PY'
from pathlib import Path
import hashlib,json,os
root=Path('apk-device-evidence/ota');proof=json.loads((root/'ota-test.json').read_text());security=json.loads((root/'ota-security.json').read_text())
assert proof['api']==security['api']==int(os.environ['TEST_API']) and proof['abi']=='x86_64'
assert all(proof[k] for k in ['no_apk_reinstall','world_retained','settings_retained','automatic_backup','ui_rollback','corrupt_module_rejected','interrupted_start_recovered','failed_start_recovered','running_install_rejected','replay_rejected','corrupt_pointer_recovered','read_only'])
pin=json.loads(Path('android/runtime-pin.json').read_text());revision=pin['core_revision']+1
assert proof['bundled_build']==pin['runtimes']['x86_64']['build_id'] and proof['updated_build']==pin['updates'][str(revision)]['build_id']
assert proof['updated_status']['version']['protocol']==777 and proof['updated_status']['players']['max']==7
assert proof['updated_sha256']==pin['updates'][str(revision)]['runtimes']['x86_64']['sha256']
assert security['untrusted_tls_rejected'] and security['tampered_signature_rejected'] and security['wrong_signer_rejected'] and security['wrong_abi_rejected']
assert security['incompatible_fields_rejected']==9 and security['unsafe_urls_rejected']==5 and security['invalid_sizes_rejected']==2
log=(root/'ota-native.log').read_text();assert 'MCSA_CORE_BUILD='+proof['updated_build'] in log and 'Готово (' in log
(root/'receipt.json').write_text(json.dumps({'status':'passed','api':proof['api'],'instrumentation_tests':2,'scope':'full x86 core signed HTTPS update; no APK reinstall; not ARM hardware','files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(root.iterdir()) if f.is_file()}},indent=2)+'\n')
PY
