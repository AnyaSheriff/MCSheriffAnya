#!/usr/bin/env python3
"""Выбранный обычный item lifecycle776; никаких полных registry/chunk captures."""
import argparse
import hashlib
import json
from pathlib import Path

SOURCE_SHA = 'd846e94b6e99fe18a72a1ae4d331c5bb34871ea20541815382f92eaeba588263'


def build(source):
    raw = source.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == SOURCE_SHA, 'source observation changed'
    data = json.loads(raw)
    assert data['protocol'] == 776 and data['success'] and data['server_exit'] == 0
    assert data['port_free'] and data['process_group_absent']
    assert data['item_kind'] == 71 and data['player_entity'] == 1
    packets = data['packets']
    assert [p['name'] for p in packets] == ['spawn_entity', 'entity_metadata', 'entity_metadata', 'collect', 'entity_destroy']
    assert packets[0]['decoded']['position'] == [3.5, 80.0, .5]
    assert packets[0]['decoded']['velocity_hex'] == '00'
    assert all(p['body'] == '6302050801080703010000ff' for p in packets[1:3])
    assert packets[3]['body'] == '7c020103' and packets[4]['body'] == '4d0102'
    return {
        'format': 1, 'protocol': 776, 'source_recording': source.parent.name + '/' + source.name,
        'source_sha256': SOURCE_SHA, 'script_sha256': data['script_sha256'],
        'provenance': data['provenance'], 'case': 'ordinary stone3 item with NoGravity, actual collect then destroy',
        'item_kind': 71, 'player_entity': 1, 'packets': packets, 'checks': data['checks'],
        'limits': ['Only selected item packets retained; no registry/chunk/recipe/world data',
                   'Spawn velocity is zero; nonzero velocity and item movement remain synthetic composition',
                   'NoGravity field5/serializer8 and item field8/serializer7 observed together',
                   'Actual collect count3 precedes destroy; not substituted by disappearance',
                   'Public776 admission and full gameplay are not verified by this fixture'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parent / 'testdata/item-lifecycle-26.2-observation.json')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(build(args.source), ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert args.output.read_text() == text
    else:
        args.output.write_text(text)
    print('selected item776 fixture: ' + ('check PASS' if args.check else 'written'))


if __name__ == '__main__':
    main()
