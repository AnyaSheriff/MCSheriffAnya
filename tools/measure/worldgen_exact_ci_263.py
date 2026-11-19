#!/usr/bin/env python3
"""Парное наблюдение на GitHub: временные миры не попадают в артефакт."""
import hashlib
import io
import json
import os
from pathlib import Path
import queue
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
import zipfile

SOURCE = '177fd36c55d9ebad4ebbedb0564e2ad3f6dfe3ff'
RUN, ARTIFACT = 43835560378, 14212406211
ARCHIVE_SHA = '529789aacdfad5b62d1f4578f1fff2222ebcb807d1309ed00aacb1f92e525e09'
ELF_SHA = 'd74115f3ecfadbf75baa0f5ec64bae087f11536608b954939b90245eb96088de'
SEEDS = (1792932773345722119, 100554032945340)
CHUNKS = ((512,512),(512,513),(513,512),(513,513),(-513,-513),(-513,-512),(-512,-513),(-512,-512),(69,-30),(70,-30),(69,-31),(70,-31))
ROOT = Path(__file__).resolve().parents[2]
ALLOWED_ADDITIONS = frozenset({'.github/workflows/worldgen-exact-checks.yml', 'tools/measure/worldgen_exact_263.py', 'tools/measure/test_worldgen_exact_263.py', 'tools/measure/worldgen_exact_ci_263.py', 'tools/measure/test_worldgen_exact_ci_263.py', 'tools/measure/WORLDGEN-EXACT-20261116.md'})


def digest(data):
    return hashlib.sha256(data).hexdigest()


def validate_source_diff(records):
    seen = set()
    for line in records.splitlines():
        fields = line.split('\t')
        if len(fields) != 2 or fields[0] != 'A' or fields[1] not in ALLOWED_ADDITIONS or fields[1] in seen:
            raise ValueError('unsafe source/catalog diff: '+line)
        seen.add(fields[1])
    required = ALLOWED_ADDITIONS - {'tools/measure/WORLDGEN-EXACT-20261116.md'}
    if not required.issubset(seen):
        raise ValueError('expected comparator/CI additions missing')
    return sorted(seen)


def validate_metadata(run, jobs, artifact):
    if (run['id'] != RUN or run['head_sha'] != SOURCE or run['status'] != 'completed'
            or run['conclusion'] != 'success'):
        raise ValueError('source run mismatch')
    entries = jobs['jobs']
    if jobs['total_count'] != len(entries) or {j['name'] for j in entries} != {'source', 'baseline'}:
        raise ValueError('unexpected source jobs')
    for job in entries:
        wanted = 'skipped' if job['name'] == 'baseline' else 'success'
        if (job['run_id'] != RUN or job['head_sha'] != SOURCE or job['status'] != 'completed'
                or job['conclusion'] != wanted):
            raise ValueError('source job mismatch: ' + job['name'])
    if (artifact['id'] != ARTIFACT or artifact['expired'] or
            artifact['name'] != f'allay-source-{RUN}-1' or artifact['digest'] != 'sha256:' + ARCHIVE_SHA or
            artifact['workflow_run']['id'] != RUN or artifact['workflow_run']['head_sha'] != SOURCE):
        raise ValueError('artifact mismatch')


def extract_runtime(archive):
    if digest(archive) != ARCHIVE_SHA:
        raise ValueError('artifact SHA mismatch')
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        if len(z.namelist()) != len(set(z.namelist())):
            raise ValueError('duplicate zip member')
        receipt = json.loads(z.read('receipt.json'))
        entry = receipt['files']['runtime-elf']
        data = z.read('runtime-elf')
        if (receipt['source'] != SOURCE or entry['sha256'] != ELF_SHA or
                len(data) != entry['bytes'] or digest(data) != ELF_SHA or not data.startswith(b'\x7fELF')):
            raise ValueError('runtime receipt mismatch')
        return data, receipt


def api(path):
    return subprocess.check_output(['gh', 'api', '--method', 'GET', 'repos/AnyaSheriff/MCSheriffAnya' + path], timeout=300)


