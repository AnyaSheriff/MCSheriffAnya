"""Synthetic first-response guards only; never launch a server or Cargo."""
import json,tempfile,unittest
from pathlib import Path
import simulation_distance_ci as probe
PROTOCOL={'login':{'toClient':{'types':{'packet':['container',[{'type':['mapper',{'mappings':{'0x00':'disconnect','0x02':'success','0x03':'compress'}}]}]]}}}}
class FirstResponse(unittest.TestCase):
 def test_disconnect_is_persisted_and_never_skipped(self):
  reason='{"text":"not whitelisted"}';packet=b'\0'+probe.varint(len(reason))+reason.encode()
  with tempfile.TemporaryDirectory() as folder:
   target=Path(folder)/'first.json'
   with self.assertRaisesRegex(RuntimeError,'LoginDisconnect'):probe.record_first_login(packet,PROTOCOL,target)
   row=json.loads(target.read_text());self.assertEqual(row['kind'],'LoginDisconnect');self.assertEqual(bytes.fromhex(row['packet_hex']),packet);self.assertEqual(target.with_suffix('.packet').read_bytes(),packet)
 def test_success_bytes_saved_before_later_full_success_decode(self):
  packet=b'\x02synthetic-payload-only'
  with tempfile.TemporaryDirectory() as folder:
   target=Path(folder)/'first.json';self.assertEqual(probe.record_first_login(packet,PROTOCOL,target),packet[1:]);self.assertEqual(json.loads(target.read_text())['kind'],'LoginSuccess')
 def test_unexpected_response_persisted_and_rejected(self):
  packet=b'\x03\x80\x02'
  with tempfile.TemporaryDirectory() as folder:
   target=Path(folder)/'first.json'
   with self.assertRaises(AssertionError):probe.record_first_login(packet,PROTOCOL,target)
   self.assertEqual(json.loads(target.read_text())['kind'],'UnexpectedLoginResponse')
 def test_malformed_packet_header_still_preserves_actual_bytes(self):
  packet=b'\x80'
  with tempfile.TemporaryDirectory() as folder:
   target=Path(folder)/'first.json'
   with self.assertRaises(EOFError):probe.record_first_login(packet,PROTOCOL,target)
   self.assertEqual(target.with_suffix('.packet').read_bytes(),packet)
if __name__=='__main__':unittest.main()
