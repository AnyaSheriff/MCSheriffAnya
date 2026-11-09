#!/usr/bin/env bash
set -euo pipefail
# Только одноразовый CI runner. Приложение сохраняет обычный TLS и подписанный URL.
test "${GITHUB_ACTIONS:-}" = true
python3 - <<'PY'
import ipaddress,json,os,urllib.request
from pathlib import Path
address=ipaddress.IPv4Address(os.environ['GITHUB_DNS_IPV4']);assert address.is_global
with urllib.request.urlopen('https://api.github.com/meta',timeout=20) as response:meta=json.load(response)
assert any(address in ipaddress.ip_network(cidr) for cidr in meta['web']), 'Not an official GitHub web IPv4'
servers=[line.split()[1] for line in Path('/etc/resolv.conf').read_text().splitlines() if line.startswith('nameserver ')]
assert servers and servers[0]!='127.0.0.1'
Path(os.environ['RUNNER_TEMP'],'mcsa-dns-upstream').write_text(servers[0])
root=Path('apk-device-evidence');root.mkdir(exist_ok=True)
(root/'network-route.json').write_text(json.dumps({'mode':'CI-only DNS route','github_ipv4':str(address),'official_web_cidrs':meta['web'],'upstream':servers[0],'tls_hostname':'github.com','apk_code_changed':False,'tls_trust_changed':False,'limits':'Regional availability is not established by this alternate-route test'},indent=2)+'\n')
PY
if ! command -v dnsmasq >/dev/null; then
  sudo apt-get update -qq
  sudo apt-get install -y --no-install-recommends dnsmasq-base
fi
upstream="$(cat "$RUNNER_TEMP/mcsa-dns-upstream")"
sudo dnsmasq --conf-file=/dev/null --no-hosts --no-resolv --server="$upstream" \
  --listen-address=127.0.0.1 --bind-interfaces --host-record="github.com,$GITHUB_DNS_IPV4" \
  --pid-file="$RUNNER_TEMP/mcsa-github-dns.pid"
# Прямой запрос только к новому DNS; системный DNS runner не меняется.
python3 - <<'PY'
import os,socket,struct
address=socket.inet_aton(os.environ['GITHUB_DNS_IPV4']);question=b'\x17\x71\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x06github\x03com\x00\x00\x01\x00\x01'
with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as client:
 client.settimeout(3);client.sendto(question,('127.0.0.1',53));response=client.recv(4096)
 assert response[:2]==question[:2] and struct.unpack('>H',response[6:8])[0]>0 and address in response
print('CI-only GitHub DNS route verified; APK and HTTPS trust unchanged')
PY
