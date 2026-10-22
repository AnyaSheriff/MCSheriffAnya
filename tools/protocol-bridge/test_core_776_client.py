"""Offline adversarial tests: no Minecraft/Java/Xvfb/Cargo or remote IO."""
import io
import json
import socket
import struct
import threading
import time
import unittest
import tempfile
from pathlib import Path
import hashlib
import live_client
import probe_gameplay
import probe_core_776_client as runner
import signal
import subprocess
from contextlib import ExitStack
from unittest.mock import Mock
import uuid
from unittest.mock import patch

from observe import frame
from probe_core_776_client import (Observer, FrameDeadline, calibration, ground_ready,
                                   inventory_is, digest)
from probe_registry_client import string
from smoke import varint,send


def protocol():
    # Small test transport dictionary; not a Minecraft implementation.
    def table(mapping):
        return {'types':{'packet':['container',[{'type':['mapper',{'mappings':mapping}]}]]}}
    return {phase:{direction:table({'0x00':'teleport_confirm','0x03':'finish_configuration','0x07':'registry_data',
             '0x0e':'select_known_packs','0x12':'window_items','0x14':'set_slot',
             '0x1e':'position','0x60':'set_cursor_item','0x6c':'set_player_inventory'})
             for direction in ('toClient','toServer')} for phase in ('play','configuration')}


class RelayTests(unittest.TestCase):
    def setUp(self):
        self.rows=[];self.o=Observer(protocol(),self.rows)

    def login(self):
        identity=uuid.UUID(bytes=__import__('hashlib').md5(b'OfflinePlayer:McsaProbe').digest(),version=3)
        self.o.observe('toClient',b'\2'+identity.bytes+string('McsaProbe')+b'\0'+uuid.uuid4().bytes)
        self.o.observe('toServer',b'\3')

    def test_real_ack_transitions_before_forwarding(self):
        self.login()
        self.o.observe('toClient',b'\x0e\1'+string('minecraft')+string('core')+string('26.2'))
        self.o.observe('toServer',b'\7\0')
        self.o.observe('toClient',b'\3')
        self.assertEqual(set(self.o.phase.values()),{'configuration'})
        self.o.observe('toServer',b'\3')
        self.assertEqual(set(self.o.phase.values()),{'play'})
        self.assertEqual(self.o.reply,[])

    def test_invalid_or_premature_acks_rejected(self):
        with self.assertRaises(ValueError): self.o.observe('toServer',b'\3')
        for bad in (b'\3\0',b'\1',b'\3'):
            o=Observer(protocol(),[])
            o.phase=dict.fromkeys(o.phase,'configuration')
            with self.assertRaises(ValueError):o.observe('toServer',bad)

    def test_compression_encryption_not_accepted(self):
        for body in (b'\1',b'\3',b'\2\0'):
            with self.assertRaises((ValueError,EOFError)):
                Observer(protocol(),[]).observe('toClient',body)

    def test_no_registry_body_retention(self):
        self.login()
        raw=string('minecraft:probe')+varint(2)+b'secret-test-data'*50
        self.o.observe('toClient',b'\7'+raw)
        row=self.rows[-1]
        self.assertEqual(row['registry'],'minecraft:probe');self.assertEqual(row['count'],2)
        self.assertNotIn('secret-test-data',json.dumps(self.rows))
        self.assertEqual(row['sha256'],digest(raw))

    def test_ack_is_not_pose_and_unknown_ack_fails(self):
        self.o.phase=dict.fromkeys(self.o.phase,'play');self.o.teleports.add(8)
        self.o.observe('toServer',b'\0\10')
        self.assertEqual(self.o.pose,(None,)*5)
        with self.assertRaises(ValueError): self.o.observe('toServer',b'\0\11')

    def test_movement_requires_actual_position(self):
        self.o.phase=dict.fromkeys(self.o.phase,'play')
        self.o.observe('toServer',b'\x1e'+struct.pack('>dddB',.5,64,.5,1))
        self.assertEqual(self.rows[-1]['motion']['pose'][:3],[.5,64.,.5])
        with self.assertRaises(ValueError):self.o.observe('toServer',b'\x1e'+struct.pack('>dddB',.5,float('nan'),.5,1))

    def test_authoritative_stone_move_not_client_prediction(self):
        self.o.phase=dict.fromkeys(self.o.phase,'play')
        stone=4
        slots=[b'\0']*46;slots[36]=b'\5'+varint(stone)+b'\0\0'
        self.o.observe('toClient',b'\x12\0\1'+varint(46)+b''.join(slots)+b'\0')
        self.assertTrue(inventory_is(self.o,stone,False))
        self.assertFalse(inventory_is(self.o,stone,True))
        self.o.observe('toClient',b'\x14\0\2'+struct.pack('>h',36)+b'\0')
        self.o.observe('toClient',b'\x14\0\3'+struct.pack('>h',37)+b'\5'+varint(stone)+b'\0\0')
        self.o.observe('toClient',b'\x60\0')
        self.assertTrue(inventory_is(self.o,stone,True))
        self.assertGreater(self.o.authority['cursor'],self.o.authority[37])
        self.assertEqual(self.o.inventory.inventory['revision'],3)

    def test_record_cap_stops(self):
        self.o.rows.extend({} for _ in range(8192))
        with self.assertRaises(ValueError):self.o.observe('toServer',b'\3')


