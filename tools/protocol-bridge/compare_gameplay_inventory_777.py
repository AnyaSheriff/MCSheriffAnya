"""Сверка закреплённого набора: номера ревизий не заменяют содержимое инвентаря."""
import argparse
import json
from pathlib import Path

EQUIPMENT = ['iron_helmet', 'iron_chestplate', 'iron_leggings', 'iron_boots', 'elytra', 'carved_pumpkin', 'shield']
STAGES = [item + '-' + phase for item in EQUIPMENT for phase in ['equipped', 'occupied', 'unequipped']]
STAGES += [name + '-' + phase for name in ['stack8-inventory.0', 'stack8-hotbar.0', 'offhand-helmet'] for phase in ['before', 'after']]
STAGES += [name + '-' + phase for name in ['pickup-current', 'pickup-stale'] for phase in ['before', 'after', 'closed-authoritative']]
STAGES += ['before-death', 'after-respawn']
CHECKS = [item + '-' + phase for item in EQUIPMENT for phase in ['shift-equips', 'occupied-moves-main', 'shift-unequips']]
CHECKS += ['main-inventory-shift-equips', 'full-hotbar-shift-equips-free-armor', 'stale-state-full-resync',
           'pickup-current-exact-prediction-single-action', 'pickup-stale-exact-prediction-single-action',
           'keep-inventory-respawn-restores-xp-packet']


def validate(report, target):
    if report.get('protocol') != 777 or report.get('target') != target or report.get('exit_code') != 0:
        raise ValueError('wrong protocol, target or server exit')
    if 'error' in report or report.get('behavior_failures'):
        raise ValueError('probe recorded an error or behavioral failure')
    if target == 'core' and (report.get('observe_only') is not False or report.get('checks_kind') != 'asserted-invariants'):
        raise ValueError('core must run strict behavioral checks')
    if report.get('checks') != CHECKS:
        raise ValueError('missing, reordered or extra probe checks')
    rows = report.get('snapshots', [])
    if [r.get('stage') for r in rows] != STAGES:
        raise ValueError('missing, reordered or extra snapshots')
    packets = report.get('packets', [])
    if not packets or not all(any(p.get('direction') == direction and p.get('name') == name for p in packets)
                              for direction, name in [('out', 'window_click'), ('out', 'client_command'),
                                                      ('in', 'window_items'), ('in', 'respawn'), ('in', 'experience')]):
        raise ValueError('missing real click, respawn or inventory packet evidence')
    for row in rows:
        inv = row['inventory']
        if inv.get('window') != 0 or len(inv.get('slots', [])) != 46 or not isinstance(row.get('packet_index'), int):
            raise ValueError('invalid inventory snapshot')
        if not 0 <= row['packet_index'] <= len(packets):
            raise ValueError('snapshot outside raw packet range')
        if row.get('ground_item_entities') != []:
            raise ValueError('unexpected or unobserved ground items')
    return rows


def compare(official, ours, runtime_sha):
    left, right = validate(official, 'official'), validate(ours, 'core')
    if official.get('jar_sha1') != '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c':
        raise ValueError('wrong official executable')
    if ours.get('target_sha256') != runtime_sha:
        raise ValueError('runtime does not match verified build')
    mismatches = []
    for a, b in zip(left, right):
        for field in ['slots', 'cursor']:
            if a['inventory'].get(field) != b['inventory'].get(field):
                mismatches.append({'stage': a['stage'], 'field': field})
        if a.get('xp') != b.get('xp'):
            mismatches.append({'stage': a['stage'], 'field': 'xp'})
    return {'status': 'match' if not mismatches else 'mismatch', 'snapshots': len(left),
            'checks_per_server': len(CHECKS), 'mismatches': mismatches,
            'limitations': ['No timing or graphical-client claim', 'Crafting-table stale states covered by Rust tests only',
                            'Bedrock experience covered by Rust packet tests only', 'keep_inventory toggling during death not compared']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--official', type=Path, required=True)
    parser.add_argument('--ours', type=Path, required=True)
    parser.add_argument('--runtime-sha', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = compare(json.loads(args.official.read_text()), json.loads(args.ours.read_text()), args.runtime_sha)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
    raise SystemExit(result['status'] != 'match')
