#!/usr/bin/env python3
"""Новый fresh original/candidate UB128+4, неизменные границы и historical primary."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile

import crossbow_unbreaking_probe as probe

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / 'tools/blackbox/ci/crossbow-ub128.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def require_statistical_pass(samples):
    assert set(samples) == {'official', 'ours'}
    for side, sample in samples.items():
        assert sample['fixed_n'] == 128
        assert sample['fixed_criteria_sha256'] == probe.CRITERIA_SHA256
        assert sample['passed'] and not sample['failures'], (side, sample)


def validate_readiness(raw, expected_trials=132):
    """Подтверждения ACK/меню/удаления проверяются по сохранённым пакетам."""
    sys.path.insert(0, str(ROOT / 'tools/protocol-bridge'))
    from smoke import read_varint
    assert raw.get('causal_menu_readiness') is True
    uses = [r for r in raw['sent'] if r['action'].get('use')]
    clicks = [r for r in raw['sent'] if 'click' in r['action']]
    closes = [r for r in raw['sent'] if r['action'].get('close')]
    teleports = [r for r in raw['sent'] if r['action'].get('cmd', '').startswith('tp $(bot) ')]
    acknowledgments = [r['fresh_tp_ack'] for r in raw['commands']
                       if r.get('command', '').startswith('tp blackbox ') and r.get('fresh_tp_ack', {}).get('at', 0) >= raw['start']]
    menus = [r for r in raw['commands'] if r.get('readiness') == 'actual-open-window-and-full-items']
    cleanups = [r for r in raw['commands'] if r.get('via') == 'OWN isolated RCON' and r.get('command') == 'kill @e[type=arrow]']
    assert len(uses) == len(teleports) == len(acknowledgments) == len(cleanups) == expected_trials
    assert len(clicks) == len(menus) == expected_trials * 2
    assert len(closes) == expected_trials * 2
    assert all(a['id'] < b['id'] for a, b in zip(acknowledgments, acknowledgments[1:]))
    wires = raw['clients'][0]['wire_sent']
    use_wires = [r for r in wires if r['name'] == 'use_item']
    close_wires = [r for r in wires if r['name'] == 'close_window']
    assert len(use_wires) == expected_trials and len(close_wires) == expected_trials * 2
    for use, wire in zip(uses, use_wires):
        data = io.BytesIO(bytes.fromhex(wire['hex']))
        assert read_varint(data.read) == 0
        assert read_varint(data.read) > 0
        assert struct.unpack('>ff', data.read(8)) == (0., -20.) and not data.read()
        assert use['at'] <= wire['at']
    for i, (teleport, ack) in enumerate(zip(teleports, acknowledgments)):
        assert teleport['at'] <= ack['at'] < clicks[i * 2]['at'] < uses[i]['at']
        assert ack['pose'][:3] == [103.5, -59., 103.5]
        actual = []
        for wire in wires:
            if wire['name'] != 'teleport_confirm':
                continue
            data = io.BytesIO(bytes.fromhex(wire['hex']))
            if read_varint(data.read) != ack['id']:
                continue
            assert list(struct.unpack('>dddff', data.read(32))) == ack['pose'] and not data.read()
            actual.append(wire)
        assert actual and any(teleport['at'] <= r['at'] <= ack['at'] for r in actual)
        assert closes[i * 2]['at'] < use_wires[i]['at'] < clicks[i * 2 + 1]['at']
    for click, menu, close, wire in zip(clicks, menus, closes, close_wires):
        assert click['action']['after'] == menu['action_after'] and click['at'] <= menu['received_at']
        assert menu['received_at'] < close['at'] <= wire['at']
        data = io.BytesIO(bytes.fromhex(wire['hex']))
        assert read_varint(data.read) == menu['window'] and not data.read()
        assert any(r['name'] == 'window_items' and r['at'] == menu['received_at'] for r in raw['clients'][0]['events'])
    destroyed = [{}, {}]
    for viewer, client in enumerate(raw['clients']):
        for event in client['events']:
            if event['name'] != 'entity_destroy':
                continue
            data = io.BytesIO(bytes.fromhex(event['hex']))
            for _ in range(read_varint(data.read)):
                destroyed[viewer][read_varint(data.read)] = event['at']
            assert not data.read()
    for i, cleanup in enumerate(cleanups):
        assert cleanup['old_arrow_ids_still_tracked'] == [[], []]
        assert cleanup['issued_at'] <= cleanup['all_old_ids_gone_at'] < uses[i]['at']
        for viewer, old_ids in enumerate(cleanup['old_arrow_ids_before']):
            assert all(entity in destroyed[viewer] and destroyed[viewer][entity] < uses[i]['at'] for entity in old_ids)
    return dict(actual_use=expected_trials, actual_menu=expected_trials * 2,
                actual_TP_ACK_wire=expected_trials, actual_old_arrow_deletion=True)


def preserve(recordings, output):
    for path in recordings.glob('*.json'):
        shutil.copy2(path, output / path.name)
    for side in ['official', 'ours']:
        path = recordings / side / 'server.log'
        if path.exists():
            shutil.copy2(path, output / (side + '-server.log'))


def ci_run(output, binary):
    assert os.environ.get('GITHUB_ACTIONS') == 'true', 'fresh paired CI only'
    import combat_ci
    from crossbow_nearbreak_ci import check_raw_controls, checked_lifecycle, negative_raw_controls
    output.mkdir(parents=True, exist_ok=False)
    work = Path(tempfile.mkdtemp(prefix='mcsa-crossbow-ub128-', dir=os.environ['RUNNER_TEMP']))
    data = work / 'data'
    data.mkdir()
    config = json.loads(CONFIG.read_text())
    assert digest(ROOT / config['criteria']) == config['criteria_sha256'] == probe.CRITERIA_SHA256
    assert digest(ROOT / config['primary_reference']) == config['primary_reference_sha256']
    primary = json.loads((ROOT / config['primary_reference']).read_text())
    assert primary['original_raw_sha256'] == config['primary_raw_sha256'] == '6730a4f77a258b57c9452f02ca7ef6f347576c5e914d9b9e01f41f800e4abc28'
    assert primary['statistics']['counts'] == dict(one_arrow_then_break=29, two_arrows_then_break=19,
                                                  three_arrows_then_break=19, three_arrows_survived=61)
    assert primary['statistics']['passed']
    for source, target in [('criteria', 'fixed-probability-criteria.json'), ('primary_reference', 'historical-primary-reference.json')]:
        shutil.copy2(ROOT / config[source], output / target)
    pin = json.loads((ROOT / config['source_pin']).read_text())
    combat_ci.fetch(pin['official_jar'], work / 'server.jar')
    for entry in pin['json_files']:
        if entry['file'] in ['protocol.json', 'items.json', 'entities.json', 'blocks.json']:
            combat_ci.fetch(entry, data / entry['file'])
    selected = ['src/network/play/bow.rs', 'src/network/play/bow_tests.rs',
                'tools/blackbox/record777.py', 'tools/blackbox/crossbow_unbreaking_probe.py',
                'tools/blackbox/crossbow_unbreaking_ci.py', 'tools/blackbox/test_crossbow_unbreaking_ci.py',
                'tools/blackbox/ci/crossbow-ub128.json', config['criteria'], config['primary_reference'], config['source_pin']]
    with tarfile.open(output / 'selected-source.tar.gz', 'w:gz') as archive:
        for name in selected:
            archive.add(ROOT / name, arcname=name)
    rust_files = subprocess.check_output(['git', 'ls-files', '*.rs'], cwd=ROOT, text=True).splitlines()
    write(output / 'rust-source-sha256.json', {name: digest(ROOT / name) for name in rust_files})
    assert binary.read_bytes()[:4] == b'\x7fELF'
    (output / 'runtime').mkdir()
    shutil.copy2(binary, output / 'runtime/mcsheriffanya')
    with (output / 'runtime-elf-header.txt').open('w') as log:
        subprocess.run(['readelf', '--file-header', str(binary)], stdout=log, stderr=subprocess.STDOUT, check=True)
    manifest = dict(source=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                    runtime_sha256=digest(binary), selected_source_sha256=digest(output / 'selected-source.tar.gz'),
                    primary_reference_sha256=config['primary_reference_sha256'], criteria_sha256=probe.CRITERIA_SHA256,
                    data_sha256={p.name: digest(p) for p in data.glob('*.json')}, status='running', scope=config['scope'],
                    rule=config['future_experiment_rule'])
    result = output / 'ub128'
    probe.prepare(result, ROOT)
    assert digest(result / 'scenario.json') == config['scenario_sha256']
    assert digest(result / 'criteria-before-recording.json') == config['criteria_before_sha256']
    scenario = ROOT / 'tools/blackbox/scenarios' / (probe.NAME + '.json')
    assert not scenario.exists()
    shutil.copy2(result / 'scenario.json', scenario)
    recordings = work / 'recordings'
    command = [sys.executable, '-B', str(ROOT / 'tools/blackbox/record777.py'),
               '--data', str(data), '--output', str(recordings), '--binary', str(binary),
               '--jar', str(work / 'server.jar'), '--java', 'java', '--official-port', '26190', '--core-port', '26192',
               '--parallel', '--max-seconds', '480', '--isolate-vitals', '--causal-menu-readiness', probe.NAME]
    write(result / 'command.json', command)
    try:
        with (result / 'recorder.log').open('w') as log:
            subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=540)
        checked_lifecycle(recordings / 'lifecycle.json')
        runs = list(recordings.glob('run-*.json'))
        assert len(runs) == 1
        run = json.loads(runs[0].read_text())
        assert run.get('causal_menu_readiness') is True
        assert run['binary_sha256'] == manifest['runtime_sha256'] and run['jar_sha1'] == pin['official_jar']['sha1']
        assert run['scenario_sha256'] == {probe.NAME: config['scenario_sha256']}
        assert run['data_sha256'] == manifest['data_sha256']
        names = {v['id']: v['name'] for v in json.loads((data / 'items.json').read_text())}
        entities = {v['id']: v['name'] for v in json.loads((data / 'entities.json').read_text())}
        protocol = json.loads((data / 'protocol.json').read_text())
        rows, samples, readiness = {}, {}, {}
        for side in ['official', 'ours']:
            raw = json.loads((recordings / (probe.NAME + '-' + side + '.json')).read_text())
            check_raw_controls(raw, protocol, ROOT)
            readiness[side] = validate_readiness(raw)
            rows[side] = probe.inspect(raw, names, entities, ROOT)
            samples[side] = probe.statistics(rows[side])
        write(result / 'statistical-observations.json', dict(samples=samples, sides=rows, readiness=readiness,
              historical_primary_unchanged=True, experiment='New fresh validation; official is additional, never replaces primary'))
        raw = json.loads((recordings / (probe.NAME + '-official.json')).read_text())
        write(result / 'negative-controls.json', negative_raw_controls(probe, raw, names, entities, ROOT))
        require_statistical_pass(samples)
        manifest['status'] = 'strict-pass'
    finally:
        preserve(recordings, result)
        for path in data.glob('*.json'):
            shutil.copy2(path, output / path.name)
        shutil.copy2(CONFIG, output / 'pin.json')
        shutil.copy2(ROOT / config['source_pin'], output / 'source-pin.json')
        if scenario.exists():
            assert digest(scenario) == config['scenario_sha256']
            scenario.unlink()
        if manifest['status'] == 'running':
            manifest['status'] = 'failed'
        write(output / 'run.json', manifest)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/mcsheriffanya')
    args = parser.parse_args()
    ci_run(args.output, args.binary)
