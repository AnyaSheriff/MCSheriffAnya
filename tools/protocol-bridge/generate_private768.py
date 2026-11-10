#!/usr/bin/env python3
"""Закрытое точное соседнее ребро768↔769 из уже закреплённых JSON; не admission."""
import argparse
import hashlib
import json
from pathlib import Path
from generate_client_chain import HERE, KINDS, locked, registries, compress_mapping
from generate_static_776 import direct_mapping
from compare_schemas import ids,schema


def generate(cache,fetch):
    manifest=json.loads((HERE/'client-chain-sources.json').read_text());docs={}
    for version in (768,769):
        docs[version]={k:locked(cache/str(version),k+'.json',r,fetch)
            for k,r in manifest['versions'][str(version)]['files'].items()}
    text='// Сгенерировано generate_private768.py из pinned client-chain-sources.json.\n'
    text+='// Отдельное ребро768/769; неизвестная цель не воздух. Не public admission.\n'
    text+='use crate::{State,Direction,ids::RegistryKind};\n'
    arrays=[];branches=[];packets=[];report={'scope':'private768 edge tables ONLY; no runtime/public support','registries':{},'changed_packets':{}}
    for source,target in ((768,769),(769,768)):
        for kind in KINDS:
            values=direct_mapping(registries(docs[source])[kind],registries(docs[target])[kind]);symbol=f'{kind.upper()}_{source}'
            runs=compress_mapping(values)
            arrays.append(f'static {symbol}: &[(u32,u32,i32)] = &[\n'+''.join(f'    ({a},{b},{65535 if d is None else d}),\n' for a,b,d in runs)+'];\n')
            branches.append(f'        (RegistryKind::{kind},{source}) => Some(({len(values)},{symbol})),\n')
            report['registries'][f'{kind}/{source}-{target}']={'source_count':len(values),'missing':values.count(65535)}
        for phase,title in [('login','Login'),('configuration','Configuration'),('play','Play')]:
            for direction,suffix in [('toClient','Clientbound'),('toServer','Serverbound')]:
                a,b=docs[source]['protocol'],docs[target]['protocol'];before,after=ids(a,phase,direction),ids(b,phase,direction)
                symbol=f'{title.upper()}_{suffix.upper()}_{source}';rows=[];changed=[]
                for name,ident in sorted(before.items(),key=lambda r:r[1]):
                    mapped=after.get(name,65535);equal=mapped!=65535 and schema(a,phase,direction,name)[0]==schema(b,phase,direction,name)[0]
                    rows.append(f'    ({json.dumps(name)},{mapped},{str(equal).lower()}),\n')
                    if not equal:changed.append(name)
                arrays.append(f'static {symbol}: &[(&str,u16,bool)] = &[\n'+''.join(rows)+'];\n')
                packets.append(f'        (State::{title},Direction::{suffix},{source}) => Some({symbol}),\n')
                report['changed_packets'][f'{phase}/{direction}/{source}']=changed
    text+="pub(super) fn ranges(kind:RegistryKind,source:u16)->Option<(u32,&'static [(u32,u32,i32)])>{\n    match (kind,source) {\n"+''.join(branches)+'        _=>None,\n    }\n}\n'
    text+="pub(super) fn packets(state:State,direction:Direction,source:u16)->Option<&'static [(&'static str,u16,bool)]>{\n    match (state,direction,source) {\n"+''.join(packets)+'        _=>None,\n    }\n}\n'
    return text+''.join(arrays),report


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache',type=Path,required=True);p.add_argument('--fetch',action='store_true');p.add_argument('--check',action='store_true');a=p.parse_args()
    text,report=generate(a.cache,a.fetch)
    for path,value in [(HERE/'src/generated_private768.rs',text),(HERE/'private768-table-report.json',json.dumps(report,indent=2)+'\n')]:
        if a.check:
            if path.read_text()!=value:raise ValueError('private768 generated table drift: '+str(path))
        else:path.write_text(value)

if __name__=='__main__':main()