class SafetyTests(unittest.TestCase):
    def test_slow_partial_frame_uses_whole_deadline(self):
        class Slow:
            def settimeout(self,value):self.remaining=value
            def recv(self,size):
                time.sleep(.025)
                return b'\x80'
        with self.assertRaises(TimeoutError):frame(FrameDeadline(Slow(),.04))

    def test_relay_reframing_preserves_payload(self):
        # Real loopback-free socketpair + fragmented delivery, no MC process.
        left,right=socket.socketpair()
        body=b'\x12'+bytes(range(256))*40
        encoded=varint(len(body))+body
        def writer():
            for i in range(0,len(encoded),7):left.sendall(encoded[i:i+7])
            left.close()
        thread=threading.Thread(target=writer);thread.start()
        try:self.assertEqual(frame(FrameDeadline(right,2)),body)
        finally:right.close();thread.join(2)
        self.assertFalse(thread.is_alive())

    def test_opaque_launch_options_written_before_process(self):
        # Popen replaced: never starts Java/Xvfb or reads a real client JAR.
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);cache=root/'cache';game=root/'game';cache.mkdir();game.mkdir()
            (cache/'client.jar').write_bytes(b'opaque test bytes')
            sha=hashlib.sha1(b'opaque test bytes').hexdigest()
            (cache/'version.json').write_text(json.dumps({'id':'26.2','downloads':{'client':{'sha1':sha}},'assetIndex':{'id':'test'}}))
            (cache/'classpath.txt').write_text('/opaque/client.jar')
            seen=[]
            def fake(command,**kwargs):
                seen.append((command,kwargs,(game/'options.txt').read_text()))
                return object()
            with patch.dict(live_client.CLIENT_SHA1,{'26.2':sha}), patch.object(live_client.subprocess,'Popen',side_effect=fake):
                live_client.launch(cache,game,26030,io.StringIO(),version='26.2',gui_scale=2,pause_on_lost_focus=False)
            command,kwargs,options=seen[0]
            self.assertIn('guiScale:2\n',options);self.assertIn('pauseOnLostFocus:false\n',options)
            self.assertTrue(kwargs['start_new_session'])
            self.assertNotIn('DISPLAY',kwargs['env']);self.assertNotIn('WAYLAND_DISPLAY',kwargs['env'])
            self.assertEqual(command[-1],'127.0.0.1:26030')

    def test_inventory_bounds_before_allocating(self):
        observer=Observer(protocol(),[]);observer.phase=dict.fromkeys(observer.phase,'play')
        with self.assertRaises(ValueError):observer.observe('toClient',b'\x12\0\1'+varint(2**30))

    def test_connect_uses_optional_uuid_without_changing_default(self):
        class Sock:
            def __init__(self):self.sent=[]
            def sendall(self,body):self.sent.append(body)
            def close(self):pass
        for identity in (None,b'Z'*16):
            sock=Sock()
            with patch.object(probe_gameplay.socket,'create_connection',return_value=sock), patch.object(
                    probe_gameplay,'frame',side_effect=[b'\2',b'\3']):
                probe_gameplay.connect(26028,protocol(),lambda s,p:None,776,login_uuid=identity)
            reader=io.BytesIO(sock.sent[1]);integer=probe_gameplay.read_varint
            size=integer(reader.read);payload=reader.read();self.assertEqual(size,len(payload))
            self.assertTrue(payload.startswith(b'\0\x09McsaProbe'))
            self.assertEqual(payload[-16:],b'%'*16 if identity is None else identity)

    def test_invalid_uuid_cannot_open_connection(self):
        with patch.object(probe_gameplay.socket,'create_connection') as connect:
            for bad in (b'',b'x'*15,b'x'*17,'x'*16):
                with self.assertRaises(ValueError):probe_gameplay.connect(26028,protocol(),login_uuid=bad)
            connect.assert_not_called()

    def test_calibration_requires_exact_reviewed_image(self):
        good={'screenshot_sha256':'abc','inventory_verified':True,
              'slot36':[285,376],'slot37':[321,376],'recipe_book':[100,100]}
        self.assertEqual(calibration(json.dumps(good),'abc')['slot36'],[285,376])
        for change in ({'screenshot_sha256':'old'},{'slot37':[854,10]},
                       {'inventory_verified':False},{'slot37':[285,376]},{'slot36':[True,1]}):
            with self.assertRaises(ValueError):calibration(json.dumps(dict(good,**change)),'abc')

    def test_ground_stale_fall_and_early_samples_fail(self):
        def row(t,y=64,ground=True):return {'elapsed':t,'motion':{'pose':[.5,y,.5,0,0],'ground':ground}}
        good=[row(0),row(1),row(2.1)]
        self.assertTrue(ground_ready(good,0,2.2))
        self.assertFalse(ground_ready(good,0,4))
        self.assertFalse(ground_ready(good,1,2.2))
        self.assertFalse(ground_ready([row(0),row(1,63),row(2.1)],0,2.2))
        self.assertFalse(ground_ready([row(0),row(.5),row(1)],0,1))


