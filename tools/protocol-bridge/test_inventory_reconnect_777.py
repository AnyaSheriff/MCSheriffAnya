"""Синтетические guards проверяют доказательства, не заменяют fresh paired."""
import copy
import unittest
from probe_inventory_reconnect_777 import (CASES, STAGES, JAR_SHA1, click_payload,
                                          compare, replay, totals, validate)
from smoke import varint

NAMES = {1:'stone', 2:'oak_planks', 3:'dirt', 4:'oak_button'}


def slot(value):
    if value is None:
        return b'\0'
    item = next(key for key,name in NAMES.items() if name==value['item'])
    return varint(value['count'])+varint(item)+b'\0\0'


def packet(inv):
    payload = varint(inv['window'])+varint(inv['revision'])+varint(len(inv['slots']))
    payload += b''.join(slot(value) for value in inv['slots'])+slot(inv['cursor'])
    return {'direction':'in', 'name':'window_items', 'payload':payload.hex()}


def reports():
    official = {'protocol':777, 'target':'official', 'status':'PASS', 'exit_code':0,
                'jar_sha1':JAR_SHA1, 'item_names':NAMES, 'behavior_failures':[],
                'observer':[{'direction':'in', 'name':'login', 'payload':''}],
                'snapshots':[], 'sessions':[]}
    for kind, mode in CASES:
        for phase in ('before', 'after'):
            stage = kind+'-'+mode+'-'+phase
            inv = {'window':2 if kind=='chest' else 0, 'revision':4,
                   'slots':[None]*(63 if kind=='chest' else 46), 'cursor':None}
            stone = {'item':'stone', 'count':7 if kind=='chest' else 3}
            if phase=='before':
                inv['cursor'] = stone
            else:
                inv['slots'][27 if kind=='chest' else 9] = stone
            if kind=='chest':
                inv['slots'][1] = {'item':'dirt', 'count':5}
            elif kind=='crafting':
                inv['slots'][1 if phase=='before' else 10] = {'item':'oak_planks', 'count':2}
                if phase=='before':
                    inv['slots'][0] = {'item':'oak_button', 'count':1}
            initial = {'window':0, 'revision':0, 'slots':[None]*46, 'cursor':None}
            packets = [{'direction':'in', 'name':'login', 'payload':''},packet(initial)]
            if phase=='before':
                for place in ([0] if kind=='chest' else [10,1,9] if kind=='crafting' else [9]):
                    packets.append({'direction':'out', 'name':'window_click',
                                    'payload':click_payload(inv['window'],3,place).hex()})
            packets.append(packet(inv))
            index = len(packets)
            if phase=='before' and mode=='close-fin':
                packets.append({'direction':'out', 'name':'close_window', 'payload':varint(inv['window']).hex()})
            session = {'name':stage, 'packets':packets}
            if phase=='before':
                session['disconnect'] = mode
            official['sessions'].append(session)
            official['snapshots'].append({'stage':stage, 'packet_index':index,
                                          'inventory':inv, 'totals':totals(inv)})
    core = copy.deepcopy(official)
    core['target'] = 'core'
    core['jar_sha1'] = None
    return official,core


class EvidenceTests(unittest.TestCase):
    def test_all_twelve_stages_and_both_disconnect_modes(self):
        official, core = reports()
        self.assertEqual([row['stage'] for row in validate(official,'official')],STAGES)
        self.assertEqual(compare(official,core),[])

    def test_preview_is_not_owned_but_crafting_inputs_and_cursor_are(self):
        official,_ = reports()
        row = official['snapshots'][4]
        self.assertEqual(row['inventory']['slots'][0],{'item':'oak_button','count':1})
        self.assertEqual(totals(row['inventory']),{'oak_planks':2,'stone':3})

    def test_replay_does_not_trust_fabricated_snapshot_or_totals(self):
        for field in ('inventory', 'totals'):
            official,_ = reports()
            row = official['snapshots'][0]
            if field=='inventory':
                row[field]['cursor']['count'] = 4
            else:
                row[field]['stone'] = 4
            with self.assertRaises(ValueError):
                validate(official,'official')

    def test_missing_reordered_or_duplicate_stages_rejected(self):
        for mutation in ('missing','reordered','duplicate'):
            official,_ = reports()
            if mutation=='missing': official['snapshots'].pop()
            elif mutation=='reordered': official['snapshots'].reverse()
            else: official['snapshots'][1] = copy.deepcopy(official['snapshots'][0])
            with self.assertRaises(ValueError): validate(official,'official')

    def test_spawn_then_destroy_still_fails_offline_world_accounting(self):
        official,_ = reports()
        spawn = varint(123)+bytes(16)+varint(72)
        official['observer'] += [
            {'direction':'in','name':'spawn_entity','payload':spawn.hex()},
            {'direction':'in','name':'entity_destroy','payload':(b'\1'+varint(123)).hex()}]
        client = replay(official['observer'], NAMES)
        self.assertEqual(client.ground_items,set())
        self.assertEqual(client.drops_seen,{123})
        with self.assertRaises(ValueError): validate(official,'official')

    def test_missing_observer_click_disconnect_or_raw_inventory_rejected(self):
        for mutation in ('observer','click','disconnect','inventory'):
            official,_ = reports()
            session = official['sessions'][0]
            if mutation=='observer': official['observer'] = []
            elif mutation=='click': session['packets'][2]['name'] = 'tick_end'
            elif mutation=='disconnect': del session['disconnect']
            else: session['packets'][3]['name'] = 'tick_end'
            with self.assertRaises(ValueError): validate(official,'official')

    def test_rst_case_must_not_close_window(self):
        official,_ = reports()
        official['sessions'][2]['packets'].append({'direction':'out','name':'close_window','payload':'00'})
        with self.assertRaises(ValueError): validate(official,'official')

    def test_failed_official_is_not_accepted_as_expectation(self):
        for change in ({'exit_code':1},{'status':'INCOMPLETE'},{'jar_sha1':'0'*40},
                       {'behavior_failures':['loss']},{'error':'timeout'}):
            official,core = reports()
            official.update(change)
            with self.assertRaises(ValueError): compare(official,core)

    def test_count_loss_in_matching_raw_and_snapshot_still_rejected(self):
        official,_ = reports()
        row = official['snapshots'][1]
        row['inventory']['slots'][9]['count'] = 2
        row['totals'] = totals(row['inventory'])
        official['sessions'][1]['packets'][-1] = packet(row['inventory'])
        with self.assertRaises(ValueError): validate(official,'official')

    def test_comparison_keeps_exact_slots_but_ignores_window_id_revision(self):
        official,core = reports()
        row = core['snapshots'][1]
        row['inventory']['slots'][10] = row['inventory']['slots'][9]
        row['inventory']['slots'][9] = None
        core['sessions'][1]['packets'][-1] = packet(row['inventory'])
        self.assertEqual(compare(official,core),[{'stage':row['stage'],'field':'slots'}])
        official,core = reports()
        for row,session in zip(core['snapshots'],core['sessions']):
            row['inventory']['revision'] = 888
            if row['inventory']['window']: row['inventory']['window'] = 5
            index = row['packet_index']-1
            session['packets'][index] = packet(row['inventory'])
        self.assertEqual(compare(official,core),[])


if __name__=='__main__':
    unittest.main()
