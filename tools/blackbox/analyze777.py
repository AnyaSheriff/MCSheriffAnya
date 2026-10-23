#!/usr/bin/env python3
"""Декодирование закреплённых схем и строгая сверка боевых пакетов777."""
import argparse
import collections
import io
import json
from pathlib import Path
import struct
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from record777 import velocity
from probe_block_entities import nbt
from smoke import read_varint


class Decoder:
    def __init__(self, protocol, entities=()):
        self.types=protocol['types']|protocol['play']['toClient']['types']
        self.entity_names={v['id']:v['name'] for v in entities}
    def read(self, data, kind, parent=None):
        parent=parent or {}
        if isinstance(kind,str):
            if kind=='void':return None
            if kind in ('varint','varlong','optvarint'):return read_varint(data.read)
            if kind=='bool':return bool(struct.unpack('>B',data.read(1))[0])
            if kind in ('i8','u8','i16','u16','i32','u32','i64','u64','f32','f64'):
                fmt={'i8':'b','u8':'B','i16':'h','u16':'H','i32':'i','u32':'I','i64':'q','u64':'Q','f32':'f','f64':'d'}[kind]
                return struct.unpack('>'+fmt,data.read(struct.calcsize(fmt)))[0]
            if kind=='string':return data.read(read_varint(data.read)).decode()
            if kind=='UUID':return data.read(16).hex()
            if kind=='lpVec3':return velocity(data)
            if kind=='entityDelta':
                flags=read_varint(data.read);count=flags>>1
                assert count<=1024
                steps=[{'tickOffset':read_varint(data.read),'delta':list(struct.unpack('>hhh',data.read(6)))} for _ in range(count)] if count else [{'delta':list(struct.unpack('>hhh',data.read(6)))}]
                return {'onGround':bool(flags&1),'steps':steps}

            if kind in ('anonymousNbt','nbt'):return nbt(data)
            if kind=='position':return data.read(8).hex()
            if kind not in self.types or self.types[kind]=='native':raise ValueError('unimplemented '+kind)
            return self.read(data,self.types[kind],parent)
        op,arg=kind
        if op=='container':
            out={}
            for field in arg:
                value=self.read(data,field['type'],out|{'..':parent})
                if field.get('anon'):
                    if value is not None:out.update(value)
                else:out[field['name']]=value
            return out
        if op=='mapper':
            v=self.read(data,arg['type'],parent);return arg['mappings'].get(str(v),v)
        if op=='switch':
            context=parent;path=arg['compareTo'].split('/')
            for part in path:context=context[part]
            key=str(context).lower() if isinstance(context,bool) else str(context)
            return self.read(data,arg['fields'].get(key,arg.get('default','void')),parent)
        if op=='array':
            count=self.read(data,arg['countType'],parent) if 'countType' in arg else arg['count'] if isinstance(arg['count'],int) else parent[arg['count']]
            assert 0<=count<65536
            return [self.read(data,arg['type'],parent) for _ in range(count)]
        if op=='option':return self.read(data,arg,parent) if self.read(data,'bool') else None
        if op=='registryEntryHolder':
            value=read_varint(data.read)
            return {arg['baseName']:value-1} if value else {arg['otherwise']['name']:self.read(data,arg['otherwise']['type'],parent)}
        if op=='entityMetadataLoop':
            values=[]
            while data.getbuffer()[data.tell()]!=arg['endVal']:values.append(self.read(data,arg['type'],parent))
            data.read(1);return values
        if op=='topBitSetTerminatedArray':
            values=[]
            while True:
                at=data.tell();flag=data.getbuffer()[at];value=self.read(data,arg['type'],parent)
                if 'slot' in value:value['slot'] &= 127
                values.append(value)
                if flag<128:return values
        raise ValueError('unimplemented '+op)


COMBAT={'update_health','damage_event','hurt_animation','entity_velocity','entity_metadata','entity_equipment','entity_update_attributes','entity_status','animation','world_particles','sound_effect','entity_effect','remove_entity_effect','entity_destroy','spawn_entity','sync_entity_position','entity_move_look','rel_entity_move','entity_look','entity_head_rotation','set_cooldown','set_slot','set_player_inventory','window_items'}


