"""Офлайн проверки границ собственных проб; не сетевое доказательство776."""
import io
import struct
import unittest
from unittest.mock import patch

from probe_776_payloads import authored_section, metadata, motion, palette, validate_slots
from smoke import varint


def packed(values, width):
    result = bytes([width])
    per_word = 64 // width
    for start in range(0,len(values),per_word):
        value = sum(n << (i*width) for i,n in enumerate(values[start:start+per_word]))
        result += value.to_bytes(8,'big')
    return result


class PayloadTests(unittest.TestCase):
    def test_palette_direct_cross_word_and_short_reads(self):
        values = [0,1,32767,91,5,19,2]
        data = packed(values,15)
        self.assertEqual(palette(io.BytesIO(data),len(values),4,8),(15,values))
        for n in range(len(data)):
            with self.assertRaises((AssertionError,IndexError,EOFError)):
                palette(io.BytesIO(data[:n]),len(values),4,8)

    def test_authored_section_is_exact_and_natural_sections_not_saved(self):
        values = list(range(1,300)) + [400] * (4096-299)
        plain = struct.pack('>HH',4096,0) + b'\0' + varint(400) + b'\0\0'
        authored = struct.pack('>HH',4096,0) + packed(values,15) + b'\0\0'
        body = plain*10 + authored + plain*13
        result = authored_section(body,values)
        self.assertEqual((result['width'],result['unique_states']),(15,300))
        self.assertEqual(result['section_hex'],authored.hex())
        with self.assertRaises(AssertionError):authored_section(body+b'\0',values)
        wrong = values.copy();wrong[1]=77
        with self.assertRaises(AssertionError):authored_section(body,wrong)

    def test_metadata_retains_size_index_and_rejects_duplicate_unknown_tail(self):
        packet = bytes([0x63,4,18,1,4,255])
        self.assertEqual(metadata(packet),(4,{18:{'serializer':1,'value':4}}))
        for invalid in [packet[:-1],packet+b'\0',bytes([0x63,4,18,1,4,18,1,4,255]),bytes([0x63,4,18,44,255])]:
            with self.assertRaises((AssertionError,EOFError)):metadata(invalid)

    def test_old_form_relative_rotation_exhausts_payload(self):
        body = bytes([0x36,9])+struct.pack('>hhhBBB',1024,0,0,21,10,0)
        result = motion('entity_move_look',body)
        self.assertEqual(result,{'entity':9,'delta':[1024,0,0],'angles':[21,10],'on_ground':0})
        for invalid in [body[:-1],body+b'\0',body[:-1]+b'\2']:
            with self.assertRaises(AssertionError):motion('entity_move_look',invalid)

    def test_plain_and_removed_slot_assertions(self):
        cases=[{'name':'plain','command':'stick 3','expected_count':3,'expected_item':'stick','expected_components':{}},
               {'name':'empty','command':'air','expected_count':0}]
        report={'protocol':776,'exit_code':0,'cases':[{'name':'plain','command':'stick 3','slot_hex':'03050000'},
                  {'name':'empty','command':'air','slot_hex':'00'}]}
        with patch('probe_776_payloads.table',return_value=[{'name':'stick','id':5}]):
            self.assertEqual(len(validate_slots(report,cases,None)),2)
            report['cases'][0]['slot_hex']='0305000100'
            with self.assertRaises(AssertionError):validate_slots(report,cases,None)

    def test_shifted_component_is_verified_not_old_id(self):
        cases=[{'name':'collar','command':'stick[wolf/collar="red"]','expected_count':1,
                'expected_item':'stick','expected_components':{'85':14}}]
        report={'protocol':776,'exit_code':0,'cases':[{'name':'collar','command':cases[0]['command'],'slot_hex':'01050100550e'}]}
        with patch('probe_776_payloads.table',return_value=[{'name':'stick','id':5}]):
            self.assertEqual(validate_slots(report,cases,None)[0]['components'],{'85':14})
            report['cases'][0]['slot_hex']='01050100540e'
            with self.assertRaises(AssertionError):validate_slots(report,cases,None)


if __name__ == '__main__':
    unittest.main()
