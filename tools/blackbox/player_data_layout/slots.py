"""Узкий lossless decoder fixture-компонент: неизвестные компоненты отвергаются."""
import struct
from smoke import read_varint
def exact(b,n):
 v=b.read(n);assert len(v)==n;return v
def string(b):return exact(b,int.from_bytes(exact(b,2),'big')).decode('utf8','strict')
def custom(b):
 assert exact(b,1)==b'\x0a';fields={}
 while True:
  kind=exact(b,1)[0]
  if not kind:return fields
  key=string(b);assert key not in fields
  if kind==3:value=struct.unpack('>i',exact(b,4))[0]
  elif kind==8:value=string(b)
  else:raise AssertionError('fixture custom NBT unsupported type')
  fields[key]=dict(type=kind,value=value)
class SlotDecoder:
 def __init__(self,items):self.names={x['id']:x['name'] for x in items}
 def slot(self,b):
  count=read_varint(b.read)
  if not count:return None
  assert 0<count<=99;item=read_varint(b.read);added=read_varint(b.read);removed=read_varint(b.read);assert 0<=added<=8 and removed==0
  out=dict(item=self.names[item],count=count);seen=set()
  for _ in range(added):
   kind=read_varint(b.read);assert kind not in seen;seen.add(kind)
   if kind==0:
    start=b.tell();out['custom_data']=custom(b);end=b.tell();out['custom_data_raw_hex']=b.getvalue()[start:end].hex()
   elif kind==3:out['damage']=read_varint(b.read)
   elif kind==13:out['enchantments']=[[read_varint(b.read),read_varint(b.read)] for _ in range(read_varint(b.read))]
   else:raise AssertionError('unknown fixture component '+str(kind))
  return out