class CleanupTests(unittest.TestCase):
    def run_cleanup(self, *, client_error=None, server_error=None, core_exit=0,
                    copy_error=None, groups=None):
        with tempfile.TemporaryDirectory() as folder, ExitStack() as stack:
            root=Path(folder);core=root/'core';game=root/'game';output=root/'out'
            core.mkdir();game.mkdir();output.mkdir()
            report={'result':'NETWORK_PASS_VISUAL_PENDING','error':'original scenario failure',
                    'screenshots':[]}
            client=Mock(pid=101,returncode=0);server=Mock(pid=102,returncode=core_exit)
            events=[]
            def stop_client(process):
                events.append('client')
                if client_error: raise client_error
            def stop_core(process):
                events.append('core')
                if server_error: raise server_error
                return 'stop-and-stdin-eof'
            stack.enter_context(patch.object(runner,'stop_client_bounded',side_effect=stop_client))
            stack.enter_context(patch.object(runner,'stop_server',side_effect=stop_core))
            stack.enter_context(patch.object(runner,'group_pids',side_effect=groups or (lambda group:[])))
            stack.enter_context(patch.object(runner,'free_port',return_value=True))
            stack.enter_context(patch.object(runner.os,'killpg',side_effect=lambda *args:events.append(('kill',args))))
            stack.enter_context(patch.object(runner.signal,'signal',side_effect=lambda sig,handler:events.append(('signal',sig,handler))))
            stack.enter_context(patch.object(runner.signal,'alarm'))
            if copy_error:stack.enter_context(patch.object(runner.shutil,'copytree',side_effect=copy_error))
            runner.finish_run(report,output,core,game,client,server,None,[],None,[],
                              threading.Event(),{signal.SIGINT:'old-int',signal.SIGTERM:'old-term'})
            saved=json.loads((output/'result.json').read_text())
            return saved,events

    def test_client_stop_failure_cannot_skip_core_and_result(self):
        saved,events=self.run_cleanup(client_error=RuntimeError('client stop failed'),
                                      copy_error=OSError('diagnostic copy failed'))
        self.assertEqual(saved['result'],'FAIL')
        self.assertEqual(saved['error'],'original scenario failure')
        self.assertEqual(saved['server_exit'],0)
        self.assertLess(events.index('client'),events.index('core'))
        for sig in (signal.SIGINT,signal.SIGTERM):
            self.assertLess(events.index(('signal',sig,signal.SIG_IGN)),events.index('client'))
        self.assertEqual({e['step'] for e in saved['cleanup_errors']},
                         {'stop-client','preserve-failed-core'})

    def test_stop_core_exception_still_preserves_report(self):
        saved,_=self.run_cleanup(server_error=RuntimeError('core stop failed'))
        self.assertEqual(saved['result'],'FAIL')
        self.assertEqual(saved['shutdown'],'stop-failed')
        self.assertEqual(saved['error'],'original scenario failure')
        self.assertIn('stop-core',[e['step'] for e in saved['cleanup_errors']])

    def test_forced_core_exit_and_group_cleanup_remain_failure(self):
        calls=[]
        def groups(group):
            calls.append(group)
            return [group] if group==101 and calls.count(group)==1 else []
        saved,events=self.run_cleanup(core_exit=-9,groups=groups)
        self.assertEqual(saved['result'],'FAIL');self.assertEqual(saved['server_exit'],-9)
        self.assertTrue(any(isinstance(e,tuple) and e[0]=='kill' for e in events))
        self.assertIn('forced-group-cleanup',[e['step'] for e in saved['cleanup_errors']])
        self.assertFalse(any(saved['cleanup']['owned_groups'].values()))

    def test_client_wait_after_sigkill_is_bounded(self):
        client=Mock(pid=123)
        client.wait.side_effect=subprocess.TimeoutExpired('own-test-child',1)
        with patch.object(runner.os,'killpg') as kill:
            with self.assertRaises(subprocess.TimeoutExpired):runner.stop_client_bounded(client)
        self.assertEqual([c.kwargs['timeout'] for c in client.wait.call_args_list],[10,5])
        self.assertEqual([c.args for c in kill.call_args_list],[(123,signal.SIGTERM),(123,signal.SIGKILL)])


