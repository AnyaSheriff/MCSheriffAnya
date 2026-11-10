import io,struct,unittest
from slots import SlotDecoder
class SlotTests(unittest.TestCase):
 def test_damage_enchant_custom_typed_and_raw(self):
  nbt=b'\x0a\x03\0\x0bmcsa_layout'+struct.pack('>i',1)+b'\x08\0\x06marker\0\x0cLAYOUT_FIXED\0'
  payload=b'\x01\x05\x03\0\x03\x07\x0d\x01\x02\x03\0'+nbt
  out=SlotDecoder([dict(id=5,name='crossbow')]).slot(io.BytesIO(payload))
  self.assertEqual(out['damage'],7);self.assertEqual(out['enchantments'],[[2,3]])
  self.assertEqual(out['custom_data']['marker'],dict(type=8,value='LAYOUT_FIXED'))
  self.assertEqual(out['custom_data_raw_hex'],nbt.hex())
 def test_unknown_component_and_truncation_rejected(self):
  for payload in [b'\1\5\1\0\x63',b'\1\5\1\0\0\x0a\x03']:
   with self.assertRaises((AssertionError,EOFError,TypeError)):SlotDecoder([dict(id=5,name='crossbow')]).slot(io.BytesIO(payload))
if __name__=='__main__':unittest.main()
