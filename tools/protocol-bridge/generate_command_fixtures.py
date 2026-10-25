#!/usr/bin/env python3
"""Проекция сетевого графа и словари обычного дополнения, без серверного кода."""
import json
from pathlib import Path
import sys
from smoke import varint

ROOT = Path(__file__).resolve().parents[2]
IMPLEMENTED = '''advancement attribute ban ban-ip banlist bossbar clear clone damage data datapack defaultgamemode deop difficulty effect enchant execute experience fill fillbiome forceload function gamemode gamerule give help item kick kill list locate loot me msg op pardon pardon-ip particle place playsound random recipe reload return ride rotate save-all save-off save-on say schedule scoreboard seed setblock setidletimeout setworldspawn spawnpoint spectate spreadplayers stop stopsound summon tag team teammsg teleport tell tellraw tick time title tm tp transfer trigger version w weather whitelist worldborder xp'''.split()


def project(tree):
    nodes = [dict(n, children=list(n['children'])) for n in tree['nodes']]
    nodes[tree['root']]['children'] = [i for i in nodes[tree['root']]['children'] if nodes[i]['name'] in IMPLEMENTED]
    pending = [tree['root']]; seen = set()
    while pending:
        i = pending.pop()
        if i in seen: continue
        seen.add(i); pending.extend(nodes[i]['children'])
        if 'redirect' in nodes[i]: pending.append(nodes[i]['redirect'])
    order = sorted(seen); mapping = {old:new for new,old in enumerate(order)}
    output = []
    for old in order:
        n = dict(nodes[old]); n['children'] = [mapping[i] for i in n['children']]
        if 'redirect' in n: n['redirect'] = mapping[n['redirect']]
        output.append(n)
    return dict(nodes=output, root=mapping[tree['root']])


def encode(tree):
    def string(s):
        value=s.encode();return varint(len(value))+value
    out=varint(len(tree['nodes']))
    for n in tree['nodes']:
        out+=bytes([n['flags']])+varint(len(n['children']))+b''.join(varint(c) for c in n['children'])
        if 'redirect'in n:out+=varint(n['redirect'])
        if n['flags']&3:out+=string(n['name'])
        if 'parser'in n:
            out+=varint(n['parser'])+bytes.fromhex(n['properties'])
            if 'suggestion'in n:out+=string(n['suggestion'])
    return out+varint(tree['root'])


def resources(samples):
    by_input={s['input']:s for s in samples}
    catalog={}
    def add(key, prefix):
        entries={}
        for sample in samples:
            if sample['input']==prefix or (not key.startswith('selector_') and sample['input'].startswith(prefix) and not any(ch in sample['input'][len(prefix):] for ch in ' [{')):
                entries.update((m['text'],m) for m in sample['matches'])
        catalog[key]=[entries[k] for k in sorted(entries)]
    for key,prefix in [('item','/give McsaProbe '),('block','/setblock ~ ~ ~ '),('sound','/playsound '),('recipe','/recipe give McsaProbe '),('minecraft:mob_effect','/effect give McsaProbe '),('minecraft:enchantment','/enchant McsaProbe '),('summon','/summon '),('minecraft:attribute','/attribute McsaProbe '),('minecraft:worldgen/biome','/locate biome '),('minecraft:worldgen/structure','/locate structure '),('particle','/particle '),('dimension','/execute in '),('gamemode','/gamemode '),('timemarker','/time set '),('selector_keys','/kill @e['),('selector_type','/kill @e[type='),('selector_sort','/kill @e[sort='),('selector_gamemode','/kill @e[gamemode=')]:
        add(key,prefix)
    catalog['item_components']=by_input.get('/give McsaProbe stone[', {}).get('matches', [])
    catalog['selectors']=by_input['/kill @']['matches']
    catalog['minecraft:timeline']=[m for m in by_input['/time query ']['matches'] if ':' in m['text']]
    return catalog


def main():
    dest=ROOT/'tools/protocol-bridge/testdata/commands-777'
    runtime=ROOT/'src/network/play'
    for protocol,source in [(777,Path(sys.argv[1])),(775,Path(sys.argv[2]))]:
        recording=json.loads((source/'official.json').read_text())
        fixture=dest if protocol==777 else dest/'native-775';fixture.mkdir(parents=True,exist_ok=True)
        (fixture/'official.json').write_text(json.dumps(recording,indent=2)+'\n')
        for role,entry in recording['roles'].items():
            raw=(source/(role+'.bin')).read_bytes();assert encode(entry['tree'])==raw
            (fixture/(role+'.bin')).write_bytes(raw)
            projected=project(entry['tree'])
            (fixture/(role+'-projected.bin')).write_bytes(encode(projected))
            if role=='op4':
                projected['protocol']=protocol
                (runtime/f'command_tree_op4_{protocol}.json').write_text(json.dumps(projected,separators=(',',':'))+'\n')
        catalog=resources(recording['roles']['op4']['suggestions'])
        (runtime/f'command_resources_{protocol}.json').write_text(json.dumps(catalog,separators=(',',':'))+'\n')
        print(protocol,'nodes',len(projected['nodes']),'resources',{key:len(value) for key,value in catalog.items()})


if __name__=='__main__':main()