def port_free(port):
    with socket.socket() as sock:
        try:
            sock.bind(('127.0.0.1', port))
            return True
        except OSError:
            return False


class Console:
    def __init__(self, command, cwd, log_path, commands_path, port):
        if not port_free(port):
            raise RuntimeError('port is occupied')
        self.port = port
        self.log = log_path.open('w', encoding='utf8')
        self.commands = commands_path.open('w', encoding='utf8')
        self.process = subprocess.Popen(command, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.STDOUT, text=True, bufsize=1, start_new_session=True)
        self.lines = queue.Queue()
        self.serial = 0
        def drain():
            for line in self.process.stdout:
                self.log.write(line); self.log.flush(); self.lines.put(line)
        self.thread = threading.Thread(target=drain, daemon=True); self.thread.start()

    def wait_ready(self, side, timeout=120):
        # Команды до инициализации мира могут получить ошибочный контекст.
        markers = {'vanilla': ('Done (', 'For help, type'),
                   'ours': ('Готово (', 'Чтобы узнать команды')}[side]
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(side + ': startup readiness not observed')
            if self.process.poll() is not None:
                raise RuntimeError(side + ': exited before startup readiness')
            try:
                line = self.lines.get(timeout=min(1.0, remaining))
            except queue.Empty:
                continue
            if all(marker in line for marker in markers):
                return line

    def command(self, command, timeout=300):
        self.serial += 1
        marker = 'MCSA_EXACT_SYNC_%d_END' % self.serial
        self.commands.write(command + '\n'); self.commands.flush()
        self.process.stdin.write(command + '\nsay ' + marker + '\n'); self.process.stdin.flush()
        deadline = time.monotonic() + timeout
        received = []
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError('server exited: %s' % self.process.returncode)
            try:
                line = self.lines.get(timeout=1)
            except queue.Empty:
                continue
            received.append(line)
            if any(s in line.lower() for s in ('unknown command', 'incorrect argument', 'неизвестная команда', 'неверный аргумент', 'unexpected error occurred while trying to execute')):
                raise RuntimeError('command rejected: '+command)
            if marker in line:
                response = ''.join(received)
                return response
        raise TimeoutError('console marker timed out: '+command)

    def close(self):
        forced = False
        if self.process.poll() is None:
            try:
                self.commands.write('stop\n'); self.commands.flush()
                self.process.stdin.write('stop\n'); self.process.stdin.flush()
                self.process.wait(timeout=180)
            except (subprocess.TimeoutExpired, BrokenPipeError):
                forced = True
                os.killpg(self.process.pid, signal.SIGTERM)
                try:
                    self.process.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    os.killpg(self.process.pid, signal.SIGKILL); self.process.wait(timeout=20)
        self.thread.join(timeout=10)
        self.log.close(); self.commands.close()
        try:
            os.killpg(self.process.pid, 0); gone = False
        except ProcessLookupError:
            gone = True
        return {'server_exit': self.process.returncode, 'forced': forced,
                'group_gone': gone, 'port_free': port_free(self.port)}


def saved_chunks(world, requested):
    # Reader verifies full Anvil status/coordinates and MCS decoding/CRC.
    from genaudit_263_io import Reader, region_dir
    reader = Reader(); folder = region_dir(world, 'overworld'); result = set()
    wanted = set(requested)
    for f in sorted(list(folder.glob('*.mca')) + list(folder.glob('*.mcs'))):
        result.update((cx, cz) for cx, cz, *_ in reader.read(f, wanted))
    return result


def allocated_chunks(world, requested):
    # The region header also catches incomplete/non-full chunks; absence of a
    # fully decoded chunk alone does not prove that generation never started.
    from genaudit_263_io import region_dir
    folder = region_dir(world, 'overworld')
    result = set()
    for cx, cz in requested:
        for suffix in ('mca', 'mcs'):
            file = folder / f'r.{cx//32}.{cz//32}.{suffix}'
            if not file.exists():
                continue
            with file.open('rb') as stream:
                slot = cx % 32 + (cz % 32) * 32
                stream.seek((8 if suffix == 'mcs' else 0) + slot * (8 if suffix == 'mcs' else 4))
                entry = stream.read(8 if suffix == 'mcs' else 4)
            if len(entry) != (8 if suffix == 'mcs' else 4):
                raise ValueError('truncated region header')
            if suffix == 'mcs':
                offset, size = struct.unpack('>II', entry)
                if offset or size:
                    result.add((cx, cz))
            elif int.from_bytes(entry, 'big'):
                result.add((cx, cz))
    return result


