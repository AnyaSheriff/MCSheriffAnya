import struct,unittest
from unittest.mock import patch
import rcon_transport as r
class Sock:
 def __init__(self,data):self.data=data
 def recv(self,n):out=self.data[:min(n,3)];self.data=self.data[len(out):];return out
def frame(i,t,b):
 body=struct.pack('<ii',i,t)+b+b'\0\0';return struct.pack('<i',len(body))+body
class TransportTests(unittest.TestCase):
 def test_multipart_split_utf8_concat_then_decode_and_raw(self):
  data='Аня'.encode();rows=[]
  self.assertEqual(r.collect(Sock(frame(1,0,data[:1])+frame(1,0,data[1:])+frame(2,0,b'2 players online')),1,2,rows),'Аня')
  self.assertEqual(len(rows),3);self.assertTrue(all('frame_hex' in x for x in rows))
 def test_incomplete_stale_negative_type_and_sentinel_rejected(self):
  for data in [frame(-1,0,b'x'),frame(3,0,b'x'),frame(1,2,b'x'),frame(2,0,b'x'),frame(1,0,b'x'),frame(1,0,b'\xff')+frame(2,0,b'2 players online'),struct.pack('<i',-1),frame(1,0,b'x')[:-1]]:
   with self.subTest(data=data):
    with self.assertRaises((AssertionError,EOFError,UnicodeDecodeError)):r.collect(Sock(data),1,2,[])
 def test_complete_isolated_exchange_and_duplicate_or_outoforder_rejection(self):
  class Connection(Sock):
   def __enter__(self):return self
   def __exit__(self,*a):pass
   def settimeout(self,*a):pass
   def gettimeout(self):return 5
   def setblocking(self,*a):pass
   def sendall(self,*a):pass
   def shutdown(self,*a):raise AssertionError('no SHUT_WR allowed in measurement')
   def recv(self,n,flags=0):
    if flags:return self.data[:n]
    return super().recv(n)
  valid=frame(103,2,b'')+frame(104,0,b'root')+frame(105,0,b'2 players')
  with patch.object(r.socket,'create_connection',return_value=Connection(valid)):
   client=r.IsolatedRcon(1,'fixture');self.assertEqual(client.run('data get entity blackbox'),'root')
   self.assertEqual([q['id'] for q in client.calls[0]['requests']],[104,105])
  for payload in [frame(105,0,b'2 players')+frame(104,0,b'late'),frame(104,0,b'root')+frame(105,0,b'2 players')+frame(105,0,b'duplicate'),frame(103,0,b'duplicateauth'),frame(104,0,b'root')+frame(106,0,b'wrong')]:
   with patch.object(r.socket,'create_connection',return_value=Connection(frame(103,2,b'')+payload)):
    with self.assertRaises((AssertionError,EOFError)):r.IsolatedRcon(1,'fixture').run('data get entity blackbox')
 def test_real_socketpair_first_target_before_delimiter_no_halfclose_or_eof_wait(self):
  import socket,threading,select
  client,server=socket.socketpair();server.settimeout(2);errors=[]
  def peer():
   try:
    auth=r.receive(server);server.sendall(frame(auth['id'],2,b''))
    command=r.receive(server);self.assertEqual(command['type'],2)
    self.assertFalse(select.select([server],[],[],.03)[0],'delimiter sent before first response')
    server.sendall(frame(command['id'],0,b'root'))
    delimiter=r.receive(server);self.assertEqual(bytes.fromhex(delimiter['body_hex']),b'list')
    self.assertEqual(delimiter['id'],command['id']+1)
    server.sendall(frame(delimiter['id'],0,b'2 players'))
    self.assertEqual(server.recv(1),b'') # Client closes after delimiter, no server EOF needed.
   except BaseException as error:errors.append(repr(error))
   finally:server.close()
  thread=threading.Thread(target=peer);thread.start()
  with patch.object(r.socket,'create_connection',return_value=client):
   self.assertEqual(r.IsolatedRcon(1,'fixture').run('data get entity blackbox'),'root')
  thread.join(3);self.assertFalse(thread.is_alive());self.assertEqual(errors,[])
 def test_same_id_chunks_have_only_TCP_order_not_invented_ordinals(self):
  rows=[];answer=r.collect(Sock(frame(1,0,b'x')+frame(1,0,b'x')+frame(2,0,b'list')),1,2,rows)
  self.assertEqual(answer,'xx');self.assertEqual(len(rows),3)
 def test_bad_lengths_rejected_before_body(self):
  for n in [0,9,r.MAX_FRAME+1]:
   with self.assertRaises(AssertionError):r.receive(Sock(struct.pack('<i',n)))
if __name__=='__main__':unittest.main()
