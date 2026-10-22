#!/usr/bin/env python3
"""Только разрешённые JSON: точные предметы1.12.2 → собственные ID775."""
import argparse
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]

def main():
    p = argparse.ArgumentParser(); p.add_argument('--data', type=Path, required=True); a = p.parse_args()
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    assert json.loads((a.data / 'version.json').read_text())['version'] == 340
    legacy = json.loads((a.data / 'legacy.json').read_text())['items']
    items = {x['id']: x for x in json.loads((a.data / 'items.json').read_text())}
    current = {x['name']: x['id'] for x in json.loads((ROOT / 'tools/data/items.json').read_text())}
    rows, skipped, seen = [], [], set()
    for pair, name in legacy.items():
        old, meta = map(int, pair.split(':')); short = name.removeprefix('minecraft:')
        # Spawn egg1.12 требует EntityTag; одна пара ID:damage недостаточна.
        if old not in items or old == 383 or short not in current:
            skipped.append({'legacy':pair,'name':name}); continue
        modern = current[short]
        assert modern not in seen, ('ambiguous target', pair, name)
        seen.add(modern)
        durable = 'maxDurability' in items[old]
        assert not durable or meta == 0
        rows.append((old, meta, modern, durable, short))
    text = '// Сгенерировано generate_legacy_items.py; только JSON, legacy-sources-1.12.2.json.\n'
    text += '// legacy ID, variant, core775 ID, durability, name. Не визуальные замены.\n'
    text += 'pub const ITEMS: &[(u16, u16, u32, bool, &str)] = &[\n'
    for old, meta, modern, durable, name in sorted(rows):
        text += f'    ({old}, {meta}, {modern}, {str(durable).lower()}, "{name}"),\n'
    text += '];\npub const ENCHANTMENTS: &[(u16, &str)] = &[\n'
    for e in json.loads((a.data / 'enchantments.json').read_text()):
        text += f'    ({e["id"]}, "{e["name"]}"),\n'
    text += '];\n'
    (ROOT / 'tools/protocol-bridge/src/legacy_item_ids.rs').write_text(text)
    (ROOT / 'tools/protocol-bridge/legacy-sources-1.12.2.json').write_text(json.dumps(manifest,indent=2)+'\n')
    report = {'mapped':len(rows),'skipped':skipped,'core_items_sha256':hashlib.sha256((ROOT / 'tools/data/items.json').read_bytes()).hexdigest()}
    (ROOT / 'tools/protocol-bridge/legacy-items-1.12.2.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Exact item variants:',len(rows),'unsupported:',len(skipped))
if __name__ == '__main__': main()
