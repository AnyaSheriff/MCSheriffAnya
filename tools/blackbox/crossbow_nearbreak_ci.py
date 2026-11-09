#!/usr/bin/env python3
"""Свежий original/candidate777 replay36: fixed outcomes, raw positives, own lifecycle."""
import argparse
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import crossbow_nearbreak_probe as main_probe
import crossbow_hands_loading_probe as hands_probe

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / 'tools/blackbox/ci/crossbow-nearbreak-777.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def validate_rows(actual, expected):
    assert isinstance(actual, list) and len(actual) == len(expected)
    assert actual == expected, 'frozen original outcomes differ'


def checked_lifecycle(path):
    value = json.loads(path.read_text())
    assert not value['cleanup_errors']
    assert value['ports_free'] == {str(port): True for port in range(26190, 26194)}
    assert len(value['servers']) == len(value['cleanup']) == 2
    assert all(r['exit_code'] == 0 and r['group_gone'] for r in value['cleanup'])
    return value


def check_raw_controls(raw, protocol, repo):
    sys.path.insert(0, str(repo / 'tools/blackbox'))
    from analyze777 import Decoder
    from combat_wire import health_packet
    assert raw['protocol'] == 777 and raw['isolate_vitals'] is True
    assert len(raw['initial']) == len(raw['clients']) == 2
    for initial in raw['initial']:
        assert initial['health'] == health_packet(initial['health_receipt'])
    decoder = Decoder(protocol)
    for initial, client in zip(raw['initial'], raw['clients']):
        assert not client.get('failure') and client['events']
        for event in client['events']:
            assert event['name'] != 'respawn', 'client died during emission-only case'
            if event['name'] == 'update_health':
                assert health_packet(event) == initial['health'], 'HP changed during emission-only case'
            if event['name'] == 'spawn_entity':
                data = io.BytesIO(bytes.fromhex(event['hex']))
                decoder.read(data, 'packet_spawn_entity')
                assert not data.read(), 'trailing spawn payload'


def negative_raw_controls(probe, raw, names, entities, repo):
    """Пять настоящих raw повреждений обязаны отвергаться позитивным анализатором."""
    mutations = []
    v = copy.deepcopy(raw)
    v['clients'][0]['events'] = [e for e in v['clients'][0]['events'] if e['name'] != 'window_items']
    mutations.append(('missing-server-inventory', v))
    v = copy.deepcopy(raw)
    v['sent'] = [r for r in v['sent'] if not r['action'].get('use')]
    mutations.append(('missing-use', v))
    v = copy.deepcopy(raw)
    v['clients'][1]['events'] = [e for e in v['clients'][1]['events'] if e['name'] != 'spawn_entity']
    mutations.append(('missing-observer-spawn', v))
    v = copy.deepcopy(raw)
    v['clients'][1]['events'] = [e for e in v['clients'][1]['events'] if e['name'] != 'entity_status']
    mutations.append(('missing-observer-break', v))
    v = copy.deepcopy(raw)
    next(e for e in v['clients'][0]['events'] if e['name'] == 'window_items')['hex'] += '00'
    mutations.append(('trailing-inventory-payload', v))
    result = []
    for name, value in mutations:
        try:
            probe.inspect(value, names, entities, repo)
        except (AssertionError, ValueError, EOFError):
            result.append(dict(mutation=name, rejected=True))
        else:
            raise AssertionError('raw corruption accepted: ' + name)
    return result


def preserve(recordings, output):
    for path in recordings.glob('*.json'):
        shutil.copy2(path, output / path.name)
    for side in ['official', 'ours']:
        path = recordings / side / 'server.log'
        if path.exists():
            shutil.copy2(path, output / (side + '-server.log'))


