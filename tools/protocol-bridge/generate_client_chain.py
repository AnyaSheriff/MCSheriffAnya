#!/usr/bin/env python3
"""Закреплённые соседние таблицы770↔771↔772↔773↔774; не полный Play/dопуск."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import urllib.request

from compare_schemas import compare, ids, schema
from generate_registry_26_2 import block_states, mapper, named
from generate_static_776 import direct_mapping

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
VERSIONS = {770: 'V1_21_5', 771: 'V1_21_6', 772: 'V1_21_8', 773: 'V1_21_9', 774: 'V1_21_11'}
KINDS = ('Item', 'Block', 'BlockState', 'Entity', 'Component', 'Particle')


def locked(directory, filename, record, fetch=False):
    # Разрешены только таблицы JSON закреплённого minecraft-data, не реализации.
    assert re.fullmatch(r'https://raw.githubusercontent.com/(?:DallasCarraher|PrismarineJS)/minecraft-data/[0-9a-f]{40}/data/(?:pc/(?:[0-9.]+|common)/[A-Za-z]+|dataPaths)\.json', record['url'])
    assert re.fullmatch('[0-9a-f]{64}', record['sha256'])
    destination = directory / filename
    if fetch and not destination.exists():
        with urllib.request.urlopen(record['url'], timeout=30) as response:
            raw = response.read(32 * 1024 * 1024 + 1)
        assert len(raw) == record['bytes'] <= 32 * 1024 * 1024
        assert hashlib.sha256(raw).hexdigest() == record['sha256']
        json.loads(raw)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(raw)
    raw = destination.read_bytes()
    assert len(raw) == record['bytes'] and hashlib.sha256(raw).hexdigest() == record['sha256'], destination
    return json.loads(raw)


def load(cache, cache774, fetch=False):
    manifest = json.loads((HERE / 'client-chain-sources.json').read_text())
    meta = {k: locked(cache, k + '.json', r, fetch) for k, r in manifest['metadata'].items()}
    docs = {}
    for number, row in manifest['versions'].items():
        version = int(number)
        docs[version] = {k: locked(cache / number, k + '.json', r, fetch) for k, r in row['files'].items()}
        assert docs[version]['version']['version'] == version
        for release in row['releases']:
            assert any(r.get('minecraftVersion') == release and r['version'] == version for r in meta['versions'])
        for kind, record in row['files'].items():
            # 1.21.7 не имеет отдельного dataPaths; протокол772 закреплён по1.21.8.
            path = meta['paths']['pc'][docs[version]['version']['minecraftVersion']][kind]
            assert record['url'].endswith('/data/' + path + '/' + kind + '.json')
    locks774 = json.loads((HERE / 'step1-sources-774.json').read_text())
    docs[774] = {k: locked(cache774, k + '.json', r, fetch) for k, r in locks774.items()}
    return docs


def registries(doc):
    return dict(Item=named(doc['items']), Block=named(doc['blocks']), BlockState=block_states(doc['blocks']),
                Entity=named(doc['entities']), Component=mapper(doc['protocol']['types']['SlotComponentType']),
                Particle=mapper(doc['protocol']['types']['Particle'][1][0]['type']))


def generate(docs):
    tables = {n: registries(d) for n, d in docs.items()}
    text = '// Сгенерировано generate_client_chain.py из client-chain-sources.json и step1-sources-774.json.\n'
    text += '// Только соседние таблицы: неизвестные ID/цели и payload остаются закрыты.\n'
    text += 'use crate::{JavaVersion, ids::RegistryKind};\n'
    text += "type Ranges = (u32, &'static [(u32,u32,i32)]);\n"
    ranges, packets, arrays = [], [], []
    report = {'scope': 'Pinned schema/static ID evidence only; controls implemented, full Play and admission CLOSED', 'edges': {}}
    for older in (773, 772, 771, 770):
        newer = older + 1
        rows = compare(docs[older]['protocol'], docs[newer]['protocol'])
        report['edges'][f'{older}-{newer}'] = {'packets': rows, 'registries': {}}
        for source, target in [(older, newer), (newer, older)]:
            pair = f'(JavaVersion::{VERSIONS[source]},JavaVersion::{VERSIONS[target]})'
            for kind in KINDS:
                values = direct_mapping(tables[source][kind], tables[target][kind])
                runs = []
                for i, value in enumerate(values):
                    delta = 65535 if value == 65535 else value - i
                    if runs and runs[-1][2] == delta:
                        runs[-1] = (runs[-1][0], i, delta)
                    else:
                        runs.append((i, i, delta))
                symbol = f'{kind.upper()}_{source}_{target}'
                arrays.append(f'static {symbol}: &[(u32,u32,i32)] = &[\n' + ''.join(f'    ({a},{b},{c}),\n' for a,b,c in runs) + '];\n')
                ranges.append(f'        (RegistryKind::{kind},{pair}) => Some(({len(values)},{symbol})),\n')
                report['edges'][f'{older}-{newer}']['registries'][f'{kind}/{source}-{target}'] = dict(count=len(values), missing=sum(v == 65535 for v in values))
            for phase, title in [('login', 'Login'), ('configuration', 'Configuration'), ('play', 'Play')]:
                for direction, suffix in [('toClient', 'Clientbound'), ('toServer', 'Serverbound')]:
                    kind = title + suffix
                    before, after = docs[source]['protocol'], docs[target]['protocol']
                    a, b = ids(before, phase, direction), ids(after, phase, direction)
                    assert sorted(a.values()) == list(range(len(a)))
                    symbol = f'{kind.upper()}_{source}_{target}'
                    data = []
                    for name, number in sorted(a.items(), key=lambda r: r[1]):
                        mapped = b.get(name, 65535)
                        equal = mapped != 65535 and schema(before, phase, direction, name)[0] == schema(after, phase, direction, name)[0]
                        data.append(f'    ({json.dumps(name)},{mapped},{str(equal).lower()}),\n')
                    arrays.append(f'static {symbol}: &[(&str,u16,bool)] = &[\n' + ''.join(data) + '];\n')
                    packets.append(f'        (RegistryKind::{kind},{pair}) => Some({symbol}),\n')
    text += 'pub(crate) fn ranges(kind: RegistryKind, from: JavaVersion, to: JavaVersion) -> Option<Ranges> {\n    match (kind,(from,to)) {\n' + ''.join(ranges) + '        _ => None,\n    }\n}\n'
    text += "pub(crate) fn packets(kind: RegistryKind, from: JavaVersion, to: JavaVersion) -> Option<&'static [(&'static str,u16,bool)]> {\n    match (kind,(from,to)) {\n" + ''.join(packets) + '        _ => None,\n    }\n}\n'
    return text + ''.join(arrays), json.dumps(report, indent=2) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--cache774', type=Path, required=True)
    parser.add_argument('--fetch', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text, report = generate(load(args.cache, args.cache774, args.fetch))
    for path, value in [(HERE / 'src/generated_client_chain.rs', text), (HERE / 'client-chain-report.json', report)]:
        if args.check:
            assert path.read_text() == value, path
        else:
            path.write_text(value)
    print(json.dumps({'edges': 4, 'rust_bytes': len(text.encode()), 'sha256': hashlib.sha256(text.encode()).hexdigest()}))


if __name__ == '__main__':
    main()