def decode_record(record,decoder,sounds=None):
    sounds=sounds or {}
    ids={c['entity']:f'player{i+1}' for i,c in enumerate(record['initial'])}
    counts=collections.Counter()
    for client in record['clients']:
        for e in client['events']:
            if e['name']=='spawn_entity':
                b=io.BytesIO(bytes.fromhex(e['hex']));eid=read_varint(b.read);b.read(16);kind=read_varint(b.read)
                if eid not in ids:counts[kind]+=1;ids[eid]=f'type{kind}:{counts[kind]}'
    counts=collections.Counter()
    for initial in record['initial']:
        for eid,kind in reversed(initial['entities']):
            if eid not in ids:counts[kind]+=1;ids[eid]=f'initial{kind}:{counts[kind]}'
    result=[];errors=[]
    for i,client in enumerate(record['clients']):
        for e in client['events']:
            if e['name'] not in COMBAT:continue
            tick=int((e['at']-record['start'])/.05//1)
            if tick<0:continue
            data=io.BytesIO(bytes.fromhex(e['hex']))
            try:
                value=decoder.read(data,'packet_'+e['name'])
                if data.read():raise ValueError('trailing bytes')
                if 'entityId' in value:value['entityId']=ids.get(value['entityId'],f"unknown:{value['entityId']}")
                if e['name']=='spawn_entity':
                    value.pop('objectUUID',None)
                    if decoder.entity_names.get(value['type']) in ('arrow','spectral_arrow','trident') and value['objectData']:
                        value['objectData']=ids.get(value['objectData']-1,f"unknown:{value['objectData']-1}")
                if e['name']=='entity_destroy':value['entityIds']=[ids.get(v,f'unknown:{v}') for v in value['entityIds']]
                if e['name']=='damage_event':
                    for k in ('sourceCauseId','sourceDirectId'):
                        if value[k]:value[k]=ids.get(value[k]-1,f"unknown:{value[k]-1}")
                if e['name']=='sound_effect':
                    # Случайные seed/pitch уже исключены штатным compare.js.
                    value.pop('seed',None);value.pop('pitch',None)
                    sound=value['sound']
                    value['sound']=sounds.get(str(sound['soundId']),sound) if 'soundId' in sound else sound['data'].get('soundName',sound['data'])
                result.append({'client':i+1,'tick':tick,'name':e['name'],'value':canonical(value)})
            except Exception as error:
                errors.append({'client':i+1,'tick':tick,'name':e['name'],'error':repr(error),'hex':e['hex']})
    return result,errors


def canonical(value):
    # Штатный bot.js уже округляет числа до трёх знаков; сырые bytes сохранены.
    if isinstance(value,float):
        rounded=round(value,3)
        return 0.0 if rounded==0 else rounded
    if isinstance(value,list):return [canonical(v) for v in value]
    if isinstance(value,dict):
        value={k:canonical(v) for k,v in value.items() if k!='stateId'}
        if 'components' in value:value['components'].sort(key=lambda c:c['type'])
        if 'metadata' in value:value['metadata'].sort(key=lambda c:c['key'])
        if 'equipments' in value:value['equipments'].sort(key=lambda c:c['slot'])
        if 'properties' in value:value['properties'].sort(key=lambda c:str(c['key']))
        return value
    return value


def main():
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('--protocol',type=Path,required=True)
    a=p.parse_args();entity_file=a.protocol.parent/'entities.json'
    decoder=Decoder(json.loads(a.protocol.read_text()),json.loads(entity_file.read_text()) if entity_file.exists() else ())
    summary=json.loads((a.folder/'summary.json').read_text())
    sound_file=a.folder/'sound-observations.json'
    sounds=json.loads(sound_file.read_text())['sounds'] if sound_file.exists() else {}
    for row in summary:
        paths=[a.folder/f"{row['scenario']}-{side}.json" for side in ('official','ours')]
        if not all(path.exists() for path in paths):continue
        both=[decode_record(json.loads(path.read_text()),decoder,sounds) for path in paths]
        for side,(events,errors) in zip(('official','ours'),both):
            (a.folder/f"{row['scenario']}-{side}-decoded.json").write_text(json.dumps({'events':events,'decode_errors':errors},indent=1)+'\n')
        counters=[collections.Counter(json.dumps(e,sort_keys=True) for e in pair[0]) for pair in both]
        missing=counters[0]-counters[1];extra=counters[1]-counters[0]
        row['missing_packets']=sum(missing.values());row['extra_packets']=sum(extra.values());row['decode_errors']=[len(pair[1]) for pair in both]
        row['status']='match' if not(missing or extra or any(row['decode_errors'])) else 'mismatch-open'
        kinds=collections.Counter(json.loads(v)['name'] for v in list(missing)+list(extra))
        row['reason']='strict decoded combat packet/tick match' if row['status']=='match' else f"packet/tick differences {sum(missing.values())}/{sum(extra.values())}: {', '.join(kinds)}; decoder gaps {row['decode_errors']}; RNG rules not relaxed"

        (a.folder/f"{row['scenario']}-diff.json").write_text(json.dumps({'missing':[json.loads(v)|{'count':n} for v,n in missing.items()],'extra':[json.loads(v)|{'count':n} for v,n in extra.items()]},indent=1)+'\n')
    (a.folder/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(collections.Counter(r['status'] for r in summary))


if __name__=='__main__':main()
