import io
import os
import json
from pathlib import Path
import struct
import tempfile
import threading
import unittest
from unittest.mock import Mock, patch
import uuid
import probe_actual_777_dig as p
from probe_white_concrete_slab import inputs, pack_position
from smoke import varint

CACHE=Path(os.environ.get('MCSA_777_DATA', '~/.local/share/mcsa-java-26.3-probe/candidate-json')).expanduser()


class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.tables=inputs(CACHE)
    def observer(self):
        return p.Observer777(self.tables['protocol.json'],[],72,clock=lambda:10.)
    def packet(self,o,direction,name,payload=b'',phase='play'):
        ident=next(k for k,v in o.names[phase][direction].items() if v==name)
        o.observe(direction,varint(ident)+payload)
    def play(self):
        o=self.observer();o.phase=dict.fromkeys(o.phase,'play');return o

    def test_plan_only_has_no_process_or_network_actions(self):
        with patch('sys.argv',['probe_actual_777_dig.py']), patch('sys.stdout',new_callable=io.StringIO) as out, \
             patch.object(p,'launch') as launch, patch.object(p.subprocess,'Popen') as process, \
             patch.object(p.socket,'socket') as network:
            p.main()
            self.assertFalse(json.loads(out.getvalue())['execute'])
            launch.assert_not_called();process.assert_not_called();network.assert_not_called()

    def test_wrong_phase_or_compression_fails_without_dumping_body(self):
        o=self.observer()
        with self.assertRaises(ValueError):o.observe('toClient',b'\3private-payload')
        self.assertEqual(o.rows,[])

    def test_real777_configuration_ack_transitions(self):
        o=self.observer()
        identity=uuid.UUID(bytes=__import__('hashlib').md5(b'OfflinePlayer:McsaProbe').digest(),version=3).bytes
        o.observe('toClient',b'\2'+identity+b'\11McsaProbe\0'+bytes([1])*16)
        o.observe('toServer',b'\3')
        packs=b'\1\11minecraft\4core\4'+b'26.3'
        self.packet(o,'toClient','select_known_packs',packs,'configuration')
        self.packet(o,'toServer','select_known_packs',packs,'configuration')
        self.packet(o,'toClient','finish_configuration',phase='configuration')
        self.assertEqual(o.phase['toClient'],'configuration')
        self.packet(o,'toServer','finish_configuration',phase='configuration')
        self.assertEqual(o.phase,{'toClient':'play','toServer':'play'})
        self.assertFalse(any('payload_hex' in row for row in o.rows))

    def test_no_registry_or_chunk_payload_is_retained(self):
        o=self.observer();o.phase=dict.fromkeys(o.phase,'configuration')
        self.packet(o,'toClient','registry_data',b'privatebody'*100,'configuration')
        self.assertEqual(o.rows,[])
        o.phase=dict.fromkeys(o.phase,'play')
        self.packet(o,'toClient','map_chunk',b'chunkbody'*100)
        self.assertEqual(o.rows,[])
        self.assertEqual(o.counts['toClient/play/map_chunk'],1)

    def test_777_extended_ack_short776_is_rejected(self):
        o=self.play()
        self.packet(o,'toClient','position',b'\1'+struct.pack('>ddddddffI',.5,65.,.5,0.,0.,0.,-90.,30.,0))
        with self.assertRaises((ValueError,EOFError)):
            self.packet(o,'toServer','teleport_confirm',b'\1')
        self.packet(o,'toServer','teleport_confirm',b'\1'+struct.pack('>dddff',.5,65.,.5,-90.,30.))
        self.assertEqual(o.acks,{1})
        self.assertEqual(o.pose,[None]*5,'ACK is not movement evidence')

    def test_ground_flag_two_is_horizontal_collision_only(self):
        o=self.play()
        self.packet(o,'toServer','position_look',struct.pack('>dddffB',.5,65.,.5,-90.,30.,2))
        self.assertFalse(o.rows[-1]['motion']['ground'])
        rows=[{'elapsed':t,'motion':{'ground':True,'pose':[.5,65.,.5,0.,0.]}} for t in [1.,2.,3.]]
        self.assertTrue(p.ground_ready(rows,0,3.1))
        self.assertFalse(p.ground_ready(rows,0,5.))

    def test_wrong_target_never_passes_even_with_air(self):
        o=self.play()
        self.packet(o,'toServer','block_dig',b'\0'+pack_position((3,65,0))+b'\1\1')
        self.packet(o,'toClient','block_change',pack_position(p.TARGET)+b'\0')
        with self.assertRaises(ValueError):p.case_evidence(o.rows,0,False)

    def test_actual_start_and_air_do_not_require_finish(self):
        o=self.play()
        self.packet(o,'toServer','block_dig',b'\0'+pack_position(p.TARGET)+b'\1\1')
        self.packet(o,'toClient','block_change',pack_position(p.TARGET)+b'\0')
        self.assertTrue(p.case_evidence(o.rows,0,False))
        self.assertFalse(p.case_evidence(o.rows,0,True))
        o.rows.append({'direction':'toClient','name':'entity_metadata','entity':10,'slot':{'item':721,'count':1}})
        self.assertTrue(p.case_evidence(o.rows,0,True))
        o.rows[-1]['slot']['count']=2
        self.assertFalse(p.case_evidence(o.rows,0,True))

    def test_ack_only_and_non_target_air_are_not_break_evidence(self):
        o=self.play()
        self.packet(o,'toServer','block_dig',b'\0'+pack_position(p.TARGET)+b'\1\5')
        self.packet(o,'toClient','acknowledge_player_digging',b'\5')
        self.packet(o,'toClient','block_change',pack_position((3,65,0))+b'\0')
        self.assertFalse(p.case_evidence(o.rows,0,False))
        self.assertEqual(o.rows[0]['target'],list(p.TARGET))
        self.assertEqual(o.rows[0]['payload_hex'],'0000000080000000410105')

    def test_inventory_trace_exposes_only_mainhand_and_cursor(self):
        o=self.play()
        payload=b'\0\0'+varint(46)+b'\0'*36+b'\1'+varint(1047)+b'\0\0'+b'\0'*9+b'\0'
        self.packet(o,'toClient','window_items',payload)
        self.assertEqual(o.mainhand,{'item':1047,'count':1})
        self.assertNotIn('inventory',o.rows[-1])
        self.assertNotIn('payload_hex',o.rows[-1])

    def test_checkpoint_requires_exact_current_image_target_and_case(self):
        value={'screenshot_sha256':'abc','case':'stone','crosshair_target_verified':True,
               'floor_tool_hud_verified':True,'target':list(p.TARGET)}
        self.assertEqual(p.bounded_calibration(json.dumps(value),'abc','stone'),value)
        for key,replacement in [('screenshot_sha256','old'),('case','slab'),('target',[3,65,0]),
                                ('crosshair_target_verified',False)]:
            altered=dict(value);altered[key]=replacement
            with self.assertRaises(ValueError):p.bounded_calibration(json.dumps(altered),'abc','stone')

    def test_mouse_releases_on_air_before_waiting_for_drop(self):
        calls=[]
        actions=[lambda name=name:calls.append(name) for name in ['press','air','drop','diagnose','release']]
        p.run_mouse_dig(actions[0],actions[4],actions[1],actions[2],actions[3],[])
        self.assertEqual(calls,['press','air','release','drop','diagnose'])

    def test_original_air_failure_survives_release_and_diagnostic_errors(self):
        failure=TimeoutError('original air timeout');calls=[];errors=[]
        def air():calls.append('air');raise failure
        def release():calls.append('release');raise RuntimeError('release failed')
        def diagnose():calls.append('diagnose');raise RuntimeError('diagnostic failed')
        with self.assertRaises(TimeoutError) as raised:
            p.run_mouse_dig(lambda:calls.append('press'),release,air,
                            lambda:calls.append('drop'),diagnose,errors)
        self.assertIs(raised.exception,failure)
        self.assertEqual(calls,['press','air','release','diagnose'])
        self.assertEqual([e['step'] for e in errors],['release-mouse','after-state'])

    def test_drop_failure_still_collects_after_state_after_mouse_release(self):
        calls=[];failure=AssertionError('missing slab count1')
        def drop():calls.append('drop');raise failure
        with self.assertRaises(AssertionError) as raised:
            p.run_mouse_dig(lambda:calls.append('press'),lambda:calls.append('release'),
                            lambda:calls.append('air'),drop,lambda:calls.append('diagnose'),[])
        self.assertIs(raised.exception,failure)
        self.assertEqual(calls,['press','air','release','drop','diagnose'])

    def test_after_state_queries_are_only_three_exact_own_predicates(self):
        commands=p.after_state_commands('stone')
        self.assertEqual([name for name,_ in commands],['air','stone','slab'])
        self.assertEqual(commands[2][1],
            'execute if block 2 65 0 minecraft:white_concrete_slab[type=bottom,waterlogged=false] run say MCSA_GUI_stone_AFTER_slab')
        with self.assertRaises(ValueError):p.after_state_commands('arbitrary')

    def test_cleanup_continues_after_gui_and_socket_errors(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);game=root/'game';game.mkdir()
            report={'result':'PASS-packet-evidence-visual-review-pending','screenshots':[]}
            client=Mock(pid=111,returncode=-15);server=Mock(pid=222,returncode=0)
            peer=Mock();peer.close.side_effect=OSError('close failed')
            worker=Mock();worker.is_alive.return_value=False
            ending=threading.Event()
            with patch.object(p,'gui',side_effect=RuntimeError('display unavailable')), \
                 patch.object(p,'stop_client_bounded') as stopclient, \
                 patch.object(p,'stop_server',return_value='stop') as stopserver, \
                 patch.object(p,'group_pids',return_value=[]), \
                 patch.object(p,'free_port',return_value=True), \
                 patch.object(p.signal,'alarm'):
                p.finish_run(report,root,game,client,server,[peer],None,[worker],ending,{}, {})
            self.assertTrue(ending.is_set());stopclient.assert_called_once();stopserver.assert_called_once()
            worker.join.assert_called_once_with(timeout=5)
            self.assertEqual(json.loads((root/'result.json').read_text())['result'],'FAIL')
            self.assertEqual(len(report['cleanup_errors']),2)


if __name__=='__main__':unittest.main()
