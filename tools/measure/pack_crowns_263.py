"""Упаковывает только наблюдённые блоки крон; исходник gzip сохраняется для аудита."""
import argparse
import collections
import gzip
import json
from pathlib import Path
from crown_statistics import population

def pack(raw):
    models=[]
    for name in ['acacia','birch','fancy_oak','natural_poplar','oak','poplar','spruce','swamp_oak']:
        for tree in raw[name]:
            rows=collections.defaultdict(int)
            for x,y,z,distance in tree['leaves']:
                assert -8<=x<=8 and -8<=z<=8
                rows[y,z,distance] |= 1<<(x+8)
            models.append(dict(species=name,trunk=tree['trunk'],
                logs=[[x,y,z,{'x':0,'y':1,'z':2}[axis or 'y']] for x,y,z,axis in tree['logs']],
                rows=[[y,z,mask,distance] for (y,z,distance),mask in sorted(rows.items())]))
    return models

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('observations',type=Path)
    parser.add_argument('--models',type=Path,required=True)
    parser.add_argument('--statistics',type=Path,required=True)
    args=parser.parse_args()
    opener=gzip.open if args.observations.suffix=='.gz' else open
    with opener(args.observations,'rt') as stream: raw=json.load(stream)
    args.models.write_text(json.dumps(pack(raw),separators=(',',':'))+'\n')
    args.statistics.write_text(json.dumps({name:population(trees) for name,trees in raw.items()},separators=(',',':'))+'\n')

if __name__=='__main__':main()
