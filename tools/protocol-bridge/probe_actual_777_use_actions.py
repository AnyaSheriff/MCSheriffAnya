#!/usr/bin/env python3
"""Обычные777 bow release / spear jab: enum только из реального GUI."""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import sys
import time

import probe_actual_777_dig as base
from probe_actual_777_actions import ActionObserver777, actions, menu_slot, verify_calibration
from probe_registry_client import exact, integer, require_end


def uint(data):
    value=integer(data)
    if not 0<=value<=0x7fffffff:raise ValueError('invalid unsigned value')
    return value


def use_slot(data):
    """Свой bounded decoder лишь count/id/damage/enchantments (pinned777 JSON)."""
    count=uint(data)
    if not count:return None
    if count>127:raise ValueError('unexpected stack count')
    result={'item':uint(data),'count':count}
    added,removed=uint(data),uint(data)
    if added>2 or removed:raise ValueError('unsupported fixture components')
    seen=set()
    for _ in range(added):
        kind=uint(data)
        if kind in seen:raise ValueError('duplicate component')
        seen.add(kind)
        if kind==3:result['damage']=uint(data)
        elif kind==13:
            count=uint(data)
            if count>8:raise ValueError('too many fixture enchantments')
            values=[]
            for _ in range(count):values.append({'id':uint(data),'level':uint(data)})
            if len({v['id'] for v in values})!=len(values):raise ValueError('duplicate enchantment')
            result['enchantments']=values
        else:raise ValueError('unsupported fixture component '+str(kind))
    return result


class UseObserver777(ActionObserver777):
    arrow_entity=None

    def observe(self,direction,body):
        data=io.BytesIO(body);ident=integer(data);payload=data.read()
        name=self.names['play'][direction].get(ident)
        if self.phase[direction]!='play':return super().observe(direction,body)
        inventory={'window_items','set_slot','set_player_inventory','set_cursor_item'}
        if not ((direction=='toClient' and name in inventory|{'spawn_entity'}) or
                (direction=='toServer' and name=='use_item')):
            return super().observe(direction,body)
        data=io.BytesIO(payload)
        if name=='spawn_entity':
            entity=integer(data);exact(data,16);kind=integer(data)
            if kind!=self.arrow_entity:return super().observe(direction,body)
            position=list(struct.unpack('>ddd',exact(data,24)))
            if not all(math.isfinite(v) for v in position):raise ValueError('nonfinite arrow spawn')
            if not all(abs(v-e)<12 for v,e in zip(position,(.5,65,.5))):return
            values={'entity':entity,'entity_type':kind,'position':position}
            # Сохраняем лишь положительный spawn выбранной arrow, не полный payload.
        elif name=='use_item':
            hand,sequence=uint(data),uint(data)
            rotation=list(struct.unpack('>ff',exact(data,8)));require_end(data,name)
            if hand not in (0,1) or not all(math.isfinite(v) for v in rotation):raise ValueError('invalid use_item')
            values={'hand':hand,'sequence':sequence,'rotation':rotation,'payload_hex':payload.hex()}
        else:
            values={}
            if name=='window_items':
                window,revision,count=uint(data),uint(data),uint(data)
                if window!=0 or count!=46:raise ValueError('unexpected player window')
                slots=[use_slot(data) for _ in range(count)]
                values.update(mainhand=slots[36],offhand=slots[45],cursor=use_slot(data),revision=revision)
            elif name=='set_cursor_item':values['cursor']=use_slot(data)
            else:
                if name=='set_slot':
                    window=uint(data);values['revision']=uint(data);index=struct.unpack('>h',exact(data,2))[0]
                else:window=0;index=menu_slot(uint(data))
                value=use_slot(data);require_end(data,name)
                if window!=0 or index not in (36,45):return
                values['mainhand' if index==36 else 'offhand']=value
            require_end(data,name)
            for hand in ('mainhand','offhand'):
                if hand in values:setattr(self,hand,values[hand]);self.authority[hand]=len(self.rows)
        key=direction+'/play/'+name;self.counts[key]=self.counts.get(key,0)+1
        self.add({'direction':direction,'phase':'play','name':name,'id':ident,'bytes':len(payload),
                  'elapsed':self.clock()-self.began,'stage':self.stage,**values})


