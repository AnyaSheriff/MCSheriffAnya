#!/usr/bin/env python3
"""NEW SDK → NEW Java JAR → NEW Place components; old WASM never retranslated."""
from pathlib import Path
import shutil
import translate
root=Path(__file__).resolve().parent
translate.bootstrap(toolchain=True)
for suffix in ['a','b','empty']:
    name='place-staged-'+suffix;folder=root/'fixtures'/name;target=root/'target/fixtures'/name
    assert not target.exists();target.mkdir(parents=True)
    translate.run('javac','-proc:none','--release','11','-encoding','UTF-8','-cp',root/'target/mcsa-bukkit.jar','-d',target,*sorted(folder.glob('*.java')))
    jar=root/'target'/(name+'.jar')
    translate.run('jar','cf',jar,'-C',target,'.','-C',folder,'plugin.yml')
    evidence=root.parents[1]/'place-staged-evidence/sdk-inputs';evidence.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(root/'target/mcsa-bukkit.jar',evidence/('sdk-'+suffix+'.jar'))
    translate.translate(jar,root.parents[1]/'tmp/place-staged/plugins',staged_block_place=True)
