#!/usr/bin/env python3
"""Декодирование закреплённых схем; сверка настоящими bot.js/compare.js."""
import argparse
import collections
import io
import json
from pathlib import Path
import struct
import sys
import subprocess
import re
sys.path.insert(0,str(Path(__file__).resolve().parent))
from record777 import velocity
from probe_block_entities import nbt
from smoke import read_varint


class Decoder:
    def __init__(self, protocol, entities=(), direction="toClient"):
        self.protocol=protocol
        self.types=protocol['types']|protocol['play'][direction]['types']
        self.entity_names={v['id']:v['name'] for v in entities}
        # Wiki Packets777/Explosion + оба получателя Wind Burst24.10:
        # закреплённый candidate JSON пропустил завершающий Play Sound.
        if direction=='toClient' and 'entityDelta' in protocol['types']:
            # Wiki Packets777 + villager-claim: On Ground перенесён перед
            # углами; pinned JSON оставил порядок775.
            op,fields=self.types['packet_entity_look']
            self.types['packet_entity_look']=[op,[fields[0],fields[3],fields[1],fields[2]]]
            # team-basic777: три компонента, два enum, optional color, flags.
            # Порядок и отсутствие цвета сняты с обычных /team add/modify.
            import copy
            team=copy.deepcopy(self.types['packet_teams'])
            for mode,fields in team[1][2]['type'][1]['fields'].items():
                by_name={field['name']:field for field in fields[1]}
                by_name['formatting']['type']=['option','varint']
                fields[1]=[by_name[name] for name in ('name','prefix','suffix','nameTagVisibility','collisionRule','formatting','flags')]
            self.types['packet_teams']=team
            op,fields=self.types['packet_explosion']
            if fields[-1]['name']=='blockParticles':
                self.types['packet_explosion']=[op,fields+[{'name':'playSound','type':'bool'}]]
    def read(self, data, kind, parent=None):
        parent=parent or {}
        if isinstance(kind,str):
            if kind=='void':return None
            if kind in ('varint','varlong','optvarint'):return read_varint(data.read)
            if kind=='bool':
                value=struct.unpack('>B',data.read(1))[0]
                if value>1:raise ValueError('invalid boolean')
                return bool(value)
            if kind in ('i8','u8','i16','u16','i32','u32','i64','u64','f32','f64'):
                fmt={'i8':'b','u8':'B','i16':'h','u16':'H','i32':'i','u32':'I','i64':'q','u64':'Q','f32':'f','f64':'d'}[kind]
                return struct.unpack('>'+fmt,data.read(struct.calcsize(fmt)))[0]
            if kind=='string':return data.read(read_varint(data.read)).decode()
            if kind=='UUID':return data.read(16).hex()
            if kind=='lpVec3':return dict(zip(('x','y','z'),velocity(data)))
            if kind=='entityDelta':
                flags=read_varint(data.read);count=flags>>1
                assert count<=1024
                steps=[{'tickOffset':read_varint(data.read),'delta':list(struct.unpack('>hhh',data.read(6)))} for _ in range(count)] if count else [{'delta':list(struct.unpack('>hhh',data.read(6)))}]
                return {'onGround':bool(flags&1),'steps':steps}

            if kind in ('anonymousNbt','nbt','anonOptionalNbt'):return nbt(data)
            if kind=='position':return self.read(data,self.types[kind],parent)
            if kind not in self.types or self.types[kind]=='native':raise ValueError('unimplemented '+kind)
            result=self.read(data,self.types[kind],parent)
            if kind=='packet_teams' and 'entityDelta' in self.protocol['types'] and 'formatting' in result and result['formatting'] is None:
                result['formatting']=21
            return result
        op,arg=kind
        if op=='bitfield':
            total=sum(f['size'] for f in arg);value=int.from_bytes(data.read((total+7)//8),'big');out={}
            for f in arg:
                total-=f['size'];v=(value>>total)&((1<<f['size'])-1)
                if f['signed'] and v&(1<<(f['size']-1)):v-=1<<f['size']
                out[f['name']]=v
            return out
        if op=='bitflags':
            value=self.read(data,arg['type'],parent)
            return {flag:bool(value&(1<<i)) for i,flag in enumerate(arg['flags'])}
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


def observed_attribute_names(decoder,baseline):
    # Candidate JSON содержит устаревшие имена атрибутов777. Берём ID из
    # закреплённых Slot-наблюдений, а подписи775 — из схемы самого bot.js.
    def mapping(protocol):
        schema=protocol['play']['toClient']['types']['packet_entity_update_attributes']
        return schema[1][1]['type'][1]['type'][1][0]['type'][1]['mappings']
    aliases={name.rsplit('.',1)[-1]:name for name in mapping(baseline).values()}
    evidence=json.loads((Path(__file__).parents[1]/'protocol-bridge/testdata/attribute-id-observations.json').read_text())
    names={}
    for row in evidence['new']['cases']:
        item=decoder.read(io.BytesIO(bytes.fromhex(row['slot_hex'])),'Slot')
        modifiers=next(c['data'] for c in item['components'] if c['type']=='attribute_modifiers')
        name=row['name'].removeprefix('minecraft:')
        names[str(modifiers[0]['typeId'])]=aliases.get(name,name)
    mapping(decoder.protocol).clear();mapping(decoder.protocol).update(names)


def project_record(record,decoder,scenario,data,output,sounds):
    # Читаем лишь пакеты, на которые подписан bot.js; ошибки не скрываем.
    source=(Path(__file__).parent/'bot.js').read_text().split("bot.once('spawn', async () => {")[0]
    names=set(re.findall(r"\.on\('([^']+)'",source))
    for declaration in ('MOB_PACKETS','OTHER_PACKETS'):
        names.update(re.findall(r"'([^']+)'",re.search(r'const '+declaration+r' = new Set\(\[(.*?)\]\)',source).group(1)))
    names.update(('entity_look','entity_move_look','rel_entity_move','entity_teleport','sync_entity_position'))
    names.update(re.findall(r"case '([^']+)'",source))
    if not scenario.get('track_recipes'):
        names.discard('recipe_book_add');names.discard('recipe_book_remove')
    packets=[];errors=[{'client':i,'error':client['failure']} for i,client in enumerate(record['clients']) if client.get('failure')]
    for i,client in enumerate(record['clients']):
        for e in client.get('preroll',[])+client['events']:
            if e['name'] not in names:continue
            try:
                stream=io.BytesIO(bytes.fromhex(e['hex']))
                value=decoder.read(stream,'packet_'+e['name'])
                if stream.read():raise ValueError('trailing bytes')
                if e['name']=='world_event' and value['effectId']==1010:
                    songs=client.get('registry_names',{}).get('minecraft:jukebox_song')
                    if songs is None and record.get('protocol')==777:raise ValueError('missing advertised jukebox song registry')
                    if songs is not None:
                        # Номер песни относится к реестру данного соединения.
                        # Сравниваем имена, а не порядок данного соединения,
                        # сохраняя исходный номер в сыром пакете.
                        if not 0 <= value['data'] < len(songs):raise ValueError('jukebox song outside advertised registry')
                        value['data']='song:'+songs[value['data']]
                if e['name'] in ('rel_entity_move','entity_move_look') and 'move' in value:
                    movement=value.pop('move')
                    # Один delta777 имеет тот же масштаб1/4096, что старые dX/dY/dZ.
                    # [проверить] Многотактовые пачки: не скрываем их как нулевое движение.
                    if len(movement['steps'])!=1:raise ValueError('multi-step entityDelta not yet projected')
                    value.update(zip(('dX','dY','dZ'),movement['steps'][0]['delta']))
                    value['onGround']=movement['onGround']
                if e['name']=='sync_entity_position' and value.get('positionType')=='stepped':
                    steps=value.pop('steps')
                    if len(steps)!=1:raise ValueError('multi-step sync_entity_position not yet projected')
                    # Поля старого Sync из единственного шага; исходный payload
                    # со смещением интерполяции остаётся в сырой записи.
                    value.update(steps[0])
                packets.append({'client':i,'at':e['at'],'name':e['name'],'value':value})
            except Exception as error:errors.append({'client':i,'name':e['name'],'error':repr(error)})
    outgoing=Decoder(decoder.protocol,direction='toServer')
    for i,client in enumerate(record['clients']):
        if scenario.get('track_client_moves'):
            for e in client.get('outbound',[]):
                value=outgoing.read(io.BytesIO(bytes.fromhex(e['hex'])),'packet_'+e['name'])
                packets.append({'client':i,'at':e['at'],'name':e['name'],'value':value,'outbound':True})
        if i==1 and scenario.get('track_second_pos'):
            for e in client.get('moves',[]):packets.append({'client':i,'name':'move','at':e['at'],'value':e,'move':True})
        if (scenario.get('track_client_moves') or scenario.get('track_second_pos')) and 'moves' not in client:
            errors.append({'client':i,'error':'client movement observations missing'})
    # bot.js записывает record_ticks+10 и слушает ещё3 такта. Эти пакеты
    # уже есть в сырой записи; не обрезаем завершающие десять тактов.
    payload={**record,'packets':packets,'last':scenario['record_ticks']+10,'sounds':sounds}
    temp=output.with_suffix('.replay.json');temp.write_text(json.dumps(payload))
    subprocess.run(['node',str(Path(__file__).parent/'replay777.js'),str(temp),str(Path(__file__).parent/'scenarios'/f"{scenario['name']}.json"),str(data),str(output)],check=True)
    temp.unlink()
    return errors


def main():
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('--protocol',type=Path,required=True)
    a=p.parse_args();data=a.protocol.parent
    decoder=Decoder(json.loads(a.protocol.read_text()),json.loads((data/'entities.json').read_text()))
    if 'entityDelta' in decoder.protocol['types']:
        observed_attribute_names(decoder,json.loads((data/'protocol-26.1.json').read_text()))
    summary_text=(a.folder/'summary.json').read_text()
    summary=json.loads(summary_text)
    sound_file=a.folder/'sound-observations.json'
    sounds=json.loads(sound_file.read_text())['sounds'] if sound_file.exists() else json.loads((Path(__file__).parent/'data/sounds.json').read_text())
    for row in summary:
        paths=[a.folder/f"{row['scenario']}-{side}.json" for side in ('official','ours')]
        if not all(path.exists() for path in paths):continue
        scenario=json.loads((Path(__file__).parent/'scenarios'/f"{row['scenario']}.json").read_text())
        outputs=[path.with_name(path.stem+'-events.json') for path in paths]
        errors=[project_record(json.loads(path.read_text()),decoder,scenario,data,out,sounds) for path,out in zip(paths,outputs)]
        result=subprocess.run(['node',str(Path(__file__).parent/'compare.js'),row['scenario'],'--files',*map(str,outputs)],capture_output=True,text=True,check=True)
        (a.folder/f"{row['scenario']}-comparison.txt").write_text(result.stdout)
        counts=re.search(r'Совпало: (\d+). Сдвинуто по тактам: (\d+). Нет у нас: (\d+). Лишнее у нас: (\d+).',result.stdout)
        assert counts,'comparison summary missing'
        row['comparison_counts']=list(map(int,counts.groups()));row['decode_errors']=errors
        # Итог compare.js считает сдвиги/пропуски/лишние; заметки о порядке
        # оставляем отдельно, как в исходном отчёте, не меняя прежний вердикт775.
        row['ordering_notes']=result.stdout.count('порядок в такте другой')
        row['warnings']=[line for line in result.stdout.splitlines() if line.startswith('Внимание:')]
        bad=any(row['comparison_counts'][1:]) or any(errors) or bool(row['warnings'])
        row['status']='mismatch-open' if bad else 'match'
        row['reason']='original bot.js handlers and compare.js; '+str(row['comparison_counts'])+f'; decoder gaps {[len(v) for v in errors]}'
        print(row['scenario'],row['status'],row['reason'],flush=True)
    rendered=json.dumps(summary,indent=2)+'\n'
    (a.folder/'comparison-summary.json').write_text(rendered)
    if (a.folder/'summary.json').read_text()==summary_text:
        (a.folder/'summary.json').write_text(rendered)
    print(collections.Counter(r['status'] for r in summary))


if __name__=='__main__':main()
