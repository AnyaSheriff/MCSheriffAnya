#!/usr/bin/env python3
"""Только числовые таблицы wiki Module:LootChest/chests, ревизия 3499911 (2026-03-27)."""
import re
import sys
from pathlib import Path
s=Path(sys.argv[1]).read_text()
lines=['// Таблицы деревень: minecraft.wiki Module:LootChest/chests, oldid=3499911.',
       '// Только poolsJava, ревизия 2026-03-27, до 26.1.2. Идентификаторы',
       '// сундуков проверены по vanilla-struct; вероятности содержимого [проверить].',
       'use crate::loot::Pool;','']
tables=[]
alias={'empty-map':'map','clay-ball':'clay_ball','leather-tunic':'leather_chestplate','leather-cap':'leather_helmet','leather-pants':'leather_leggings'}
for m in re.finditer(r'\["(village-[\w-]+)"\] = \{',s):
    key=m.group(1)
    if key=='village-potato':continue
    body=s[m.end():].split('poolsJava = {',1)[1].split('poolsJavaUpcoming',1)[0]
    pools=re.findall(r'rolls\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*\}\s*,\s*items\s*=\s*\{(.*?)\n\s*\}',body,re.S)
    assert pools,key
    constant=key.upper().replace('-','_');tables.append((key.replace('-','_'),constant))
    lines.append(f'const {constant}: [Pool; {len(pools)}] = [')
    for lo,hi,body in pools:
        entries=re.findall(r'\["([^"\n]+)"\]\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}',body)
        empty=next((int(w) for k,a,b,w in entries if k=='empty'),0)
        lines += [f'    Pool {{ rolls: ({lo},{hi}), empty: {empty}, entries: &[']
        for k,a,b,w in entries:
            if k=='empty':continue
            k=alias.get(k,k.removeprefix('raw-').replace('-','_'))
            lines.append(f'        ("{k}",{a},{b},{w}),')
        lines.append('    ] },')
    lines.append('];\n')
lines+=['pub fn pools(name: &str) -> Option<&\'static [Pool]> {','    Some(match name {']
for key,constant in tables:lines.append(f'        "{key}" => &{constant},')
lines+=['        _ => return None,','    })','}']
Path(sys.argv[2]).write_text('\n'.join(lines)+'\n')
print(len(tables),'tables')
