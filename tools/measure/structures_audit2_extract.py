#!/usr/bin/env python3
"""Снять тайгу только из сохранённого мира: существующий офлайн-export."""
import argparse
import json
from pathlib import Path
from village import cmd_export


def main():
    p=argparse.ArgumentParser();p.add_argument('world');p.add_argument('census');p.add_argument('output');p.add_argument('scratch')
    a=p.parse_args();data=json.loads(Path(a.census).read_text())
    starts=[s for s in data['starts'] if s['kind']=='village_taiga' and s['complete_bounds']]
    if not starts:raise ValueError('no complete measured taiga village')
    scratch=Path(a.scratch);scratch.mkdir(parents=True,exist_ok=True)
    listing=scratch/'taiga-starts.tsv'
    listing.write_text(''.join(f'taiga {s["chunk"][0]} {s["chunk"][1]}\n' for s in starts))
    cmd_export(a.world,str(listing),a.output,'taiga',str(len(starts)),'100554032945340')
    out=Path(a.output)
    out.write_text('# Источник: vanilla-struct/world, только чтение region, 26.1.2.\n'
                   '# Один экземпляр: частоты, независимая сборка и очистка природных блоков [проверить].\n'+out.read_text())

if __name__=='__main__':main()
