"""Offline own synthetic packets; no network or official code."""
import io
import os
from pathlib import Path
import struct
import unittest
from unittest.mock import patch
import probe_actual_777_use_actions as p
from probe_white_concrete_slab import inputs
from smoke import varint

CACHE=Path(os.environ.get('MCSA_777_DATA','~/.local/share/mcsa-java-26.3-probe/candidate-json')).expanduser()

class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.tables=inputs(CACHE)
    def observer(self):
        o=p.UseObserver777(self.tables['protocol.json'],[],72,clock=lambda:10.)
        o.phase=dict.fromkeys(o.phase,'play');o.arrow_entity=999;return o
    def packet(self,o,direction,name,payload):
        ident=next(i for i,n in o.names['play'][direction].items() if n==name)
        o.observe(direction,varint(ident)+payload)
    def action(self,t=3,status=77):return {'direction':'toServer','name':'block_dig','status':status,'elapsed':t}
    def use(self,t=1):return {'direction':'toServer','name':'use_item','hand':0,'elapsed':t}
    def test_damage_enchantment_combinations_and_empty(self):
        self.assertIsNone(p.use_slot(io.BytesIO(b'\0')))
        # Synthetic enchant id91 is not a claim about actual lunge registry id.
        value=p.use_slot(io.BytesIO(varint(1)+varint(12)+b'\2\0\3\1\15\1'+varint(91)+b'\1'))
        self.assertEqual(value,{'item':12,'count':1,'damage':1,'enchantments':[{'id':91,'level':1}]})
    def test_unknown_duplicate_removed_or_truncated_components_fail(self):
        for tail in [b'\1\0\14',b'\2\0\3\0\3\0',b'\0\1',b'\1\0\15\1',b'\1\0\15\11']:
            with self.assertRaises((ValueError,EOFError,AssertionError)):p.use_slot(io.BytesIO(b'\1\14'+tail))
    def test_full_inventory_enchanted_hand_and_worn_update(self):
        o=self.observer();slots=[b'\0']*46
        slots[36]=b'\1\14\1\0\15\1\133\1';slots[45]=b'\3\15\0\0'
        self.packet(o,'toClient','window_items',b'\0\0'+varint(46)+b''.join(slots)+b'\0')
        self.assertEqual(o.mainhand['enchantments'],[{'id':91,'level':1}]);self.assertEqual(o.offhand,{'item':13,'count':3})
        self.packet(o,'toClient','set_player_inventory',b'\0\1\14\1\0\3\1')
        self.assertEqual(o.mainhand,{'item':12,'count':1,'damage':1})
    def test_use_is_decoded_and_rejects_trailing_nonfinite_or_invalid_hand(self):
        o=self.observer();payload=b'\0\4'+struct.pack('>ff',-90,-5)
        self.packet(o,'toServer','use_item',payload)
        self.assertEqual(o.rows[-1]['rotation'],[-90.,-5.]);self.assertEqual(o.rows[-1]['sequence'],4)
        for bad in [payload+b'\0',b'\2'+payload[1:],b'\0\4'+struct.pack('>ff',float('nan'),0)]:
            with self.assertRaises(ValueError):self.packet(o,'toServer','use_item',bad)
    def test_arrow_spawn_selected_by_pinned_id_not_item_entity(self):
        o=self.observer()
        self.packet(o,'toClient','spawn_entity',b'\1'+bytes(16)+varint(999)+struct.pack('>ddd',.5,66,.5))
        self.assertEqual(o.rows[-1]['entity_type'],999);self.assertNotIn('payload_hex',o.rows[-1])
    def test_bow_requires_actual_use_then_distinct_post_release_action(self):
        self.assertIsNone(p.observed_action([self.action()],0,2,'bow'))
        self.assertIsNone(p.observed_action([self.use()],0,2,'bow'))
        rows=[self.use(),self.action()];self.assertEqual(p.observed_action(rows,0,2,'bow')['status'],77)
        for invalid in [[self.use(),self.action(1.5)],[self.use(4),self.action()],[self.use(),self.action(),self.action(4)],[self.use(),self.use(),self.action()]]:
            with self.assertRaises(ValueError):p.observed_action(invalid,0,2,'bow')
    def test_bow_ack_or_slot_without_positive_arrow_spawn_fails(self):
        action=self.action();rows=[self.use(),action,{'name':'acknowledge_player_digging','direction':'toClient','elapsed':4}]
        self.assertFalse(p.corroborates(rows,0,action,'bow',999))
        spawn={'name':'spawn_entity','direction':'toClient','entity_type':998,'elapsed':4};rows.append(spawn)
        self.assertFalse(p.corroborates(rows,0,action,'bow',999))
        spawn['entity_type']=999;self.assertTrue(p.corroborates(rows,0,action,'bow',999))
        spawn['elapsed']=2;self.assertFalse(p.corroborates(rows,0,action,'bow',999))
    def test_spear_requires_positive_post_action_movement(self):
        action=self.action();motion={'direction':'toServer','name':'position','elapsed':4,'motion':{'pose':[.5,65,.5,-90,0]}}
        self.assertFalse(p.corroborates([action,motion],0,action,'spear',999))
        motion['motion']['pose'][0]=1.;self.assertTrue(p.corroborates([action,motion],0,action,'spear',999))
        motion['elapsed']=2;self.assertFalse(p.corroborates([motion,action],0,action,'spear',999))
    def test_all_input_releases_attempted_even_after_first_failure(self):
        calls=[];errors=[]
        def gui(client,*args):
            calls.append(args)
            if args==('mouseup','1'):raise RuntimeError('original')
        with patch.object(p.base,'gui',gui),self.assertRaisesRegex(RuntimeError,'original'):p.release_all(None,errors)
        self.assertEqual(calls,[('mouseup','1'),('mouseup','3'),('keyup','Control_L')]);self.assertEqual(len(errors),1)
    def test_exact_before_and_after_predicates_not_predicted_slot_echo(self):
        before='\n'.join(text for _,text in p.predicates('bow','BEFORE'))
        after='\n'.join(text for _,text in p.predicates('bow','AFTER'))
        self.assertIn('minecraft:damage=0',before);self.assertIn('count:3,id:"minecraft:arrow"',before)
        self.assertIn('"minecraft:damage":1',after);self.assertIn('count:2,id:"minecraft:arrow"',after)
        self.assertIn('foodLevel:20','\n'.join(text for _,text in p.predicates('spear','BEFORE')))

if __name__=='__main__':unittest.main()
