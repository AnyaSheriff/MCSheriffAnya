"""Синтетические guards проверяют доказательства, не заменяют fresh paired."""
import copy
import unittest
from probe_inventory_reconnect_777 import (CASES, STAGES, JAR_SHA1, click_payload,
                                          compare, replay, totals, validate, use_payload)
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
                'snapshots':[], 'sessions':[], 'ground_events':[]}
    for kind, mode in CASES:
        for phase in ('before', 'after'):
            stage = kind+'-'+mode+'-'+phase
            inv = {'window':2 if kind in ('chest','workbench') else 0, 'revision':4,
                   'slots':[None]*(63 if kind=='chest' else 46), 'cursor':None}
            stone = {'item':'stone', 'count':7 if kind=='chest' else 3}
            if phase=='before':
                inv['cursor'] = stone
            else:
                inv['slots'][27 if kind=='chest' else 10 if kind=='workbench' else 9] = stone
            if kind=='chest':
                inv['slots'][1] = {'item':'dirt', 'count':5}
            elif kind in ('crafting','workbench'):
                inv['slots'][1 if phase=='before' else 11 if kind=='workbench' else 10] = {'item':'oak_planks', 'count':2}
                if phase=='before':
                    inv['slots'][0] = {'item':'oak_button', 'count':1}
            initial = {'window':0, 'revision':0, 'slots':[None]*46, 'cursor':None}
            packets = [{'direction':'in', 'name':'login', 'payload':''},packet(initial)]
            if kind in ('chest','workbench'):
                packets += [{'direction':'out','name':'block_place','payload':use_payload(2,64,0,1).hex()},
                            {'direction':'in','name':'open_window','payload':(b'\2'+varint(20 if kind=='chest' else 13)).hex()}]
            if phase=='before':
                for place in ([0] if kind=='chest' else [11,1,10] if kind=='workbench' else [10,1,9] if kind=='crafting' else [9]):
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
                                          'inventory':inv, 'ground':[], 'observer_packet_index':1, 'totals':totals(inv)})
    core = copy.deepcopy(official)
    core['target'] = 'core'
    core['jar_sha1'] = None
    return official,core


class EvidenceTests(unittest.TestCase):
    def test_all_sixteen_stages_and_both_disconnect_modes(self):
        official, core = reports()
        self.assertEqual([row['stage'] for row in validate(official,'official')],STAGES)
        self.assertEqual(len(STAGES),16)
        self.assertEqual(len(CASES),8)
        self.assertEqual(compare(official,core),[])

    def test_preview_is_not_owned_but_crafting_inputs_and_cursor_are(self):
        official,_ = reports()
        row = official['snapshots'][4]
        self.assertEqual(row['inventory']['slots'][0],{'item':'oak_button','count':1})
        self.assertEqual(totals(row['inventory']),{'oak_planks':2,'stone':3})

    def test_workbench_preview_inputs_real_clicks_and_scope(self):
        official,_ = reports()
        row = official['snapshots'][12]
        self.assertEqual(row['stage'],'workbench-close-fin-before')
        self.assertNotEqual(row['inventory']['window'],0)
        self.assertEqual(len(row['inventory']['slots']),46)
        self.assertEqual(row['inventory']['slots'][0],{'item':'oak_button','count':1})
        self.assertEqual(totals(row['inventory']),{'oak_planks':2,'stone':3})
        self.assertEqual(official['snapshots'][13]['inventory']['slots'][11],{'item':'oak_planks','count':2})
        for mutation in ('missing-click','wrong-window','count-preview','missing-use'):
            modified = copy.deepcopy(official)
            target = modified['snapshots'][12]
            if mutation=='missing-click':
                modified['sessions'][12]['packets'][4]['name'] = 'tick_end'
            elif mutation=='missing-use':
                modified['sessions'][12]['packets'][2]['name'] = 'tick_end'
            elif mutation=='wrong-window':
                target['inventory']['window'] = 0
                modified['sessions'][12]['packets'][target['packet_index']-1] = packet(target['inventory'])
            else:
                target['totals']['oak_button'] = 1
            with self.assertRaises(ValueError): validate(modified,'official')

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

    def test_ground_spawn_metadata_update_and_destroy_preserve_history(self):
        spawn = varint(123)+bytes(16)+varint(72)
        packets = [
            {'direction':'in','name':'spawn_entity','payload':spawn.hex()},
            {'direction':'in','name':'entity_metadata','payload':(varint(123)+b'\x08\x07\x03\x01\0\0\xff').hex()},
            {'direction':'in','name':'entity_metadata','payload':(varint(123)+b'\x08\x07\x02\x01\0\0\xff').hex()},
            {'direction':'in','name':'entity_destroy','payload':(b'\1'+varint(123)).hex()}]
        client = replay(packets, NAMES)
        self.assertEqual(client.ground_stacks,{})
        self.assertEqual(client.drops_seen,{123})
        self.assertEqual([e['event'] for e in client.ground_events],['spawn','stack','stack','destroy'])
        self.assertEqual(client.ground_events[-1]['stack'],{'item':'stone','count':2})

    def test_ground_metadata_rejects_missing_unknown_duplicate_and_truncated(self):
        spawn = {'direction':'in','name':'spawn_entity','payload':(b'\5'+bytes(16)+varint(72)).hex()}
        from probe_inventory_reconnect_777 import ground_snapshot
        with self.assertRaises(AssertionError): ground_snapshot(replay([spawn],NAMES))
        for payload in ('05080703010000', '05080300000000ff', '05080703010000080703010000ff',
                        '0508070301010001ff', '0508070000ff', '05080703010000ff00'):
            with self.assertRaises((AssertionError,EOFError,IndexError)):
                replay([spawn,{'direction':'in','name':'entity_metadata','payload':payload}],NAMES)
        with self.assertRaises(AssertionError):
            replay([spawn,{'direction':'in','name':'entity_destroy','payload':'0105'}],NAMES)

    def test_rst_ground_is_counted_and_compared_without_entity_ids(self):
        official,core = reports()
        for report,entity in ((official,55),(core,77)):
            spawn = {'direction':'in','name':'spawn_entity','payload':(varint(entity)+bytes(16)+varint(72)).hex()}
            metadata = {'direction':'in','name':'entity_metadata','payload':(varint(entity)+b'\x08\x07\3\1\0\0\xff').hex()}
            # RST personal-after row3: stone moved to ground, absent from bag.
            report['observer'] += [spawn,metadata]
            row = report['snapshots'][3]
            row['inventory']['slots'][9] = None
            row['ground'] = [{'item':'stone','count':3}]
            row['observer_packet_index'] = 3
            report['sessions'][3]['packets'][-1] = packet(row['inventory'])
            # Cleanup history follows snapshot, before next case.
            report['observer'].append({'direction':'in','name':'entity_destroy','payload':(b'\1'+varint(entity)).hex()})
            for later in report['snapshots'][4:]: later['observer_packet_index'] = 4
            report['ground_events'] = replay(report['observer'],NAMES).ground_events
        self.assertEqual(compare(official,core),[])
        core['snapshots'][3]['ground'][0]['count'] = 2
        with self.assertRaises(ValueError): validate(core,'core')

    def test_ground_missing_raw_history_or_invented_counts_fail(self):
        official,_ = reports()
        official['ground_events'] = [{'event':'spawn','entity':9}]
        with self.assertRaises(ValueError): validate(official,'official')
        official,_ = reports()
        official['snapshots'][0]['ground'] = [{'item':'stone','count':3}]
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
