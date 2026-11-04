#!/usr/bin/env python3
"""Собирает полную матрицу V2 из парных отчётов и журналов проверок.

Запускать из корня репозитория; исходные измерения лежат вне него.
Строки вне допуска сохраняются, допуск не пересчитывается по округлениям.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--audit-root',type=Path,default=Path.home()/'.cache/mcsheriffanya/gen-audit-263')
parser.add_argument('--artifacts',type=Path,required=True)
parser.add_argument('--output',type=Path,default=Path('testdata/world-profile/genaudit-26.3/vegetation-v2-results.json'))
args = parser.parse_args()
ROOT = args.audit_root
WORK = args.artifacts
DATA = Path('testdata/world-profile/genaudit-26.3')


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pair(before_path, after_path, baseline, plan):
    before, after = read(before_path), read(after_path)
    assert before['references'] == after['references']
    assert before['ours']['chunk_sha256'] == after['ours']['chunk_sha256']
    assert before['ours']['biomes_3d'] == after['ours']['biomes_3d']
    first = {r['metric']:r for r in before['rows']}
    last = {r['metric']:r for r in after['rows']}
    rows = []
    for metric in sorted(first.keys()|last.keys()):
        a,b = first.get(metric),last.get(metric)
        ref = b or a
        av,bv = (a['ours'] if a else 0),(b['ours'] if b else 0)
        tolerance = ref['tolerance']
        rows.append([metric,av,bv,ref['vanilla'],tolerance,
                     a['outside'] if a else abs(av-ref['vanilla'])>tolerance,
                     b['outside'] if b else abs(bv-ref['vanilla'])>tolerance])
    targets = sorted(read(DATA/plan)['centers'])
    target_rows = [r for r in rows if r[0].split('/')[1] in targets]
    return {'baseline':baseline,'plan':plan,'chunks':after['ours']['chunks'],
            'planned_target_biomes':targets,
            'planned_target_rows':len(target_rows),
            'planned_target_outside_before':sum(r[5] for r in target_rows),
            'planned_target_outside_after':sum(r[6] for r in target_rows),
            'planned_target_new_outside':[r[0] for r in target_rows if r[6] and not r[5]],
            'coverage_sha256':after['ours']['chunk_sha256'],
            'official_references':after['references'],
            'reports_sha256':{'before':digest(before_path),'after':digest(after_path)},
            'region_sha256':{'before':before['ours']['files'],'after':after['ours']['files']},
            'biome_blocks':{k:v*64 for k,v in after['ours']['biomes_3d'].items()},
            'row_columns':['metric','before','after','vanilla','tolerance','before_outside','after_outside'],
            'rows':rows,'outside_before':sum(r[5] for r in rows),
            'outside_after':sum(r[6] for r in rows),
            'new_outside':[r[0] for r in rows if r[6] and not r[5]],
            'closed':[r[0] for r in rows if r[5] and not r[6]],
            'kelp_height_histogram':{'before':before['ours']['kelp_height_histogram'],
                                     'after':after['ours']['kelp_height_histogram']},
            'surface_blocks_by_biome':{'before':before['ours']['surface_blocks_by_biome'],
                                      'after':after['ours']['surface_blocks_by_biome']}}


def verification():
    log = (WORK/'workspace-selector-tests.log').read_text()
    suites = [tuple(map(int,m)) for m in re.findall(
        r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out;',log)]
    assert suites and not any(s[1] for s in suites)
    assert 'test result: FAILED' not in log
    for name in ('preview_flower_patches_cross_biome_boundary_from_their_origin',
                 'preview_poplar_shelf_matches_measured_groups_and_log_support',
                 'preview_ground_streams_only_remove_selected_whole_plants'):
        assert name+' ... ok' in log,name
    clippy = (WORK/'clippy-selector.log').read_text()
    assert 'Finished' in clippy and 'error:' not in clippy and 'warning:' not in clippy
    python_log = (WORK/'python-final-tests.log').read_text()
    assert 'Ran 21 tests' in python_log and '\nOK\n' in python_log
    return {'python_measurement_tests':21,'cargo_test_release_workspace':dict(zip(('passed','failed','ignored'),
                [sum(s[i] for s in suites if s[3]==0) for i in range(3)])),
            'nested_filtered_child_excluded':True,
            'cargo_clippy_release_all_targets_workspace_D_warnings':'clean',
            'logs_sha256':{n:digest(WORK/n) for n in ('workspace-selector-tests.log','clippy-selector.log','python-final-tests.log')}}


result = {'schema':2,'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
          'climate_revision':'9910307','seed':1792932773345722119,
          'method':'Independent biome-conditioned densities per million 3D biome blocks; no coordinate matching; census tolerance max(5, 20% vanilla) unchanged',
          'main':pair(WORK/'before-complete-final.json',WORK/'after.json',
                      'HEAD d86c9c6 before all V2 generation edits','vegetation-v2-plan.json'),
          'population':pair(WORK/'population-before-complete-final.json',WORK/'population-after.json',
                      'V2 dark-oak geometry already applied; before subsequent F/O, spruce-litter, shelf-mushroom, patch-boundary and old-growth-tree changes',
                      'vegetation-v2-population-plan.json'),
          'verification':verification(),
          'limitations':[
              'This is not complete vegetation parity. Every outside row and every new outside row is retained.',
              'Global biome shares are not measured by targeted windows.',
              'Flower picker remains a seeded 2D hash rather than a coherent seed-independent 3D gradient; spatial measurements are in flower-gradient-v2.json.',
              'Exact official decoration seed/salt/placement order parity remains unverified; internal stream collisions and cross-feature interval coupling were addressed separately.',
              'Pale-garden eligible soil differs strongly from the reference; raw density failures are not hidden by soil-conditioned supplemental rates.',
              'Mangrove muddy-root density, forest/dappled litter, cherry geometry, windswept composition and fallen-log terrain checks still require investigation.',
              'Snowy-taiga/poplar boundary exposure is insufficient; zero/zero in these windows does not prove absence in every world.',
              'Bamboo surrounding podzol geometry is unverified.']}
result['limitations'].extend([
    'Statistical V2 comparisons here use the owner seed; generalization to other seeds is unverified.',
    'Rare sparse-jungle bamboo counts have not yet distinguished local origins from biome-boundary spill.'])

result['boundary_diagnostics'] = {label:read(WORK/file) for label,file in (
    ('ours_before_old_birch','boundary-ours.json'),('ours_before_plains','boundary-fine-ours.json'),
    ('ours_after_old_birch','boundary-verified.json'),('ours_after_plains','boundary-fine-verified.json'),
    ('vanilla','boundary-vanilla.json'))}
result['explicit_zero_row'] = {'metric':'feature_biome/snowy_taiga/yellow_poplar_leaves',
    'before':0,'after':0,'vanilla':0,'note':'Recorded explicitly despite zero/zero omission in sparse row lists; insufficient adjacent dappled-forest exposure.'}

soil = {'grass_block','dirt','coarse_dirt','podzol','rooted_dirt','pale_moss_block'}
vanilla_pale = read(ROOT/'reports/targets/biome-pale_garden.json')['statistics']['vanilla']['dimensions']['overworld']
after_pale = read(WORK/'population-after.json')['ours']
before_pale = read(WORK/'population-before-complete-final.json')['ours']
soil_context = {}
for label,side,field in [('vanilla',vanilla_pale,'top_blocks_by_biome'),
                         ('before',before_pale,'surface_blocks_by_biome'),
                         ('after',after_pale,'surface_blocks_by_biome')]:
    ground = side[field]['pale_garden']
    eligible = sum(n for name,n in ground.items() if name in soil)
    count = side['features_by_biome']['pale_garden']
    soil_context[label] = {'natural_surface_columns':ground,'eligible_columns':eligible,
        'biome_blocks':side['biomes_3d']['pale_garden']*64,
        'blocks_per_1000_eligible_columns':{name:count.get(name,0)*1000/eligible for name in (
            'short_grass','tall_grass','closed_eyeblossom')}}
result['pale_garden_supplemental_soil_context'] = soil_context
sources = ['src/world/trees.rs','src/world/tree_shapes_26_3.rs','src/world/vegetation.rs','src/world/ocean.rs']
result['source_sha256'] = {f:digest(Path(f)) for f in sources}
result['measurement_source_sha256'] = {str(f):digest(f) for f in Path('tools/measure').glob('vegetation*263*.py')}
result['measurement_source_sha256']['tools/measure/pack_crowns_263.py'] = digest(Path('tools/measure/pack_crowns_263.py'))
result['head_source_sha256'] = {f:hashlib.sha256(subprocess.check_output(['git','show','HEAD:'+f])).hexdigest() for f in sources}
fixtures = [p for p in DATA.glob('*v2*') if p.is_file() and p.name!='vegetation-v2-results.json']
fixtures += [DATA/'vegetation-regression.json',DATA/'vegetation-v2-reference/biome-dappled_forest.json']
result['fixtures_sha256'] = {str(p):digest(p) for p in fixtures}
args.output.write_text(json.dumps(result,separators=(',',':'))+'\n')
for label in ('main','population'):
    side=result[label]
    print(label,len(side['rows']),side['outside_before'],'->',side['outside_after'],
          'new outside',side['new_outside'])
print(result['verification'])
