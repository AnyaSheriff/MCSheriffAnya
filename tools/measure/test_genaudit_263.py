"""Independent synthetic saved data: packing, axes, seed and coverage guards."""
import contextlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import numpy as np

from genaudit_263_io import Reader, container, level, packed
from genaudit_263_generate import windows
from genaudit_263 import analyze, comparison, piece_boxes, batch, layout_metrics


class CensusTests(unittest.TestCase):
    def test_candidate_spacing_and_coincidence_are_separate_from_count(self):
        candidates=[{'type':'village','chunk':[x,0]} for x in (0,1,2)]
        official=[{'type':'village_plains','chunk':[x,0]} for x in (0,1,3)]
        row=layout_metrics(candidates,official)['village']
        self.assertEqual(row['ours_count'],row['vanilla_count'])
        self.assertEqual(row['coincident'],2)
        self.assertAlmostEqual(row['ours_coincident_percent'],200/3)
        self.assertEqual(row['ours_nearest_spacing']['50'],16)
        self.assertEqual(row['ours_to_official_distance']['100'],16)

    def test_shared_structure_modules_share_fix_batch(self):
        self.assertEqual(batch('overworld/structure_candidate_count/desert_pyramid')[:2],
                         batch('overworld/structure_candidate_count/swamp_hut')[:2])
        self.assertEqual(batch('overworld/structure_candidate_count/shipwreck')[:2],
                         batch('overworld/structure_candidate_count/buried_treasure')[:2])
        self.assertEqual(batch('overworld/structure_box_composition/ruined_portal/obsidian')[1],'ruined_portal.rs')

    def test_263_anvil_renamed_palette_and_section_coordinates(self):
        def text(s):
            b=s.encode();return struct.pack('>H',len(b))+b
        def tag(kind,name,payload):return bytes([kind])+text(name)+payload
        def compound(*tags):return b''.join(tags)+bytes([0])
        def string(name,value):return tag(8,name,text(value))
        # The official palette accepts the 26.3 empty-name/id forms. No game
        # implementation is used to construct this minimal synthetic region.
        palette=tag(9,'palette',bytes([10])+struct.pack('>i',1)+compound(string('','minecraft:stone')))
        states=tag(10,'block_states',compound(palette))
        bio=tag(10,'biomes',compound(tag(9,'palette',bytes([8])+struct.pack('>i',1)+text('minecraft:plains'))))
        section=compound(tag(1,'Y',struct.pack('>b',4)),states,bio)
        root=compound(string('Status','minecraft:full'),tag(3,'xPos',struct.pack('>i',-32)),
                      tag(3,'zPos',struct.pack('>i',0)),tag(9,'sections',bytes([10])+struct.pack('>i',1)+section))
        raw=bytes([10])+text('')+root
        compressed=zlib.compress(raw)
        header=bytearray(8192);header[:4]=bytes([0,0,2,1])
        record=struct.pack('>I',len(compressed)+1)+bytes([2])+compressed
        with tempfile.TemporaryDirectory() as tmp:
            file=Path(tmp,'r.-1.0.mca');file.write_bytes(header+record)
            reader=Reader();rows=list(reader.read(file))
            self.assertEqual(rows[0][:2],(-32,0))
            self.assertTrue(np.all(rows[0][2][128:144]==reader.lookup['stone']))
            self.assertTrue(np.all(rows[0][2][:128]==reader.lookup['air']))
            self.assertEqual(set(rows[0][3][8]),{'plains'})

    def test_anvil_padding_and_unsigned_longs(self):
        # Five bits: twelve entries per word; high four bits are padding.
        values = [(i*13)%32 for i in range(64)]
        words = []
        for start in range(0,64,12):
            word = 15 << 60
            for offset,value in enumerate(values[start:start+12]):
                word |= value << (offset*5)
            words.append(word-(1<<64))
        self.assertEqual(packed(words,5,64).tolist(),values)

    def test_mcs_contiguous_pack_crosses_bytes(self):
        # Two bits, reversed palette; no Anvil long alignment.
        raw = bytes([2,0,4])+struct.pack('>4H',40,30,20,10)+bytes([0b11100100]*16)
        values,end = container(raw,0,64)
        self.assertEqual(values.tolist(),[40,30,20,10]*16)
        self.assertEqual(end,len(raw))

    def test_negative_spawn_and_seed_precision(self):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp,'level.dat').write_text('seed = 1792932773345722119\nspawn = -527.5 64 96.5\n')
            self.assertEqual(level(tmp)['spawn'],[-528,96])
            self.assertEqual(level(tmp)['seed'],1792932773345722119)
            ow = windows([-528,96],1536)['overworld'][0]
            self.assertEqual(ow,[-129,-90,63,102])
            self.assertEqual((ow[2]-ow[0]+1)*(ow[3]-ow[1]+1),37249)

    def test_region_axis_surface_counts_and_coverage(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = Path(tmp)
            (world/'region').mkdir()
            (world/'level.dat').write_text('seed = 42\nspawn = -1 64 -1\n')
            # Direct MCS sections avoid reusing the encoder under test. One grass
            # column at x=3,z=9 is higher, with stone beneath and a flower above.
            blocks = np.zeros((384,16,16),dtype=np.uint16)
            blocks[:128] = 1
            blocks[128] = 8  # grass_block Y64
            blocks[129,9,3] = 1
            blocks[130,9,3] = 8
            reader = Reader()
            bio = reader.biomes.index('plains')
            payload = bytes([5])+b''.join(bytes([3])+s.astype('>u2').tobytes() for s in blocks.reshape(24,4096))
            payload += (bytes([1])+struct.pack('>H',bio))*24
            import ctypes
            lib = reader.lib
            lib.ZSTD_compressBound.argtypes=[ctypes.c_size_t]
            lib.ZSTD_compressBound.restype=ctypes.c_size_t
            dest=ctypes.create_string_buffer(lib.ZSTD_compressBound(len(payload)))
            lib.ZSTD_compress.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_int]
            lib.ZSTD_compress.restype=ctypes.c_size_t
            n=lib.ZSTD_compress(dest,len(dest),payload,len(payload),1)
            compressed=dest.raw[:n]
            record=struct.pack('>II',zlib.crc32(compressed),len(compressed))+compressed
            header=bytearray(33*256)
            header[:5]=b'MCS\x01\x01'
            slot=31+31*32  # chunk -1,-1 in region -1,-1
            struct.pack_into('>II',header,8+slot*8,33,len(record))
            path=world/'region/r.-1.-1.mcs'
            path.write_bytes(header+record)
            decoded=list(reader.read(path))
            self.assertEqual(decoded[0][:2],(-1,-1))
            self.assertEqual(int(decoded[0][2][130,9,3]),reader.lookup['grass_block'])
            plan={'windows':{'overworld':[[-1,-1,-1,-1]]}}
            with contextlib.redirect_stdout(io.StringIO()):
                stats=analyze(world,plan,[])
            ow=stats['dimensions']['overworld']
            self.assertEqual(ow['columns'],256)
            self.assertEqual(ow['height_histogram'],{'64':255,'66':1})
            self.assertEqual(ow['surface_biomes'],{'plains':256})
            self.assertEqual(ow['top_blocks_by_biome']['plains'],{'grass_block':256})
            box=[-16,64,-16,-1,64,-1]
            boxed=piece_boxes(reader,[path],[{'type':'probe','boxes':[box,box]}],{(-1,-1)},'overworld')
            self.assertEqual(boxed['materials']['probe'],{'grass_block':256})
            clipped=piece_boxes(reader,[path],[{'type':'probe','boxes':[[-17,64,-16,-1,64,-1]]}],{(-1,-1)},'overworld')
            self.assertEqual(clipped['coverage']['probe']['clipped_by_sample'],1)
            self.assertEqual(clipped['materials'],{})
            missing=json.loads(json.dumps(stats))
            missing['dimensions']['overworld']['coverage']['requested']=2
            with self.assertRaises(ValueError):
                comparison(missing,missing)
            report=comparison(stats,stats,allow_incomplete=True)
            self.assertFalse(any(r['verdict']=='outside' for r in report['rows']))
            json.dumps(report,allow_nan=False)
            nether=world/'dimensions/minecraft/the_nether/region'
            nether.mkdir(parents=True)
            (nether/path.name).write_bytes(path.read_bytes())
            with contextlib.redirect_stdout(io.StringIO()):
                dimensional=analyze(world,{'windows':{'overworld':[],'the_nether':[[-1,-1,-1,-1]],'the_end':[]}},[])
            nd=dimensional['dimensions']['the_nether']
            self.assertEqual(nd['measured_y_range'],[0,255])
            self.assertEqual(nd['outside_build_height_nonair_blocks'],64*256)
            self.assertEqual(nd['biomes_3d'],{'plains':1024})
            self.assertEqual(len(nd['depth_bands']),16)
            # Corruption must fail loudly instead of silently losing a chunk.
            corrupt=bytearray(path.read_bytes());corrupt[-1]^=1;path.write_bytes(corrupt)
            with self.assertRaises(ValueError):
                list(reader.read(path))


if __name__=='__main__':
    unittest.main()