def observed_action(rows,start,released_at,label):
    selected=rows[start:];found=actions(rows,start)
    if label=='bow':
        uses=[r for r in selected if r['direction']=='toServer' and r['name']=='use_item']
        if not uses:return None
        if len(uses)!=1 or uses[0]['hand']!=0:raise ValueError('expected one mainhand bow use')
        if found and any(r['elapsed']<released_at for r in found):raise ValueError('bow action before mouse release')
        if found and found[0]['elapsed']-uses[0]['elapsed']<1.2:raise ValueError('bow hold shorter than 1.2 seconds')
    if not found:return None
    if len(found)!=1:raise ValueError('ambiguous ordinary GUI action')
    return found[0]


def corroborates(rows,start,action,label,arrow):
    if action is None:return False
    selected=[r for r in rows[start:] if r['elapsed']>=action['elapsed']]
    if label=='bow':return any(r['direction']=='toClient' and r['name']=='spawn_entity' and r.get('entity_type')==arrow for r in selected)
    # Настоящие движения клиента после серверного рывка; до клика он стоит.
    return any(r['direction']=='toServer' and 'motion' in r and r['motion']['pose'][0] is not None
               and r['motion']['pose'][0]>.8 for r in selected)


def predicates(label,phase):
    item='bow' if label=='bow' else 'iron_spear'
    component=',components:{"minecraft:damage":1}' if phase=='AFTER' else ''
    tests=[('MAIN','if data entity McsaProbe SelectedItem{count:1,id:"minecraft:'+item+'"'+component+'}')]
    if phase=='BEFORE':tests.append(('SURVIVAL','if entity @a[name=McsaProbe,gamemode=survival]'))
    if phase=='BEFORE':tests.append(('UNDAMAGED','if items entity McsaProbe weapon.mainhand minecraft:'+item+'[minecraft:damage=0]'))
    if label=='bow':tests.append(('AMMO','if data entity McsaProbe equipment.offhand{count:'+('3' if phase=='BEFORE' else '2')+',id:"minecraft:arrow"}'))
    else:
        tests.append(('OFF','unless data entity McsaProbe equipment.offhand'))
        if phase=='BEFORE':tests.append(('FOOD','if data entity McsaProbe {foodLevel:20}'))
    return [('MCSA_USE_'+label+'_'+phase+'_'+suffix,'execute '+condition+' run say MCSA_USE_'+label+'_'+phase+'_'+suffix) for suffix,condition in tests]


def release_all(client,errors):
    failure=None
    for args in [('mouseup','1'),('mouseup','3'),('keyup','Control_L')]:
        try:base.gui(client,*args)
        except BaseException as error:
            errors.append({'step':'release-input','input':list(args),'error':repr(error)})
            if failure is None:failure=error
    if failure is not None:raise failure


