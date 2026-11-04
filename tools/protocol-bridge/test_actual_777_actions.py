"""Только собственные синтетические frames/fixtures; никаких MC/Xvfb/network."""
import io
import json
import os
from pathlib import Path
import struct
import unittest
from unittest.mock import patch
import probe_actual_777_actions as p
from probe_white_concrete_slab import inputs
from smoke import varint

CACHE=Path(os.environ.get('MCSA_777_DATA','~/.local/share/mcsa-java-26.3-probe/candidate-json')).expanduser()


class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.tables=inputs(CACHE)
    def observer(self):
        o=p.ActionObserver777(self.tables['protocol.json'],[],72,clock=lambda:10.)
        o.phase=dict.fromkeys(o.phase,'play');return o
    def packet(self,o,name,payload):
        ident=next(i for i,n in o.names['play']['toClient'].items() if n==name)
        o.observe('toClient',varint(ident)+payload)
    def item(self,item,count):return varint(count)+varint(item)+b'\0\0'
    def action(self,status=77,elapsed=3.):
        # 77 — синтетическое неизвестное значение, НЕ предположение enum777.
        return {'direction':'toServer','name':'block_dig','status':status,'target':[2,65,0],'elapsed':elapsed}
    def update(self,**values):return {'direction':'toClient','name':'set_slot',**values}
    def drop(self,item,count,entity=1):
        return {'direction':'toClient','name':'entity_metadata','entity':entity,'slot':{'item':item,'count':count}}

    def test_player_indices_and_menu_slots_are_explicitly_distinct(self):
        for source,target in [(0,36),(8,44),(9,9),(35,35),(36,8),(37,7),(38,6),(39,5),(40,45)]:
            self.assertEqual(p.menu_slot(source),target)
        for invalid in [-1,41,255]:
            with self.assertRaises(ValueError):p.menu_slot(invalid)

    def test_player_inventory_40_and_menu_slot_45_both_update_only_offhand(self):
        o=self.observer()
        self.packet(o,'set_player_inventory',varint(40)+self.item(12,5))
        self.assertEqual(o.offhand,{'item':12,'count':5});self.assertIsNone(o.mainhand)
        self.assertEqual(o.rows[-1]['menu_slot'],45)
        self.packet(o,'set_slot',b'\0\0'+struct.pack('>h',45)+self.item(13,3))
        self.assertEqual(o.offhand,{'item':13,'count':3})
        self.packet(o,'set_player_inventory',varint(36)+self.item(14,1))
        self.assertIsNone(o.mainhand,'armor36 must not alias hotbar menu36')

    def test_full_inventory_retains_only_hands_cursor_and_positive_authority(self):
        o=self.observer();values=[b'\0']*46;values[36]=self.item(11,7);values[45]=self.item(12,5)
        self.packet(o,'window_items',b'\0\2'+varint(46)+b''.join(values)+b'\0')
        self.assertTrue(p.hand_ready(o,{'item':11,'count':7},{'item':12,'count':5},0))
        self.assertFalse(p.hand_ready(o,{'item':11,'count':7},{'item':12,'count':5},1))
        self.assertNotIn('inventory',o.rows[-1]);self.assertNotIn('payload_hex',o.rows[-1])

    def test_prediction_or_ack_without_real_inventory_and_drop_is_not_success(self):
        rows=[self.action(),{'direction':'toServer','name':'window_click','mainhand':{'item':11,'count':6}}]
        self.assertFalse(p.effect_evidence(rows,0,{'item':11,'count':6},'unchanged',{'item':11,'count':1}))
        rows.append(self.update(mainhand={'item':11,'count':6}))
        self.assertFalse(p.effect_evidence(rows,0,{'item':11,'count':6},'unchanged',{'item':11,'count':1}))
        rows.extend([self.drop(11,1),self.drop(11,1)])
        self.assertTrue(p.effect_evidence(rows,0,{'item':11,'count':6},'unchanged',{'item':11,'count':1}))

    def test_wrong_drop_and_wrong_count_do_not_pass(self):
        rows=[self.action(),self.update(mainhand=None),self.drop(11,10)]
        self.assertFalse(p.effect_evidence(rows,0,None,'unchanged',{'item':11,'count':11}))
        rows[-1]=self.drop(12,11)
        with self.assertRaises(ValueError):p.effect_evidence(rows,0,None,'unchanged',{'item':11,'count':11})

    def test_swap_requires_both_authoritative_updates_and_no_drop(self):
        main,off={'item':12,'count':5},{'item':13,'count':3}
        rows=[self.action(),self.update(mainhand=main)]
        self.assertFalse(p.effect_evidence(rows,0,main,off))
        rows.append(self.update(offhand=off));self.assertTrue(p.effect_evidence(rows,0,main,off))
        rows.append(self.drop(12,5))
        with self.assertRaises(ValueError):p.effect_evidence(rows,0,main,off)

    def test_cancel_requires_post_release_distinct_action_and_retained_target(self):
        rows=[self.action(0,1.),self.action(77,2.)]
        self.assertFalse(p.cancellation_trace(rows,0,2.5))
        rows.append(self.action(77,3.));self.assertTrue(p.cancellation_trace(rows,0,2.5))
        rows.append({'direction':'toClient','name':'block_change','updates':[{'position':[2,65,0],'state':0}]})
        with self.assertRaises(ValueError):p.cancellation_trace(rows,0,2.5)

    def test_wrong_cancel_target_or_ambiguous_status_fails(self):
        rows=[self.action(0,1.),self.action(77,3.),self.action(78,3.1)]
        with self.assertRaises(ValueError):p.cancellation_trace(rows,0,2.)
        rows=rows[:2];rows[1]['target']=[3,65,0]
        with self.assertRaises(ValueError):p.cancellation_trace(rows,0,2.)

    def test_checkpoint_binds_exact_image_case_hands_and_cancel_crosshair(self):
        expected={'mainhand':None,'offhand':None}
        value={'case':'cancel','screenshot_sha256':'abc','expected_hands':expected,
               'ground_hud_verified':True,'inventory_counts_verified':True,
               'crosshair_target_verified':True,'target':[2,65,0]}
        p.verify_calibration(json.dumps(value),'abc','cancel',expected)
        for key,wrong in [('case','swap'),('screenshot_sha256','old'),('inventory_counts_verified',False),
                          ('expected_hands',{'mainhand':{'item':11,'count':1},'offhand':None}),('target',[3,65,0])]:
            altered=dict(value);altered[key]=wrong
            with self.assertRaises(ValueError):p.verify_calibration(json.dumps(altered),'abc','cancel',expected)

    def test_console_predicates_cover_exact_count_id_and_empty_both_hands(self):
        values=p.hand_predicates('swap','AFTER','gold_ingot','iron_ingot',5,3)
        self.assertEqual(len(values),2)
        self.assertIn('SelectedItem{count:5,id:"minecraft:gold_ingot"}',values[0][1])
        self.assertIn('equipment.offhand{count:3,id:"minecraft:iron_ingot"}',values[1][1])
        empty=p.hand_predicates('cancel','AFTER',None,None)
        self.assertIn('unless data entity McsaProbe SelectedItem run say',empty[0][1])
        self.assertIn('unless data entity McsaProbe equipment.offhand run say',empty[1][1])

    def test_predicted_drop_without_owner_echo_requires_positive_server_count(self):
        main={'item':11,'count':6};drop={'item':11,'count':1}
        rows=[self.action(),self.drop(11,1)]
        self.assertFalse(p.effect_evidence(rows,0,main,'unchanged',drop))
        self.assertFalse(p.effect_evidence(rows,0,main,'unchanged',drop,{}))
        self.assertFalse(p.effect_evidence(rows,0,main,'unchanged',drop,{'mainhand':{'item':11,'count':7}}))
        self.assertTrue(p.effect_evidence(rows,0,main,'unchanged',drop,{'mainhand':main,'offhand':None}))
        self.assertFalse(p.effect_evidence(rows[:1],0,main,'unchanged',drop,{'mainhand':main,'offhand':None}))

    def test_predicted_swap_requires_exact_positive_server_evidence_both_hands(self):
        main,off={'item':12,'count':5},{'item':13,'count':3};rows=[self.action()]
        for proof in [None,{}, {'mainhand':main}, {'mainhand':main,'offhand':{'item':13,'count':2}}]:
            self.assertFalse(p.effect_evidence(rows,0,main,off,server_hands=proof))
        self.assertTrue(p.effect_evidence(rows,0,main,off,server_hands={'mainhand':main,'offhand':off}))
        self.assertFalse(p.effect_evidence(rows,0,None,None,server_hands={}))

    def test_modifier_release_occurs_after_observed_action_and_even_on_failure(self):
        calls=[];client=object()
        def gui(client,*args):calls.append(args)
        def observed():calls.append(('observed-action',))
        with patch.object(p.base,'gui',side_effect=gui):
            p.base.run_mouse_dig(lambda:p.press_action(client,'drop-stack'),
                lambda:p.release_action(client,'drop-stack'),observed,lambda:None,lambda:None,[])
        self.assertEqual(calls,[('keydown','Control_L'),('key','q'),('observed-action',),('keyup','Control_L')])
        calls=[]
        def failed():raise TimeoutError('no actual key action')
        with patch.object(p.base,'gui',side_effect=gui),self.assertRaises(TimeoutError):
            p.base.run_mouse_dig(lambda:p.press_action(client,'drop-stack'),
                lambda:p.release_action(client,'drop-stack'),failed,lambda:None,lambda:None,[])
        self.assertEqual(calls[-1],('keyup','Control_L'))

    def test_default_scenario_plan_has_no_process_or_socket_actions(self):
        with patch('sys.argv',['probe_actual_777_actions.py']),patch('sys.stdout',new_callable=io.StringIO) as output, \
             patch.object(p.base,'launch') as launch,patch.object(p.base.subprocess,'Popen') as process, \
             patch.object(p.base.socket,'socket') as network:
            p.base.main(p.Scenario())
            self.assertIn('cancel/drop1/dropstack/swaphands',json.loads(output.getvalue())['scope'])
            self.assertIn('selected case=swap',p.Scenario('swap').scope)
            launch.assert_not_called();process.assert_not_called();network.assert_not_called()


if __name__=='__main__':unittest.main()