def generate(side, seed, binary, scratch, evidence, port, *, after_saved=None):
    cwd = scratch / side; cwd.mkdir()
    config = cwd if side == 'vanilla' else cwd / 'config'; config.mkdir(exist_ok=True)
    common = (f'server-ip=127.0.0.1\nserver-port={port}\nlevel-seed={seed}\nlevel-type=minecraft:normal\n'
              'online-mode=false\nview-distance=2\nsimulation-distance=2\npause-when-empty-seconds=0\n'
              'difficulty=peaceful\nmax-tick-time=-1\n')
    (config/'server.properties').write_text(common)
    if side == 'vanilla':
        (cwd/'eula.txt').write_text('eula=true\n')
        command = ['java', '-Xms512M', '-Xmx3G', '-XX:ActiveProcessorCount=4', '-jar', str(binary), 'nogui']
    else:
        (config/'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\n'
            'offline-license-mode=disabled\nbedrock-port=0\nviaproxy=false\nautumn-forests=false\npink-cherry-groves=false\nconsole-colors=never\n')
        command = [str(binary)]
    server = Console(command, cwd, evidence / f'{side}.log', evidence / f'{side}-commands.txt', port)
    cleanup = None
    try:
        print(f'{side} seed={seed}: waiting for startup readiness', flush=True)
        ready = server.wait_ready(side)
        print(f'{side} seed={seed}: ready, freezing before target generation', flush=True)
        frozen = server.command('tick freeze', timeout=60)
        if 'frozen' not in frozen.lower():
            raise ValueError('freeze was not confirmed')
        speed = server.command('gamerule random_tick_speed 0')
        mobs = server.command('gamerule spawn_mobs false')
        if '0' not in speed or 'false' not in mobs.lower():
            raise ValueError('game rules were not confirmed')
        response = server.command('seed')
        if str(seed) not in response:
            raise ValueError('seed not confirmed by console')
        server.command('save-all flush')
        preexisting = allocated_chunks(cwd/'world', CHUNKS)
        (evidence / f'{side}-preexisting.json').write_text(json.dumps({'requested': [list(x) for x in CHUNKS], 'preexisting': [list(x) for x in sorted(preexisting)]}, indent=2)+'\n')
        if preexisting:
            raise ValueError('target chunks already generated before controlled request: '+repr(sorted(preexisting)))
        (evidence / f'{side}-control.json').write_text(json.dumps({'ready_line':ready.strip(),
            'freeze_response':frozen,'random_tick_response':speed,'spawn_mobs_response':mobs,
            'seed_response':response,
            'density_env':os.environ.get('MCSA_A6_DENSITY'),'preexisting_targets':[]},indent=2)+'\n')
        print(f'{side} seed={seed}: requesting {len(CHUNKS)} fresh chunks', flush=True)
        for cx,cz in CHUNKS:
            server.command(f'forceload add {cx*16} {cz*16}')
        deadline = time.monotonic()+1200
        last_error = None
        while time.monotonic()<deadline:
            server.command('save-all flush', timeout=300)
            try:
                have = saved_chunks(cwd/'world', CHUNKS)
                last_error = None
                if have == set(CHUNKS):
                    print(f'{side} seed={seed}: all requested full chunks saved', flush=True)
                    break
            except (OSError, ValueError, IndexError) as exc:
                last_error = repr(exc)
            time.sleep(2)
        else:
            raise TimeoutError('saved requested chunks incomplete: '+str(last_error))
        if after_saved is not None:
            after_saved(server, cwd/'world', evidence)
    finally:
        cleanup = server.close()
        (evidence / f'{side}-cleanup.json').write_text(json.dumps(cleanup, indent=2)+'\n')
        if cleanup != {'server_exit':0,'forced':False,'group_gone':True,'port_free':True}:
            raise RuntimeError('server cleanup failed: '+repr(cleanup))
    return cwd/'world'


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise SystemExit('GitHub only')
    if os.environ.get('MCSA_A6_DENSITY') != '0':
        raise SystemExit('Explicit default density path MCSA_A6_DENSITY=0 required')
    output = Path(sys.argv[1]).resolve(); output.mkdir(parents=True, exist_ok=False)
    head = subprocess.check_output(['git','rev-parse','HEAD'], cwd=ROOT, text=True).strip()
    changes = subprocess.check_output(['git','diff','--no-renames','--name-status',SOURCE,head], cwd=ROOT, text=True)
    added_files = validate_source_diff(changes)
    source_run = json.loads(api(f'/actions/runs/{RUN}'))
    jobs = json.loads(api(f'/actions/runs/{RUN}/jobs?per_page=100'))
    artifact = json.loads(api(f'/actions/artifacts/{ARTIFACT}'))
    validate_metadata(source_run, jobs, artifact)
    data, receipt = extract_runtime(api(f'/actions/artifacts/{ARTIFACT}/zip'))
    pin = json.loads((ROOT/'tools/blackbox/ci/sources-777.json').read_text())['official_jar']
    with urllib.request.urlopen(pin['url'], timeout=120) as response:
        jar = response.read(pin['bytes']+1)
    if len(jar) != pin['bytes'] or digest(jar) != pin['sha256']:
        raise ValueError('official JAR pin mismatch')
    (output/'inputs.json').write_text(json.dumps({'source':SOURCE,'orchestration_head':head,'allowed_added_files':added_files,'run':RUN,'artifact':ARTIFACT,'archive_sha256':ARCHIVE_SHA,
        'elf_sha256':ELF_SHA,'source_receipt':receipt,'jar_pin':pin,'candidate_jobs':jobs,'candidate_run':source_run,
        'runtime_options':{'java-version':'26.3','bedrock-port':0,'autumn-forests':False,'pink-cherry-groves':False,'viaproxy':False,'freeze':True,'random_tick_speed':0,'spawn_mobs':False,'MCSA_A6_DENSITY':os.environ['MCSA_A6_DENSITY']}},indent=2)+'\n')
    results=[]
    with tempfile.TemporaryDirectory(prefix='mcsa-worldgen-', dir=os.environ['RUNNER_TEMP']) as temp:
        temp=Path(temp); elf=temp/'server';elf.write_bytes(data);elf.chmod(0o700); jarpath=temp/'official.jar';jarpath.write_bytes(jar)
        for i,seed in enumerate(SEEDS):
            evidence=output/f'seed-{i+1}';evidence.mkdir();scratch=temp/f'seed-{i+1}';scratch.mkdir()
            plan={'version':'26.3','seed':seed,'dimension':'overworld','chunks':[list(x) for x in CHUNKS]}
            (evidence/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')
            vanilla=generate('vanilla',seed,jarpath,scratch,evidence,27860)
            ours=generate('ours',seed,elf,scratch,evidence,27861)
            subprocess.run([sys.executable,'-B',str(ROOT/'tools/measure/worldgen_exact_263.py'),'--ours',str(ours),
                            '--vanilla',str(vanilla),'--plan',str(evidence/'plan.json'),'--output',str(evidence/'comparison.json')],check=True)
            result=json.loads((evidence/'comparison.json').read_text())
            if not result.get('recording_valid'):
                raise ValueError('invalid comparison')
            results.append({'seed':seed,'recording_valid':result['recording_valid'],'parity':result['parity']})
    (output/'summary.json').write_text(json.dumps({'results':results,'baseline':True,'scope':'Fresh generated blocks by names and biome cells only; properties excluded. Initial spawn may generate before console freeze; all target chunks checked absent after freeze and before forceload.'},indent=2)+'\n')


if __name__ == '__main__':
    main()
