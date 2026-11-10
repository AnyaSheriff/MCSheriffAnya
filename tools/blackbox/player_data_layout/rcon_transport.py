"""Изолированный bounded RCON; raw framing сохраняется до strict UTF8."""
import socket,struct,time
MAX_FRAME=4*1024*1024
MAX_TOTAL=16*1024*1024
MAX_FRAMES=4096
def exact(sock,n):
 out=bytearray()
 while len(out)<n:
  part=sock.recv(n-len(out))
  if not part:raise EOFError(f'truncated RCON read wanted={n} received={len(out)} partial_hex={out.hex()}')
  out.extend(part)
 return bytes(out)
def receive(sock,header=None):
 header=exact(sock,4) if header is None else header;size=struct.unpack('<i',header)[0]
 assert 10<=size<=MAX_FRAME,'invalid RCON length'
 body=exact(sock,size);request,kind=struct.unpack('<ii',body[:8])
 assert body[-2:]==b'\0\0','invalid RCON terminators'
 return dict(length=size,id=request,type=kind,body_hex=body[8:-2].hex(),frame_hex=(header+body).hex(),at=time.monotonic(),direction='in')
def send(sock,request,kind,body):
 assert request>0 and b'\0' not in body
 payload=struct.pack('<ii',request,kind)+body+b'\0\0';wire=struct.pack('<i',len(payload))+payload;sock.sendall(wire);return dict(id=request,type=kind,body_hex=body.hex(),frame_hex=wire.hex(),length=len(payload),at=time.monotonic(),direction='out')
def collect(sock,request,sentinel,frames,first=None):
 total=0;parts=[]
 for _ in range(MAX_FRAMES):
  row=first if first is not None else receive(sock);first=None;frames.append(row)
  assert row['type']==0,'unexpected RCON response type'
  assert row['id'] in [request,sentinel],'unexpected/negative/stale RCON request ID'
  if row['id']==sentinel:
   # Ordinary list response with distinct ID closes the preceding query stream.
   assert parts and 0<len(bytes.fromhex(row['body_hex']))<=512,'missing command frame or oversized list barrier'
   bytes.fromhex(row['body_hex']).decode('utf-8','strict')
   return b''.join(parts).decode('utf-8','strict')
  chunk=bytes.fromhex(row['body_hex']);total+=len(chunk);assert total<=MAX_TOTAL
  parts.append(chunk)
 raise AssertionError('RCON response frame budget exceeded')
class IsolatedRcon:
 def __init__(self,port,password):
  self.port=port;self.password=password;self.calls=[];self.number=100
 def run(self,text):
  self.number+=3;auth,command,sentinel=self.number,self.number+1,self.number+2
  receipt=dict(command=text,auth_id=auth,request_id=command,sentinel_id=sentinel,frames=[],requests=[],issued=time.monotonic(),status='INVALID');self.calls.append(receipt)
  try:
   with socket.create_connection(('127.0.0.1',self.port),timeout=5) as sock:
    sock.settimeout(5);send(sock,auth,3,self.password.encode())
    for _ in range(3):
     row=receive(sock);receipt['frames'].append(row);assert row['id']==auth
     if row['type']==2:break
     assert row['type']==0 and row['body_hex']==''
    else:raise AssertionError('RCON authentication incomplete')
    receipt['requests'].append(send(sock,command,2,text.encode('utf8')))
    first=receive(sock)
    if first['id']!=command or first['type']!=0:
     receipt['frames'].append(first);raise AssertionError('first target response ID/type mismatch')
    # Только после actual target frame отправляем второй ordinary command.
    receipt['requests'].append(send(sock,sentinel,2,b'list'))
    answer=collect(sock,command,sentinel,receipt['frames'],first=first)
    timeout=sock.gettimeout();sock.setblocking(False)
    try:queued=sock.recv(4,socket.MSG_PEEK)
    except BlockingIOError:queued=b''
    finally:sock.settimeout(timeout)
    if queued:
     receipt['frames'].append(receive(sock));raise AssertionError('buffered trailing response after delimiter')
    receipt.update(status='complete-by-actual-list-response-id',completed=time.monotonic(),response=answer)
    return answer
  finally:
   receipt['chronological']=sorted([*receipt['requests'],*receipt['frames']],key=lambda row:row['at'])

 def stop_only(self):
  self.number+=3;request=self.number;row=dict(command='stop',role='cleanup-only-no-list',frames=[]);self.calls.append(row)
  with socket.create_connection(('127.0.0.1',self.port),timeout=5) as sock:
   sock.settimeout(5);send(sock,request,3,self.password.encode())
   for _ in range(3):
    frame=receive(sock);row['frames'].append(frame);assert frame['id']==request
    if frame['type']==2:break
    assert frame['type']==0 and frame['body_hex']==''
   else:raise AssertionError('cleanup authentication incomplete')
   row['request']=send(sock,request+1,2,b'stop');sock.shutdown(socket.SHUT_WR)
   row['status']='stop-dispatched-no-reply-required'
   return ''