class GuiPreflightTests(unittest.TestCase):
    def test_missing_tool_fails_before_any_version_process(self):
        with patch.object(runner.shutil,'which',side_effect=lambda name:None if name=='xdotool' else '/usr/bin/'+name), \
             patch.object(runner.subprocess,'run') as process:
            with self.assertRaisesRegex(RuntimeError,'xdotool'):runner.preflight_gui_tools()
            process.assert_not_called()

    def test_present_xdotool_version_is_bounded_and_requires_correct_response(self):
        with patch.object(runner.shutil,'which',side_effect=lambda name:'/private/bin/'+name), \
             patch.object(runner.subprocess,'run',return_value=Mock(stdout='xdotool version 4.20260303.1\n')) as process:
            tools=runner.preflight_gui_tools()
            self.assertEqual(tools['xdotool'],'/private/bin/xdotool')
            self.assertEqual(process.call_args.args[0],['/private/bin/xdotool','--version'])
            self.assertEqual(process.call_args.kwargs['timeout'],5)
            self.assertTrue(process.call_args.kwargs['check'])
            process.return_value.stdout='wrong executable'
            with self.assertRaises(RuntimeError):runner.preflight_gui_tools()

    def test_missing_dynamic_library_fails_preflight(self):
        with patch.object(runner.shutil,'which',side_effect=lambda name:'/usr/bin/'+name), \
             patch.object(runner.subprocess,'run',side_effect=subprocess.CalledProcessError(127,'xdotool')):
            with self.assertRaises(subprocess.CalledProcessError):runner.preflight_gui_tools()

    def test_main_missing_tool_precedes_binary_io_network_and_server(self):
        from argparse import Namespace
        args=Namespace(execute=True,test_binary=Path('/nonexistent/core'),protocol=Path('/nonexistent/protocol'),
                       client_dir=Path('/nonexistent/client'),items=Path('/nonexistent/items'),
                       output=Path('/nonexistent/output'),binary_sha256='0'*64)
        with patch.object(runner.argparse.ArgumentParser,'parse_args',return_value=args), \
             patch.object(runner.shutil,'which',return_value=None), \
             patch.object(runner.subprocess,'Popen') as process,patch.object(runner.socket,'socket') as network:
            with self.assertRaisesRegex(RuntimeError,'xdotool'):runner.main()
            process.assert_not_called();network.assert_not_called()


if __name__=='__main__':unittest.main()
