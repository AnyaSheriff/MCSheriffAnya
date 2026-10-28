"""Проверки геометрии счётчиков на независимых маленьких примерах."""
import unittest
import numpy as np
from genaudit_263_end_edges import diagnose


def row(rows, zone='all', location='border', kind='all'):
    return next(r for r in rows if (r['zone'],r['location'],r['kind']) == (zone,location,kind))


class EdgesTest(unittest.TestCase):
    def test_single_chunk_denominator_and_threshold(self):
        h = np.full((16,16), 20)
        h[:,8:] = 36
        rows = diagnose({(0,0):h})
        self.assertEqual(row(rows,location='interior')['denominator'], 480)
        self.assertEqual(row(rows,location='interior')['jump_ge16_count'], 16)
        h[:,8:] = 35
        self.assertEqual(row(diagnose({(0,0):h}),location='interior')['jump_ge16_count'], 0)

    def test_negative_coordinate_border_and_void(self):
        rows = diagnose({(-1,0): np.full((16,16),-1), (0,0):np.full((16,16),15)})
        self.assertEqual(row(rows)['denominator'],16)
        self.assertEqual(row(rows,kind='land_void')['jump_ge16_count'],16)
        self.assertEqual(row(rows,kind='land_land')['denominator'],0)
        self.assertIsNone(row(rows,kind='land_land')['share'])

    def test_missing_chunk_is_not_void_and_no_wraparound(self):
        rows = diagnose({(-1,0):np.zeros((16,16),int),(1,0):np.full((16,16),50)})
        self.assertFalse(any(r['location']=='border' for r in rows))
        self.assertEqual(row(rows,location='interior')['denominator'],960)

    def test_both_axes_once_and_partition(self):
        chunks = {(x,z): np.full((16,16),x*20+z*40) for x in [0,1] for z in [0,1]}
        rows=diagnose(chunks)
        self.assertEqual(row(rows)['denominator'],64)
        self.assertEqual(row(rows)['jump_ge16_count'],64)
        for location in ['border','interior']:
            total=row(rows,location=location)
            self.assertEqual(total['denominator'],sum(row(rows,location=location,kind=k)['denominator']
                for k in ['land_void','land_land','void_void']))
            self.assertEqual(total['denominator'],sum(row(rows,zone=z,location=location)['denominator']
                for z in ['main','gap','outer']))

    def test_outer_negative_and_gap(self):
        for cx,zone in [(-100,'outer'),(20,'gap'),(0,'main')]:
            rows=diagnose({(cx,0):np.zeros((16,16),int)})
            self.assertEqual(row(rows,zone=zone,location='interior')['denominator'],480)

    def test_sparse_map_against_scalar_oracle(self):
        # Независимый перебор мировых координат, включая отрицательные стыки.
        rng=np.random.default_rng(20261028)
        chunks={(x,z):rng.choice([-1,0,14,15,16,40],(16,16))
                for x,z in [(-1,-1),(0,-1),(-1,0),(1,0)]}
        cells={(cx*16+x,cz*16+z):int(h[z,x]) for (cx,cz),h in chunks.items()
               for z in range(16) for x in range(16)}
        expected={loc:[0,0] for loc in ['border','interior']}
        for (x,z),a in cells.items():
            for dx,dz in [(1,0),(0,1)]:
                if (x+dx,z+dz) not in cells: continue
                b=cells[x+dx,z+dz]
                border=(x//16 != (x+dx)//16 or z//16 != (z+dz)//16)
                r=expected['border' if border else 'interior']
                r[0]+=1
                r[1]+=abs(a-b)>=16
        rows=diagnose(chunks)
        for loc,(n,count) in expected.items():
            self.assertEqual(row(rows,location=loc)['denominator'],n)
            self.assertEqual(row(rows,location=loc)['jump_ge16_count'],count)

    def test_reject_invalid(self):
        for h in [np.zeros((15,16),int), np.full((16,16),-2),np.zeros((16,16),float)]:
            with self.assertRaises(ValueError): diagnose({(0,0):h})


if __name__ == '__main__':
    unittest.main()