def ci_run(output, binary):
    assert os.environ.get('GITHUB_ACTIONS') == 'true', 'CI-only fresh paired replay'
    import combat_ci
    output.mkdir(parents=True, exist_ok=False)
    work = Path(tempfile.mkdtemp(prefix='mcsa-crossbow-nearbreak-', dir=os.environ['RUNNER_TEMP']))
    data = work / 'data'
    data.mkdir()
    config = json.loads(CONFIG.read_text())
    pin = json.loads((ROOT / config['source_pin']).read_text())
    combat_ci.fetch(pin['official_jar'], work / 'server.jar')
    for entry in pin['json_files']:
        if entry['file'] in ['protocol.json', 'entities.json', 'items.json', 'blocks.json']:
            combat_ci.fetch(entry, data / entry['file'])
    names = {v['id']: v['name'] for v in json.loads((data / 'items.json').read_text())}
    entities = {v['id']: v['name'] for v in json.loads((data / 'entities.json').read_text())}
    protocol = json.loads((data / 'protocol.json').read_text())
    runtime = output / 'runtime'
    runtime.mkdir()
    shutil.copy2(binary, runtime / 'mcsheriffanya')
    assert binary.read_bytes()[:4] == b'\x7fELF'
    with (runtime / 'elf-header.txt').open('w') as log:
        subprocess.run(['readelf', '--file-header', str(binary)], stdout=log, stderr=subprocess.STDOUT, check=True)
    subprocess.run(['git', 'archive', '--format=tar.gz', '--output=' + str(output / 'source.tar.gz'), 'HEAD'], cwd=ROOT, check=True)
    for path in data.glob('*.json'):
        shutil.copy2(path, output / path.name)
    shutil.copy2(CONFIG, output / 'pin.json')
    shutil.copy2(ROOT / config['source_pin'], output / 'source-pin.json')
    manifest = dict(source=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                    runtime_sha256=digest(binary), source_archive_sha256=digest(output / 'source.tar.gz'),
                    source_pin_sha256=digest(ROOT / config['source_pin']),
                    data_sha256={p.name: digest(p) for p in data.glob('*.json')},
                    status='running', cases=[], scope=config['scope'])
    try:
        for probe, spec in zip([main_probe, hands_probe], config['probes']):
            result = output / spec['label']
            probe.prepare(result, ROOT)
            assert digest(result / 'scenario.json') == spec['scenario_sha256']
            assert digest(result / 'criteria-before-recording.json') == spec['criteria_sha256']
            expected_path = ROOT / spec['expected']
            assert digest(expected_path) == spec['expected_sha256']
            expected = json.loads(expected_path.read_text())
            shutil.copy2(expected_path, result / 'expected-before-candidate.json')
            scenario = ROOT / 'tools/blackbox/scenarios' / (probe.NAME + '.json')
            assert not scenario.exists(), 'refuse overwriting existing scenario'
            shutil.copy2(result / 'scenario.json', scenario)
            recordings = work / spec['label']
            command = [sys.executable, '-B', str(ROOT / 'tools/blackbox/record777.py'),
                       '--data', str(data), '--output', str(recordings), '--binary', str(binary),
                       '--jar', str(work / 'server.jar'), '--java', 'java',
                       '--official-port', '26190', '--core-port', '26192',
                       '--parallel', '--max-seconds', '360', '--isolate-vitals', probe.NAME]
            write(result / 'command.json', command)
            try:
                with (result / 'recorder.log').open('w') as log:
                    subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=420)
                lifecycle = checked_lifecycle(recordings / 'lifecycle.json')
                runs = list(recordings.glob('run-*.json'))
                assert len(runs) == 1
                run = json.loads(runs[0].read_text())
                assert run['binary_sha256'] == manifest['runtime_sha256']
                assert run['jar_sha1'] == pin['official_jar']['sha1'] and run['data_sha256'] == manifest['data_sha256']
                assert run['scenario_sha256'] == {probe.NAME: spec['scenario_sha256']}
                rows = {}
                for side in ['official', 'ours']:
                    raw = json.loads((recordings / (probe.NAME + '-' + side + '.json')).read_text())
                    check_raw_controls(raw, protocol, ROOT)
                    rows[side] = probe.inspect(raw, names, entities, ROOT)
                    validate_rows(rows[side], expected['rows'])
                write(result / 'strict-observations.json', dict(status='strict-pass', sides=rows))
                if probe is hands_probe:
                    # Все прочие статусы ручного swap остаются видимыми в артефакте.
                    hands_probe.analyze(result / 'auxiliary-observations.json', recordings, data, ROOT)
                raw = json.loads((recordings / (probe.NAME + '-official.json')).read_text())
                write(result / 'negative-controls.json', negative_raw_controls(probe, raw, names, entities, ROOT))
                manifest['cases'].append(dict(label=spec['label'], cases_each=len(expected['rows']),
                                               scenario_sha256=spec['scenario_sha256'], expected_sha256=spec['expected_sha256'],
                                               lifecycle_clean=not lifecycle['cleanup_errors']))
            finally:
                preserve(recordings, result)
                if scenario.exists():
                    assert digest(scenario) == spec['scenario_sha256'], 'scenario changed unexpectedly; retained'
                    scenario.unlink()
        manifest['status'] = 'strict-pass'
    finally:
        if manifest['status'] == 'running':
            manifest['status'] = 'failed'
        write(output / 'run.json', manifest)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/mcsheriffanya')
    args = parser.parse_args()
    ci_run(args.output, args.binary)
