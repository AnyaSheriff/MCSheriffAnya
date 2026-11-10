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
class RealSocketpairDiagnosticsTests(unittest.TestCase):
 def _finish_peer(self,client,thread,errors,failure=None):
  # Страховочная очистка независимо от исхода основного пути: безопасно гасим и закрываем клиент,
  # затем ограниченный join; ошибки peer собираются и валидируются, а не молча принимаются за EOF.
  import socket
  if thread is None:return
  try:client.shutdown(socket.SHUT_RDWR)
  except OSError:pass  # гашение разблокирует приём в peer, если основной путь упал раньше
  try:client.close()
  except OSError:pass
  thread.join(3)
  if failure is None:
   self.assertFalse(thread.is_alive(),'peer thread alive after bounded join')
   self.assertEqual(errors,[])  # неожиданных ошибок peer нет
  elif thread.is_alive():raise AssertionError('peer thread alive after bounded join on failure path')
 def test_truncated_header_partial_bytes_evidence(self):
  import socket
  cut=frame(9,3,b'pw')[:3]  # обрыв ровно внутри 4-байтового заголовка длины
  client,server=socket.socketpair()
  try:
   server.sendall(cut);server.close()
   with self.assertRaises(EOFError) as caught:r.receive(client)
   error=caught.exception
   self.assertIsInstance(error,r.IncompleteReadError)
   self.assertEqual(error.kind,'header');self.assertEqual((error.wanted,error.received),(4,3))
   self.assertEqual(bytes.fromhex(error.partial_hex),cut)
   self.assertIsNone(error.raw_header_hex);self.assertIsNone(error.declared_size)
   self.assertFalse(hasattr(error,'frame_hex'))  # обрывок не помечается как полный кадр
  finally:client.close();server.close()
 def test_truncated_body_keeps_raw_header_and_declared_size(self):
  import socket
  payload=struct.pack('<ii',104,0)+b'root'+b'\0\0';header=struct.pack('<i',len(payload));cut=payload[:5]
  client,server=socket.socketpair()
  try:
   server.sendall(header+cut);server.close()
   with self.assertRaises(r.IncompleteReadError) as caught:r.receive(client)
   error=caught.exception
   self.assertIsInstance(error,EOFError)  # обратная совместимость со старым контрактом
   self.assertEqual(error.kind,'body');self.assertEqual((error.wanted,error.received),(len(payload),len(cut)))
   self.assertEqual(bytes.fromhex(error.partial_hex),cut)
   self.assertEqual(bytes.fromhex(error.raw_header_hex),header);self.assertEqual(error.declared_size,len(payload))
  finally:client.close();server.close()
 def test_successful_real_socketpair_exchange(self):
  import socket,threading
  client,server=socket.socketpair();client.settimeout(2);server.settimeout(2);errors=[];thread=[None];failure=[None]
  def peer():
   try:
    row=r.receive(server);self.assertEqual(row['id'],104);server.sendall(frame(104,0,b'root'))
    # Здесь нет «намеченного» EOF: обычный успешный обмен без ждёмого обрыва.
   except BaseException as error:errors.append(repr(error))  # любой сбой peer неожиданен
   finally:
    try:server.close()
    except OSError:pass
  try:
   thread[0]=threading.Thread(target=peer);thread[0].start()
   r.send(client,104,2,b'root');answer=r.receive(client)
   self.assertEqual(answer['body_hex'],b'root'.hex())
  except BaseException as error:failure[0]=error;raise
  finally:self._finish_peer(client,thread[0],errors,failure[0])
 def test_run_receipt_keeps_truncated_header_evidence(self):
  import socket,threading,time
  cut=frame(103,3,b'pw')[:2];client,server=socket.socketpair();errors=[];thread=[None];failure=[None]
  try:
   def peer():
    try:
     server.sendall(cut);time.sleep(0.25)  # окно, чтобы клиент успел отправить аутентификацию
     server.shutdown(socket.SHUT_WR)  # явно намеченный тестовый EOF: честный конец чтения вместо RST
     time.sleep(0.25)
    except BaseException as error:errors.append(repr(error))  # неожиданный сбой peer, не «желанный» EOF
    finally:
     try:server.close()
     except OSError:pass
   thread[0]=threading.Thread(target=peer);thread[0].start()
   with patch.object(r.socket,'create_connection',return_value=client):
    measurer=r.IsolatedRcon(1,'fixture')
    with self.assertRaises(EOFError):measurer.run('data get entity blackbox')
   receipt=measurer.calls[0]
   self.assertEqual(receipt['status'],'INVALID')
   evidence=receipt['incomplete_read']
   self.assertEqual(evidence['kind'],'header');self.assertEqual((evidence['wanted'],evidence['received']),(4,2))
   self.assertEqual(bytes.fromhex(evidence['partial_hex']),cut)
   self.assertEqual(receipt['frames'],[])  # фабрикации полного кадра нет
  except BaseException as error:failure[0]=error;raise
  finally:self._finish_peer(client,thread[0],errors,failure[0])
 def test_run_receipt_keeps_truncated_body_evidence_and_request(self):
  import socket,threading,time
  payload=struct.pack('<ii',104,0)+b'root'+b'\0\0';header=struct.pack('<i',len(payload));cut=payload[:5]
  client,server=socket.socketpair();errors=[];thread=[None];failure=[None]
  try:
   def peer():
    try:
     server.sendall(frame(103,2,b'')+header+cut);time.sleep(0.25)  # окно для отправки клиента
     server.shutdown(socket.SHUT_WR)  # явно намеченный тестовый EOF: честный конец чтения вместо RST
     time.sleep(0.25)
    except BaseException as error:errors.append(repr(error))  # неожиданный сбой peer, не «желанный» EOF
    finally:
     try:server.close()
     except OSError:pass
   thread[0]=threading.Thread(target=peer);thread[0].start()
   with patch.object(r.socket,'create_connection',return_value=client):
    measurer=r.IsolatedRcon(1,'fixture')
    with self.assertRaises(EOFError):measurer.run('data get entity blackbox')
   receipt=measurer.calls[0]
   self.assertEqual(receipt['status'],'INVALID')
   evidence=receipt['incomplete_read']
   self.assertEqual(evidence['kind'],'body')
   self.assertEqual(bytes.fromhex(evidence['raw_header_hex']),header);self.assertEqual(evidence['declared_size'],len(payload))
   self.assertEqual(bytes.fromhex(evidence['partial_hex']),cut)
   self.assertEqual([f['id'] for f in receipt['frames']],[103])  # уже полный кадр аутентификации сохранён
   self.assertEqual([q['id'] for q in receipt['requests']],[104])  # квитанция отправленного запроса сохранена
   self.assertIn('chronological',receipt)
   self.assertNotIn('pw',str(receipt))  # без содержимого аутентификации в свидетельстве
  except BaseException as error:failure[0]=error;raise
  finally:self._finish_peer(client,thread[0],errors,failure[0])
 def test_stop_only_receipt_keeps_incomplete_evidence(self):
  import socket,threading,time
  cut=frame(103,3,b'pw')[:1];client,server=socket.socketpair();errors=[];thread=[None];failure=[None]
  try:
   def peer():
    try:
     server.sendall(cut);time.sleep(0.25)  # окно для отправки stop-клиента
     server.shutdown(socket.SHUT_WR)  # явно намеченный тестовый EOF: честный конец чтения вместо RST
     time.sleep(0.25)
    except BaseException as error:errors.append(repr(error))  # неожиданный сбой peer, не «желанный» EOF
    finally:
     try:server.close()
     except OSError:pass
   thread[0]=threading.Thread(target=peer);thread[0].start()
   with patch.object(r.socket,'create_connection',return_value=client):
    measurer=r.IsolatedRcon(1,'fixture')
    with self.assertRaises(EOFError):measurer.stop_only()
   receipt=measurer.calls[0]
   self.assertEqual(receipt['status'],'INVALID')  # регрессия: статус выставляется и на пути stop-ошибки
   self.assertNotIn('stop-dispatched-no-reply-required',receipt)  # успешное stop-поле не подмешивается
   evidence=receipt['incomplete_read']
   self.assertEqual(evidence['kind'],'header');self.assertEqual((evidence['wanted'],evidence['received']),(4,1))
   self.assertNotIn('pw',str(receipt))
  except BaseException as error:failure[0]=error;raise
  finally:self._finish_peer(client,thread[0],errors,failure[0])
 def test_successful_stop_only_keeps_legacy_fields(self):
  import socket,threading
  client,server=socket.socketpair();errors=[];thread=[None];failure=[None]
  def peer():
   try:
    auth=r.receive(server);server.sendall(frame(auth['id'],2,b''))
    stop=r.receive(server);self.assertEqual(stop['type'],2);self.assertEqual(stop['id'],auth['id']+1)
   except BaseException as error:errors.append(repr(error))  # неожиданный сбой peer
   finally:
    try:server.close()
    except OSError:pass
  try:
   thread[0]=threading.Thread(target=peer);thread[0].start()
   with patch.object(r.socket,'create_connection',return_value=client):
    measurer=r.IsolatedRcon(1,'fixture');self.assertEqual(measurer.stop_only(),'')
   receipt=measurer.calls[0]
   self.assertEqual(receipt['status'],'stop-dispatched-no-reply-required')  # легаси-поле успеха не менялось
   self.assertNotIn('incomplete_read',receipt)  # на успешном пути свидетельства обрыва нет
  except BaseException as error:failure[0]=error;raise
  finally:self._finish_peer(client,thread[0],errors,failure[0])
 def test_failure_path_cleanup_joins_blocked_peer(self):
  # Покрытие пути неудачи: основной поток «падает», peer заблокирован на приёме —
  # страховочный bounded join всё равно гарантирует завершение потока и честный EOF.
  import socket,threading
  client,server=socket.socketpair();server.settimeout(2)
  state={'intended_eof':False,'unexpected':[]};thread=[None]
  def peer():
   try:r.receive(server)
   except r.IncompleteReadError:
    state['intended_eof']=True  # ровно ожидаемый EOF от аварийного гашения клиента
   except BaseException as error:state['unexpected'].append(repr(error))  # всё остальное — сбой
   finally:
    try:server.close()
    except OSError:pass
  def failing_body():
   thread[0]=threading.Thread(target=peer);thread[0].start()
   try:raise RuntimeError('simulated unexpected failure in main path')
   finally:self._finish_peer(client,thread[0],[])
  with self.assertRaises(RuntimeError):failing_body()
  self.assertTrue(state['intended_eof'])  # EOF в peer был явно намеченной очисткой теста
  self.assertEqual(state['unexpected'],[])
  self.assertFalse(thread[0].is_alive())
 def test_existing_eoferror_contract_via_mock(self):
  with self.assertRaises(EOFError) as caught:r.receive(Sock(b''))
  self.assertIsInstance(caught.exception,r.IncompleteReadError)
  self.assertEqual(caught.exception.wanted,4);self.assertEqual(caught.exception.received,0)
if __name__=='__main__':unittest.main()
