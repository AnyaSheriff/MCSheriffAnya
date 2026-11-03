#!/usr/bin/env bash
# PUBLIC_PORT CORE_PORT; foreground supervisor, stops its own child PIDs on exit.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
CACHE="${VIAPROXY_CACHE:-$HOME/.cache/mcsheriffanya/viaproxy}"
RUN="${VIAPROXY_RUN:-$CACHE/run}"
PUBLIC_PORT="${1:-26101}"
CORE_PORT="${2:-26100}"
for port in "$PUBLIC_PORT" "$CORE_PORT"; do
    [[ "$port" =~ ^[0-9]+$ ]] && ((port >= 26100 && port <= 26199)) || { echo 'Ports must be 26100–26199' >&2; exit 1; }
done
[[ "$PUBLIC_PORT" != "$CORE_PORT" ]] || { echo 'Ports must differ' >&2; exit 1; }
CORE="${MCSA_BIN:-$ROOT/target/release/mcsheriffanya}"
[[ -x "$CORE" ]] || { echo "Build core first: CARGO_BUILD_JOBS=6 cargo build --release" >&2; exit 1; }
mkdir -p "$CACHE" "$RUN/core/config" "$RUN/proxy"
JAR="${VIAPROXY_JAR:-$CACHE/ViaProxy-3.4.14.jar}"
if [[ ! -f "$JAR" ]]; then
    curl -fL --retry 3 https://github.com/ViaVersion/ViaProxy/releases/download/v3.4.14/ViaProxy-3.4.14.jar -o "$JAR.part"
    mv -- "$JAR.part" "$JAR"
fi
# Keep existing world/auth/operator data and other settings across restarts.
python3 - "$RUN/core/config" "$CORE_PORT" <<'PY'
import pathlib, sys
folder = pathlib.Path(sys.argv[1])
for name, updates in {
    'server.properties': {'server-ip':'127.0.0.1', 'server-port':sys.argv[2], 'online-mode':'false', 'view-distance':'3', 'simulation-distance':'3'},
    'mcsa.properties': {'proxy-protocol':'true', 'proxy-protocol-trusted-sources':'127.0.0.1', 'bedrock-port':'0'},
}.items():
    path = folder / name
    lines = path.read_text().splitlines() if path.exists() else []
    lines = [line for line in lines if line.split('=', 1)[0] not in updates]
    path.write_text('\n'.join(lines + [f'{k}={v}' for k, v in updates.items()]) + '\n')
PY
sed -e "s/0.0.0.0:26101/0.0.0.0:$PUBLIC_PORT/" -e "s/127.0.0.1:26100/127.0.0.1:$CORE_PORT/" \
    "$ROOT/tools/viaproxy/viaproxy.yml" > "$RUN/proxy/viaproxy.yml"
core_pid='' proxy_pid=''
cleanup() {
    trap - EXIT INT TERM
    for pid in "$proxy_pid" "$core_pid"; do
        if [[ -n "$pid" ]]; then kill -TERM "$pid" 2>/dev/null || true; fi
    done
    for pid in "$proxy_pid" "$core_pid"; do
        if [[ -n "$pid" ]]; then wait "$pid" 2>/dev/null || true; fi
    done
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
(cd "$RUN/core" && exec "$CORE") > "$RUN/core.log" 2>&1 &
core_pid=$!
echo "$core_pid" > "$RUN/core.pid"
(cd "$RUN/proxy" && exec java ${VIAPROXY_JAVA_OPTS:--Xmx512m} -jar "$JAR" config viaproxy.yml) > "$RUN/proxy.log" 2>&1 &
proxy_pid=$!
echo "$proxy_pid" > "$RUN/proxy.pid"
echo "ViaProxy PID=$proxy_pid :$PUBLIC_PORT → core PID=$core_pid 127.0.0.1:$CORE_PORT; logs: $RUN"
wait -n "$core_pid" "$proxy_pid"
