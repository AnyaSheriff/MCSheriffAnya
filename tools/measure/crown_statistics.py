"""Статистика отдельных деревьев; нулевые слои тоже входят в средние."""
import collections
import math

def metrics(tree):
    leaves = {tuple(v[:3]): v[3] for v in tree['leaves']}
    logs = {tuple(v[:3]): (v[3] or 'y') for v in tree['logs']}
    m = collections.defaultdict(float)
    ys = [p[1] for p in leaves]
    radius = max(max(abs(x), abs(z)) for x, y, z in leaves)
    height = max(ys)-min(ys)+1
    m.update(trunk=tree['trunk'], leaves=len(leaves), height=height, radius=radius)
    for label, value in [('trunk', tree['trunk']), ('height', height), ('radius', radius)]:
        for k in range(0, 21): m[f'{label}_bin:{k}'] = float(value == k)
    for (x, y, z), axis in logs.items():
        if x or z:
            m['branches'] += 1
            m['branch_axis:'+axis] += 1
            if -10 <= y <= 10: m[f'branch_layer:{y}'] += 1
    for d in range(1, 8): m[f'distance:{d}'] = sum(v==d for v in leaves.values()) / len(leaves)
    for y in range(-10, 11):
        layer = {(x,z) for x,dy,z in leaves if dy==y}
        m[f'layer_count:{y}'] = len(layer)
        r = max((max(abs(x),abs(z)) for x,z in layer), default=0)
        m[f'layer_radius:{y}'] = r
        m[f'layer_corner:{y}'] = sum((x,z) in layer for x in [-r,r] for z in [-r,r])/4 if r else 0
        if layer:
            xmin,xmax=min(x for x,z in layer),max(x for x,z in layer)
            zmin,zmax=min(z for x,z in layer),max(z for x,z in layer)
            m[f'box_corner:{y}'] = sum((x,z) in layer for x in [xmin,xmax] for z in [zmin,zmax])/4 if xmin<xmax and zmin<zmax else 0
        else: m[f'box_corner:{y}'] = 0
        # Полные срезы: вероятность каждого занятого места относительно ствола.
        for x,z in layer: m[f'cell:{y}:{x}:{z}'] = 1
    for axis in ['x','y','z']: m['branch_axis:'+axis] += 0
    for y in range(-10,11): m[f'branch_layer:{y}'] += 0
    m['branches'] += 0
    return dict(m)

def population(trees):
    samples=[metrics(t) for t in trees]
    keys=sorted(set().union(*(m.keys() for m in samples)))
    result={}
    for k in keys:
        values=[m.get(k,0) for m in samples]
        mean=sum(values)/len(values)
        result[k]={'mean':mean,'variance':sum((v-mean)**2 for v in values)/max(1,len(values)-1)}
    return {'n':len(samples),'metrics':result}