class Scenario:
    observer_type=UseObserver777
    scope='actual official777 GUI bow release/spear lunge jab; observed enums only'

    def __init__(self,only_case=None):self.only_case=only_case

    def run(self,context):
        report,observer,client=(context[k] for k in ('report','observer','client'))
        barrier,wait,snapshot=(context[k] for k in ('barrier','wait','snapshot'))
        output,log=context['output'],context['log']
        items={v['name']:v['id'] for v in context['tables']['items.json']}
        observer.arrow_entity=next(v['id'] for v in context['tables']['entities.json'] if v['name']=='arrow')
        report['scenario_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        report['fixtures']={'bow':items['bow'],'arrow':items['arrow'],'iron_spear':items['iron_spear'],
                            'arrow_entity':observer.arrow_entity,'bow_hold_seconds':1.3}
        for label in ([self.only_case] if self.only_case else ['bow','spear']):
            observer.stage=label+'-setup'
            failure=None;errors=report.setdefault('input_cleanup_errors',[])
            try:
                release_all(client,errors)
                for text in ['kill @e[type=minecraft:arrow]','clear McsaProbe','gamemode survival McsaProbe',
                             'difficulty normal','effect give McsaProbe minecraft:saturation 1 10 true',
                             'setblock 2 65 0 minecraft:air','setblock 2 66 0 minecraft:air',
                             'tp McsaProbe 0.5 65 0.5 -90 '+('-5' if label=='bow' else '0')]:barrier(text)
                tool='bow[damage=0]' if label=='bow' else 'iron_spear[damage=0,enchantments={lunge:1}]'
                barrier('item replace entity McsaProbe weapon.mainhand with minecraft:'+tool)
                if label=='bow':barrier('item replace entity McsaProbe weapon.offhand with minecraft:arrow 3')
                ready_at=time.monotonic()-observer.began
                pitch=-5. if label=='bow' else 0.
                wait(lambda:base.ground_ready(report['packets'],ready_at,time.monotonic()-observer.began) and
                     observer.acks==observer.teleports and all(v is not None and abs(v-e)<.1 for v,e in zip(observer.pose[3:],(-90.,pitch))),20)
                def confirm(phase):
                    for token,text in predicates(label,phase):
                        barrier(text);wait(lambda token=token:('[Server] '+token) in log.read_text(),6)
                confirm('BEFORE')
                expected={'mainhand':{'item':items['bow' if label=='bow' else 'iron_spear'],'count':1,'damage':0},
                          'offhand':{'item':items['arrow'],'count':3} if label=='bow' else None}
                sha=snapshot(label+'-before');approval=output/('calibration-'+label+'.json')
                (output/'checkpoint.json').write_text(json.dumps({'case':label,'waiting_for':approval.name,
                    'screenshot':label+'-before.png','screenshot_sha256':sha,'expected_hands':expected,
                    'mouse_not_pressed':True,'required':['ground_hud_verified','inventory_counts_verified','crosshair_air_verified']},indent=2)+'\n')
                wait(approval.is_file,120)
                approved=verify_calibration(approval.read_text(),sha,label,expected)
                if approved.get('crosshair_air_verified') is not True:raise ValueError('inspect clear air crosshair')
                report.setdefault('calibration',[]).append(approved)
                wait(lambda:base.ground_ready(report['packets'],ready_at,time.monotonic()-observer.began),5)
                observer.stage=label+'-action';start=len(report['packets'])
                event={'case':label,'press_requested':time.monotonic()-observer.began}
                report.setdefault('input_events',[]).append(event)
                button='3' if label=='bow' else '1'
                def press():base.gui(client,'mousedown',button)
                def hold():
                    if label=='bow':
                        wait(lambda:any(r['name']=='use_item' for r in report['packets'][start:]),3)
                        time.sleep(1.3)
                    else:wait(lambda:bool(actions(report['packets'],start)),3)
                def release():
                    event['release_requested']=time.monotonic()-observer.began
                    base.gui(client,'mouseup',button);event['release_completed']=time.monotonic()-observer.began
                def after():
                    wait(lambda:observed_action(report['packets'],start,event['release_requested'],label) is not None,5)
                    confirm('AFTER')
                    wait(lambda:corroborates(report['packets'],start,observed_action(report['packets'],start,event['release_requested'],label),label,observer.arrow_entity),5)
                def diagnose():
                    barrier('data get entity McsaProbe SelectedItem')
                    barrier('data get entity McsaProbe equipment.offhand')
                base.run_mouse_dig(press,release,hold,after,diagnose,report.setdefault('action_diagnostic_errors',[]))
                report.setdefault('observations',[]).append({'case':label,'real_actions':actions(report['packets'],start),
                    'positive_predicates':predicates(label,'AFTER'),'corroboration':'arrow spawn' if label=='bow' else 'positive +X displacement after lunge; no movement keys',
                    'selected_events':report['packets'][start:]})
                snapshot(label+'-after');report['checks'].append(label+' real GUI action + exact wear/ammo + positive effect')
            except BaseException as error:failure=error
            finally:
                try:release_all(client,errors)
                except BaseException as error:
                    if failure is None:failure=error
            if failure is not None:raise failure


if __name__=='__main__':
    selector=argparse.ArgumentParser(add_help=False);selector.add_argument('--case',choices=['bow','spear'])
    selection,remaining=selector.parse_known_args();sys.argv=[sys.argv[0],*remaining]
    base.main(Scenario(selection.case))
