#!/usr/bin/env python3
"""Сверка девяти окон Overworld: число жеод считается по оболочкам."""
import argparse
import collections
import json
import math
import statistics

SPECIES = {'zombie', 'skeleton', 'spider'}

def dungeon_summary(rows):
    ys = [r['pos'][1] for r in rows if (r.get('entity') or '').removeprefix('minecraft:') in SPECIES]
    hist = collections.Counter(y//16*16 for y in ys)
    return dict(count=len(ys), minimum=min(ys), median=statistics.median(ys), maximum=max(ys),
                histogram=dict(sorted(hist.items())), heights=dict(sorted(collections.Counter(ys).items())))

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('chunks')
    p.add_argument('geodes')
    p.add_argument('vanilla_geodes')
    p.add_argument('output')
    args = p.parse_args()
    with open('tools/research/structures-audit-data/vanilla-features.json') as f:
        vanilla = dungeon_summary(json.load(f))
    records = [json.loads(line) for line in open(args.chunks)]
    ours = dungeon_summary([s for r in records for s in r['spawners']])
    assert len(records) == 49152, 'A partial census is not comparable to the full vanilla windows'
    with open(args.geodes) as f:
        geode_data = json.load(f)
        geodes = geode_data['summary']
    with open(args.vanilla_geodes) as f:
        reference_data = json.load(f)
        reference = reference_data['summary']
    # Диагностика Пуассона для независимых счётчиков. Кластерная генерация
    # и соседние блоки не становятся от этого независимыми наблюдениями.
    def diagnostic(a,b):
        return dict(ours=a, vanilla=b, difference=a-b, relative_difference=(a-b)/b,
                    poisson_z=(a-b)/math.sqrt(a+b))
    output = dict(chunks=49152, seed=100554032945340, dungeon=dict(ours=ours,vanilla=vanilla,
                  comparison=diagnostic(ours['count'],vanilla['count'])),
                  geodes=dict(ours=geodes,vanilla=reference,
                  comparison=diagnostic(geodes['geodes'],reference['geodes'])),
                  budding_blocks=diagnostic(geodes['blocks']['budding_amethyst'],reference['blocks']['budding_amethyst']))
    # Две эмпирические функции распределения, включая редкий верхний хвост.
    ca = cb = distance = 0.0
    for y in sorted(set(ours['heights']) | set(vanilla['heights'])):
        ca += ours['heights'].get(y,0)/ours['count']
        cb += vanilla['heights'].get(y,0)/vanilla['count']
        distance = max(distance,abs(ca-cb))
    output['dungeon']['height_ks'] = dict(distance=distance,
        independent_sample_95_percent_threshold=1.36*math.sqrt(1/ours['count']+1/vanilla['count']))
    # Единица выборки — целая оболочка, не каждый соседний блок минерала.
    variance = sum(c['blocks']['budding_amethyst']**2 for d in [geode_data,reference_data] for c in d['components'])
    output['budding_blocks']['component_z'] = output['budding_blocks']['difference']/math.sqrt(variance)
    del output['budding_blocks']['poisson_z']
    with open(args.output,'w') as f:
        json.dump(output,f,indent=2,sort_keys=True)
    print(json.dumps(output,indent=2))

if __name__ == '__main__':
    main()
