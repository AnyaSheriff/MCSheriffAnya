#!/usr/bin/env python3
"""Обычные777 GUI cancel/Q/Ctrl+Q/F; enum только наблюдаем, не угадываем."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import time

import probe_actual_777_dig as base
from probe_registry_client import exact, integer, require_end
from probe_white_concrete_slab import slot, TARGET


def menu_slot(index):
    if 0<=index<9:return index+36
    if 9<=index<36:return index
    if 36<=index<40:return 44-index
    if index==40:return 45
    raise ValueError('unexpected player-inventory index')


class ActionObserver777(base.Observer777):
    """Общие777фазы/действия наследуются; добавлены только две выбранные руки."""
    def __init__(self,*args,**kwargs):
        super().__init__(*args,**kwargs)
        self.offhand=None
        self.authority={}

    def observe(self,direction,body):
        data=io.BytesIO(body);ident=integer(data);payload=data.read()
        name=self.names['play'][direction].get(ident)
        if (self.phase[direction]!='play' or direction!='toClient'
                or name not in {'window_items','set_slot','set_player_inventory','set_cursor_item'}):
            return super().observe(direction,body)
        key=direction+'/play/'+name;self.counts[key]=self.counts.get(key,0)+1
        row={'direction':direction,'phase':'play','name':name,'id':ident,'bytes':len(payload),
             'elapsed':self.clock()-self.began,'stage':self.stage}
        data=io.BytesIO(payload)
        if name=='window_items':
            window=integer(data);revision=integer(data);count=integer(data)
            if window!=0 or count!=46:raise ValueError('unexpected player window')
            values=[slot(data) for _ in range(count)]
            row.update(mainhand=values[36],offhand=values[45],cursor=slot(data),revision=revision)
        elif name=='set_cursor_item':row['cursor']=slot(data)
        else:
            if name=='set_slot':
                window=integer(data);row['revision']=integer(data)
                index=struct.unpack('>h',exact(data,2))[0]
            else:
                window=0;index=menu_slot(integer(data))
            value=slot(data);require_end(data,name)
            if window!=0 or index not in (36,45):return
            row['mainhand' if index==36 else 'offhand']=value;row['menu_slot']=index
        require_end(data,name)
        for field in ('mainhand','offhand'):
            if field in row:
                setattr(self,field,row[field]);self.authority[field]=len(self.rows)
        self.add(row)


def stack(item,count):return None if item is None else {'item':item,'count':count}


def hand_ready(observer,main,off,after):
    return (observer.mainhand==main and observer.offhand==off
            and all(observer.authority.get(hand,-1)>=after for hand in ('mainhand','offhand')))


def actions(rows,start):
    return [row for row in rows[start:] if row['direction']=='toServer' and row['name']=='block_dig']


def cancellation_trace(rows,start,released_at):
    observed=actions(rows,start)
    if not observed:return False
    if any(row['target']!=list(TARGET) for row in observed):raise ValueError('cancel aimed at another block')
    if observed[0]['status']!=0:raise ValueError('cancel did not start with observed START0')
    if any(update['state']==0 for row in rows[start:] for update in row.get('updates',[])):
        raise ValueError('cancelled target broke')
    after=[row for row in observed[1:] if row['elapsed']>=released_at and row['status'] not in (0,3)]
    if len({row['status'] for row in after})>1:raise ValueError('ambiguous post-release action statuses')
    return bool(after)


def effect_evidence(rows,start,main,off,drop=None,server_hands=None):
    observed=actions(rows,start)
    if not observed:return False
    if len(observed)!=1:raise ValueError('expected one ordinary key action, not repeat')
    updates=[row for row in rows[start:] if row['direction']=='toClient']
    main_ok=(server_hands is not None and 'mainhand' in server_hands and server_hands['mainhand']==main) or any('mainhand' in row and row['mainhand']==main for row in updates)
    off_ok=off=='unchanged' or (server_hands is not None and 'offhand' in server_hands and server_hands['offhand']==off) or any('offhand' in row and row['offhand']==off for row in updates)
    if not(main_ok and off_ok):return False
    drops={row['entity']:row['slot'] for row in updates if row['name']=='entity_metadata' and row.get('slot')}
    if drop is None:
        if drops:raise ValueError('swap unexpectedly spawned an item')
        return True
    if not drops:return False
    if any(value['item']!=drop['item'] for value in drops.values()):raise ValueError('wrong dropped item')
    return sum(value['count'] for value in drops.values())==drop['count']


def verify_calibration(raw,sha,label,expected):
    value=json.loads(raw)
    if (value.get('screenshot_sha256')!=sha or value.get('case')!=label
            or value.get('ground_hud_verified') is not True
            or value.get('inventory_counts_verified') is not True
            or value.get('expected_hands')!=expected):
        raise ValueError('need inspected current image, ground/HUD and exact hand counts')
    if label=='cancel' and (value.get('crosshair_target_verified') is not True or value.get('target')!=list(TARGET)):
        raise ValueError('need inspected obsidian crosshair target')
    return value


def hand_predicates(label,phase,main_name,off_name,main_count=0,off_count=0):
    result=[]
    for hand,name,count in [('mainhand',main_name,main_count),('offhand',off_name,off_count)]:
        token='MCSA_ACTION_'+label+'_'+phase+'_'+hand
        condition='unless' if name is None else 'if'
        if hand=='mainhand':
            path='SelectedItem'+('' if name is None else '{count:'+str(count)+',id:"minecraft:'+name+'"}')
        else:
            path='equipment.offhand'+('' if name is None else '{count:'+str(count)+',id:"minecraft:'+name+'"}')
        result.append((token,'execute '+condition+' data entity McsaProbe '+path+' run say '+token))
    return result


def press_action(client,label):
    if label=='cancel':base.gui(client,'mousedown','1')
    elif label=='drop-one':base.gui(client,'key','q')
    elif label=='drop-stack':
        base.gui(client,'keydown','Control_L');base.gui(client,'key','q')
    elif label=='swap':base.gui(client,'key','f')
    else:raise ValueError('unexpected GUI action')


def release_action(client,label):
    if label=='cancel':base.gui(client,'mouseup','1')
    elif label=='drop-stack':base.gui(client,'keyup','Control_L')
    elif label not in ('drop-one','swap'):raise ValueError('unexpected GUI release')


class Scenario:
    observer_type=ActionObserver777
    scope='actual official777 cancel/drop1/dropstack/swaphands only; no enum inference or native swap implementation'

    def __init__(self,only_case=None):
        self.only_case=only_case
        if only_case:self.scope += "; selected case="+only_case

    def run(self,context):
        report,observer,client=(context[key] for key in ('report','observer','client'))
        barrier,wait,snapshot=(context[key] for key in ('barrier','wait','snapshot'))
        output,log=context['output'],context['log']
        items={item['name']:item['id'] for item in context['tables']['items.json']}
        obsidian=next(block for block in context['tables']['blocks.json'] if block['name']=='obsidian')
        assert obsidian['minStateId']==obsidian['maxStateId']
        report['scenario_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        report['fixtures']={'items':{name:items[name] for name in ('stone','cobblestone','iron_ingot','gold_ingot')},
                            'obsidian_state':obsidian['minStateId'],'cancel_hold_seconds':.2}
        cases=[('cancel',None,0,None,0),('drop-one','stone',7,None,0),
               ('drop-stack','cobblestone',11,None,0),('swap','iron_ingot',3,'gold_ingot',5)]
        if self.only_case:cases=[case for case in cases if case[0]==self.only_case]
        report['selected_cases']=[case[0] for case in cases]
        for label,main_name,main_count,off_name,off_count in cases:
            observer.stage=label+'-setup';setup_at=len(report['packets'])
            for text in ['kill @e[type=minecraft:item]',
                         'item replace entity McsaProbe weapon.mainhand with minecraft:stone',
                         'item replace entity McsaProbe weapon.offhand with minecraft:gold_ingot',
                         'clear McsaProbe','gamemode survival McsaProbe',
                         'setblock 2 65 0 minecraft:'+('obsidian' if label=='cancel' else 'air'),
                         'tp McsaProbe 0.5 65 0.5 -90 30']:
                barrier(text)
            for hand,name,count in [('mainhand',main_name,main_count),('offhand',off_name,off_count)]:
                if name:barrier(f'item replace entity McsaProbe weapon.{hand} with minecraft:{name} {count}')
            expected={'mainhand':stack(items[main_name],main_count) if main_name else None,
                      'offhand':stack(items[off_name],off_count) if off_name else None}
            ready_at=time.monotonic()-observer.began
            wait(lambda:hand_ready(observer,expected['mainhand'],expected['offhand'],setup_at) and
                 base.ground_ready(report['packets'],ready_at,time.monotonic()-observer.began) and
                 observer.acks==observer.teleports and all(v is not None and abs(v-e)<.1 for v,e in zip(observer.pose[3:],(-90.,30.))),20)

            def verify_hands(phase,main,off,main_count=0,off_count=0):
                for token,text in hand_predicates(label,phase,main,off,main_count,off_count):
                    barrier(text);wait(lambda token=token:('[Server] '+token) in log.read_text(),6)
            # Ограниченные обычные запросы только двух собственных fixture stacks.
            # equipment.offhand — гипотеза пути, до положительного predicate не факт.
            barrier('data get entity McsaProbe Inventory')
            barrier('data get entity McsaProbe equipment.offhand')
            verify_hands('BEFORE',main_name,off_name,main_count,off_count)
            token='MCSA_ACTION_'+label+'_SURVIVAL'
            barrier('execute if entity @a[name=McsaProbe,gamemode=survival] run say '+token)
            wait(lambda:('[Server] '+token) in log.read_text(),6)
            if label=='cancel':
                token='MCSA_ACTION_cancel_OBSIDIAN_BEFORE'
                barrier('execute if block 2 65 0 minecraft:obsidian run say '+token)
                wait(lambda:('[Server] '+token) in log.read_text(),6)
            sha=snapshot(label+'-before')
            approval=output/('calibration-'+label+'.json')
            (output/'checkpoint.json').write_text(json.dumps({'case':label,'waiting_for':approval.name,
                'screenshot':label+'-before.png','screenshot_sha256':sha,'expected_hands':expected,
                'target':list(TARGET) if label=='cancel' else None,'mouse_not_pressed':True,
                'required':['ground_hud_verified','inventory_counts_verified']+(['crosshair_target_verified'] if label=='cancel' else [])},indent=2)+'\n')
            wait(approval.is_file,120);verify_calibration(approval.read_text(),sha,label,expected)
            wait(lambda:base.ground_ready(report['packets'],ready_at,time.monotonic()-observer.began) and
                 observer.mainhand==expected['mainhand'] and observer.offhand==expected['offhand'] and
                 observer.acks==observer.teleports,5)
            report.setdefault('calibration',[]).append({'case':label,'screenshot_sha256':sha,'verified':True,'expected_hands':expected})
            observer.stage=label+'-action';start=len(report['packets'])
            before=time.monotonic()
            timing={'case':label}
            report.setdefault('input_events',[]).append(timing)
            def press():
                timing['press_requested']=time.monotonic()-observer.began
                press_action(client,label)
            def release():
                timing['release_requested']=time.monotonic()-observer.began
                release_action(client,label)
                timing['release_completed']=time.monotonic()-observer.began
            server_hands=None
            def evidence():
                if label=='cancel':return cancellation_trace(report['packets'],start,timing['release_requested'])
                if label=='drop-one':return effect_evidence(report['packets'],start,stack(items['stone'],6),'unchanged',stack(items['stone'],1),server_hands)
                if label=='drop-stack':return effect_evidence(report['packets'],start,None,'unchanged',stack(items['cobblestone'],11),server_hands)
                return effect_evidence(report['packets'],start,stack(items['gold_ingot'],5),stack(items['iron_ingot'],3),server_hands=server_hands)
            def diagnose():
                # Выполняется даже при исходной ошибке; она не заменяется диагнозом.
                barrier('data get entity McsaProbe SelectedItem')
                if label=='cancel':
                    token='MCSA_ACTION_cancel_OBSIDIAN_AFTER'
                    barrier('execute if block 2 65 0 minecraft:obsidian run say '+token)
                    wait(lambda:('[Server] '+token) in log.read_text(),6)
            def hold():
                if label=='cancel':time.sleep(.2)
                # Ctrl остаётся нажатым до реального action, не отпускается
                # в том же frame раньше обработки Q игровым клиентом.
                elif label=='drop-stack':wait(lambda:bool(actions(report['packets'],start)),2)
            after_names={'cancel':(None,None,0,0),'drop-one':('stone',None,6,0),
                         'drop-stack':(None,None,0,0),'swap':('gold_ingot','iron_ingot',5,3)}
            def confirm_effect():
                nonlocal server_hands
                wait(lambda:bool(actions(report['packets'],start)),5)
                # GUI01: оригинал подтверждает Q дропом, но owner-slot echo не шлёт.
                # Проверяем серверные NBT count/id ОБЕИХ рук, не клиентское предсказание.
                verify_hands('AFTER',*after_names[label])
                main,off,mc,oc=after_names[label]
                server_hands={'mainhand':stack(items[main],mc) if main else None,
                              'offhand':stack(items[off],oc) if off else None}
                wait(evidence,5)
            base.run_mouse_dig(press,release,hold,confirm_effect,diagnose,
                               report.setdefault('action_diagnostic_errors',[]))
            # Никакой enum-таблицы по номерам: сохраняем реальные выбранные actions.
            report.setdefault('observations',[]).append({'case':label,'real_actions':actions(report['packets'],start),
                'elapsed_seconds':time.monotonic()-before,'server_hands':server_hands,'last_owner_slot_echo':{'mainhand':observer.mainhand,'offhand':observer.offhand},
                'corroboration':'exact positive server NBT hand count/id predicates'+(' + retained obsidian' if label=='cancel' else ' + item entity metadata' if label.startswith('drop') else '')})
            post_at=time.monotonic()-observer.began
            wait(lambda:base.ground_ready(report['packets'],post_at,time.monotonic()-observer.began),10)
            snapshot(label+'-after');report['checks'].append(label+' real GUI action and corroborating authoritative result')


if __name__=='__main__':
    selector=argparse.ArgumentParser(add_help=False)
    selector.add_argument('--case',choices=['cancel','drop-one','drop-stack','swap'])
    selection,remaining=selector.parse_known_args()
    sys.argv=[sys.argv[0],*remaining]
    base.main(Scenario(selection.case))
