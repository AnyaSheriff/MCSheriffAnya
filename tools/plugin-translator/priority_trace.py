#!/usr/bin/env python3
"""Сравнить точные следы двух собственных fixture; baseline и public-контракт различаются."""
from pathlib import Path
import argparse
import copy
import json
def compare(rows, spec, public=False, require_raw=False):
    expected = {row['name']: row for row in spec['cohorts']}
    if len(rows) != len(expected) or len({row['name'] for row in rows}) != len(rows):
        raise ValueError('Неполный набор или повтор cohort')
    failures = []
    for row in rows:
        target = expected.get(row['name'])
        if target is None or row['load_order'] != target['load_order'] or row['mode'] != target['mode']:
            raise ValueError('Неверная идентичность cohort')
        raw = row.get('raw_message_nbt_hex')
        if require_raw and raw is None:
            raise ValueError('Нет raw NBT сообщений')
        if raw is not None:
            encoded = []
            for text in row['trace']:
                data = text.encode('utf-8')
                encoded.append((bytes([8]) + len(data).to_bytes(2, 'big') + data).hex())
            if raw != encoded:
                raise ValueError('Raw NBT не соответствует следу')
        reference = target['public_contract' if public else 'source_predicted_legacy']
        if row['trace'] != reference['trace'] or row['verdict'] != reference['verdict']:
            failures.append(row['name'])
    return failures
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--public', action='store_true')
    parser.add_argument('--require-raw', action='store_true')
    args = parser.parse_args()
    spec = json.loads((Path(__file__).parent / 'fixtures/priority-expected.json').read_text())
    capture = json.loads(args.capture.read_text())
    failures = compare(capture['cohorts'], spec, args.public, args.require_raw)
    print(json.dumps({'comparison': 'public Bukkit contract' if args.public else 'legacy baseline only', 'mismatches': failures, 'passed': not failures}, ensure_ascii=False))
    return 1 if failures else 0
if __name__ == '__main__':
    raise SystemExit(main())
