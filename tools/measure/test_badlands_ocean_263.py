# Регрессии измерителя: неизвестный край не считается берегом, маски совпадают.
import csv
import json
import tempfile
import unittest
from pathlib import Path
import badlands_ocean_263 as measure


class OceanMeasurementTests(unittest.TestCase):
    def write_grid(self, path, width=7):
        with path.open('w') as stream:
            out=csv.writer(stream)
            out.writerow(['x','z','biome','height'])
            for z in range(7):
                for x in range(width):
                    out.writerow([x*4,z*4,'plains' if (x,z)==(3,3) else 'warm_ocean',64 if (x,z)==(3,3) else 40])

    def test_known_shore_and_unknown_edges(self):
        with tempfile.TemporaryDirectory() as root:
            source=Path(root)/'grid.csv';out=Path(root)/'out.json'
            self.write_grid(source)
            measure.ocean(source,None,out)
            row=json.loads(out.read_text())['biomes']['warm_ocean']
            self.assertEqual(row['cells'],48)
            self.assertEqual(row['distance_quantiles_exact'][0],4)
            self.assertGreater(row['censored'],0)
            self.assertEqual(row['exact_distances']+row['censored'],48)

    def test_physical_land_is_distinct_from_biome_boundaries(self):
        with tempfile.TemporaryDirectory() as root:
            source=Path(root)/'grid.csv';out=Path(root)/'out.json'
            self.write_grid(source)
            with source.open() as stream:
                rows=list(csv.DictReader(stream))
            next(r for r in rows if (r['x'],r['z'])==('12','8'))['height']='63'
            with source.open('w') as stream:
                writer=csv.DictWriter(stream,fieldnames=rows[0].keys())
                writer.writeheader();writer.writerows(rows)
            measure.ocean(source,None,out,physical=True)
            row=json.loads(out.read_text())['biomes']['warm_ocean']
            self.assertEqual(row['on_dry_land_columns'],1)
            self.assertEqual(row['distance_quantiles_exact'][0],0)
            measure.ocean(source,None,out)
            self.assertEqual(json.loads(out.read_text())['biomes']['warm_ocean']['distance_quantiles_exact'][0],4)

    def test_incomplete_export_cannot_change_comparison_area(self):
        with tempfile.TemporaryDirectory() as root:
            source=Path(root)/'partial.csv';mask=Path(root)/'full.csv'
            self.write_grid(source,6);self.write_grid(mask)
            with self.assertRaisesRegex(AssertionError,'full reference mask'):
                measure.ocean(source,mask,Path(root)/'out.json')


if __name__=='__main__':
    unittest.main()
