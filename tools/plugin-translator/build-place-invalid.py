#!/usr/bin/env python3
"""Собственные NEW components с присутствующим, но неполным/неверным Place namespace."""
from pathlib import Path
import tempfile
import shutil
import translate
import place_priority_wit
root=Path(__file__).resolve().parent
translate.bootstrap(toolchain=True)
original_prepare=place_priority_wit.prepare
for kind in ['missing','type']:
    def prepare(destination):
        package=original_prepare(destination);p=package/'world.wit';s=p.read_text()
        old='finish: func' if kind=='missing' else '-> result<state, frame-error>'
        new='finisz: func' if kind=='missing' else '-> result<bool, frame-error>'
        assert s.count(old)==1;p.write_text(s.replace(old,new));return package
    place_priority_wit.prepare=prepare
    try:
        with tempfile.TemporaryDirectory(prefix='place-invalid-',dir=translate.TARGET) as folder:
            folder=Path(folder);adapter=(root/'adapter.c').read_text()
            if kind=='missing':
                old='exports_mcsa_place_priority_events_block_place_finish('
                assert adapter.count(old)==1;adapter=adapter.replace(old,'exports_mcsa_place_priority_events_block_place_finisz(')
            else:
                old='exports_mcsa_place_priority_events_block_place_state_t *ret,'
                assert adapter.count(old)==1;adapter=adapter.replace(old,'bool *ret,')
                old='ret->cancelled=(code&1)!=0;ret->can_build=(code&2)!=0;'
                assert adapter.count(old)==1;adapter=adapter.replace(old,'*ret=(code&1)!=0;')
            path=folder/'invalid-adapter.c';path.write_text(adapter)
            translate.translate(root/'target/place-staged-a.jar',folder/'output',adapter=path,staged_block_place=True)
            name='java-place-staged-invalid-'+kind;out=root.parents[1]/'tmp/place-staged/plugins'/name;assert not out.exists()
            shutil.copytree(folder/'output/java-place-staged-a',out)
            p=out/'plugin.toml';p.write_text(p.read_text().replace('java-place-staged-a',name))
            evidence=root.parents[1]/'place-staged-evidence';evidence.mkdir(exist_ok=True)
            (evidence/('invalid-'+kind+'-adapter.c')).write_text(adapter)
            (evidence/('invalid-'+kind+'-world.wit')).write_text(prepare(folder/'evidence-package').joinpath('world.wit').read_text())
    finally:
        place_priority_wit.prepare=original_prepare
